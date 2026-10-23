#include "vlikeon_encoder.hpp"
#include "vlikeon_capture.hpp"
#include "vlikeon_colour.hpp"
#include "vlikeon_decoder.hpp"
#include <mferror.h>
#include <array>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cerrno>
#include <cstdlib>
#include <cwchar>
#include <thread>

// Raw v1 accepts owned NV12 only. Owned v2 additionally requires an inherited
// query/synchronize game HANDLE, a pinned HWND and native local confirmation.
// No file/network destination or arbitrary encoder CLSID is accepted.
// Decoder v3 accepts admitted Annex B only over the parent's private pipe.
namespace {
using vlikeon::video::Configuration;
using vlikeon::video::Encoder;
using vlikeon::video::Packet;
using vlikeon::video::WindowCapture;
using vlikeon::video::BgraFrame;
using vlikeon::video::Decoder;
using vlikeon::video::DecodedFrame;
constexpr uint32_t max_input=1920*1080*3/2+9;
constexpr uint16_t configure=1,frame=2,poll=3,stop=4,keyframe=5,status_reply=0x8000,packet_reply=0x8001;
constexpr uint16_t decoded_reply=0x8002;
struct Command{uint16_t kind;uint64_t id,received;std::vector<uint8_t> bytes;};
struct Reply{std::vector<uint8_t> bytes;bool terminal;uint64_t expires;};
struct Shared{
  HANDLE input,output;
  HANDLE game; // Borrowed from wmain; WindowCapture duplicates least privilege.
  uint16_t version;
  std::mutex mutex;
  std::condition_variable changed;
  std::deque<Command> commands;
  std::deque<Reply> replies;
  std::atomic<bool> broken{false},reader_done{false},writer_done{false},finished{false};
  std::atomic<uint64_t> stop_id{0},deadline{UINT64_MAX};
  Shared(HANDLE in,HANDLE out,HANDLE target,bool decode):input(in),output(out),game(target),version(decode?3:target?2:1){}
  void fail(){broken.store(true);changed.notify_all();}
};
uint64_t integer(std::span<const uint8_t> bytes,size_t offset,unsigned count){
  uint64_t value=0;for(unsigned i=0;i<count;++i)value|=uint64_t(bytes[offset+i])<<(8*i);return value;
}
void append(std::vector<uint8_t>& bytes,uint64_t value,unsigned count){
  for(unsigned i=0;i<count;++i)bytes.push_back(static_cast<uint8_t>(value>>(8*i)));
}
bool read_exact(HANDLE input,std::span<uint8_t> bytes){
  while(!bytes.empty()){
    DWORD read=0;
    if(!ReadFile(input,bytes.data(),static_cast<DWORD>(bytes.size()),&read,nullptr)||!read)return false;
    bytes=bytes.subspan(read);
  }return true;
}
void reader(const std::shared_ptr<Shared>& shared){
  uint64_t previous=0;
  for(;;){
    std::array<uint8_t,20> header{};
    if(!read_exact(shared->input,header))break;
    uint16_t kind=static_cast<uint16_t>(integer(header,6,2));
    uint64_t id=integer(header,8,8);uint32_t length=static_cast<uint32_t>(integer(header,16,4));
    if(integer(header,0,4)!=0x564b4c56||integer(header,4,2)!=shared->version||
        kind<configure||kind>(shared->game?keyframe:stop)||id!=previous+1||id>10000000||length>max_input||
        (kind==configure&&length!=(shared->game?69u:61u))||(kind==frame&&(shared->game||length<9))||
        (kind==frame&&shared->version==3&&(length<=16||length>2*1024*1024+16))||
        ((kind==poll||kind==stop||kind==keyframe)&&length!=0)){shared->fail();break;}
    Command command{kind,id,GetTickCount64(),std::vector<uint8_t>(length)};
    if(!read_exact(shared->input,command.bytes))break;
    previous=id;
    if(kind==stop){shared->stop_id.store(id);shared->changed.notify_all();break;}
    {
      std::lock_guard lock(shared->mutex);
      if(shared->commands.size()>=2){shared->fail();break;}
      shared->commands.push_back(std::move(command));
    }
    shared->changed.notify_all();
    if(shared->broken.load())break;
  }
  shared->reader_done.store(true);shared->changed.notify_all();
}
bool write_exact(const std::shared_ptr<Shared>& shared,const Reply& reply){
  std::span<const uint8_t> bytes=reply.bytes;
  while(!bytes.empty()){
    if(shared->broken.load()||(!reply.terminal&&
        (shared->stop_id.load()||GetTickCount64()>=shared->deadline.load()||GetTickCount64()>=reply.expires)))return false;
    DWORD written=0;
    if(!WriteFile(shared->output,bytes.data(),static_cast<DWORD>(bytes.size()),&written,nullptr)||!written)return false;
    bytes=bytes.subspan(written);
  }return true;
}
void writer(const std::shared_ptr<Shared>& shared){
  for(;;){
    Reply reply;
    {
      std::unique_lock lock(shared->mutex);
      shared->changed.wait(lock,[&]{return shared->broken.load()||shared->finished.load()||!shared->replies.empty();});
      if(shared->broken.load())break;
      if(shared->replies.empty()){if(shared->finished.load())break;continue;}
      reply=std::move(shared->replies.front());shared->replies.pop_front();
    }
    // Whole unsent replies can be discarded when Stop supersedes old calls.
    // If a reply was partially written, write_exact fails and closes the whole
    // transport. Never append a Stop receipt onto a truncated packet frame.
    if(!reply.terminal&&shared->stop_id.load())continue;
    if(!write_exact(shared,reply)){shared->fail();break;}
  }
  shared->writer_done.store(true);shared->changed.notify_all();
}
bool send(const std::shared_ptr<Shared>& shared,uint16_t kind,uint64_t id,
    std::vector<uint8_t> payload,bool terminal=false,uint64_t media_deadline=UINT64_MAX){
  size_t maximum=kind==decoded_reply?1920*1080*3/2+32:2*1024*1024+65536+32;
  if(payload.size()>maximum||(kind==decoded_reply&&shared->version!=3))return false;
  std::vector<uint8_t> bytes;bytes.reserve(20+payload.size());
  append(bytes,0x564b4c56,4);append(bytes,shared->version,2);append(bytes,kind,2);append(bytes,id,8);append(bytes,payload.size(),4);
  bytes.insert(bytes.end(),payload.begin(),payload.end());
  {
    std::lock_guard lock(shared->mutex);
    if(shared->broken.load())return false;
    if(terminal)shared->replies.clear();
    if(shared->replies.size()>=2)return false;
    shared->replies.push_back(Reply{std::move(bytes),terminal,
        (kind==packet_reply||kind==decoded_reply)?std::min<uint64_t>(GetTickCount64()+100,media_deadline):shared->deadline.load()});
  }
  shared->changed.notify_all();return true;
}
bool status(const std::shared_ptr<Shared>& shared,uint64_t id,HRESULT result,bool terminal=false){
  std::vector<uint8_t> payload;append(payload,static_cast<uint32_t>(result),4);
  return send(shared,status_reply,id,std::move(payload),terminal);
}
bool nonzero(std::span<const uint8_t> bytes){for(uint8_t byte:bytes)if(byte)return true;return false;}
int actor(const std::shared_ptr<Shared>& shared){
  std::unique_ptr<Encoder> encoder;
  std::unique_ptr<WindowCapture> capture;
  std::unique_ptr<Decoder> decoder;
  struct Captured{int64_t pts;uint64_t sequence,received;bool keyframe;};
  std::deque<Captured> captured;
  uint64_t capture_sequence=0;bool force_keyframe=true;
  uint64_t started=GetTickCount64(),parent_deadline=started+5000;
  uint32_t raw_size=0;
  bool configured=false,explicit_stop=false;
  HRESULT terminal_status=S_OK;
  for(;;){
    if(shared->broken.load())break;
    uint64_t stop_request=shared->stop_id.load();
    if(stop_request){
      // Fence delivery before any potentially blocking MF stop operation.
      if(capture)capture->close_delivery();
      if(encoder)encoder->close_delivery();
      if(decoder)decoder->close_delivery();
      HRESULT capture_status=capture?capture->stop():S_OK;
      HRESULT encode_status=encoder?encoder->stop():S_OK;
      HRESULT decode_status=decoder?decoder->stop():S_OK;
      terminal_status=FAILED(capture_status)?capture_status:FAILED(encode_status)?encode_status:decode_status;
      explicit_stop=status(shared,stop_request,terminal_status,true);break;
    }
    if(shared->reader_done.load())break;
    uint64_t now=GetTickCount64();
    if(now>=parent_deadline||now>=shared->deadline.load())break;
    if(encoder){terminal_status=encoder->poll();if(FAILED(terminal_status))break;}
    if(decoder){terminal_status=decoder->poll();if(FAILED(terminal_status))break;}
    if(capture&&configured&&captured.size()<6){
      BgraFrame image;HRESULT media=capture->take(image);
      if(FAILED(media)){if(shared->stop_id.load())continue;terminal_status=media;break;}
      if(media==S_OK){
        // Stop/expiry can arrive while a native GPU call or conversion blocks.
        // Do not submit the returned image after that authority was withdrawn.
        if(shared->stop_id.load())continue;
        std::vector<uint8_t> nv12;media=vlikeon::video::to_nv12(image,nv12);
        if(FAILED(media)){terminal_status=media;break;}
        now=GetTickCount64();
        if(shared->stop_id.load())continue;
        if(now>=parent_deadline||now>=shared->deadline.load())break;
        if(now-image.received_tick_ms<=100){
          if(image.timestamp_us>static_cast<uint64_t>(INT64_MAX)/10||capture_sequence>=10000000){terminal_status=E_INVALIDARG;break;}
          int64_t pts=static_cast<int64_t>(image.timestamp_us*10);
          media=encoder->submit(nv12,pts,force_keyframe);
          if(FAILED(media)){terminal_status=media;break;}
          if(media==S_OK){captured.push_back(Captured{pts,++capture_sequence,image.received_tick_ms,force_keyframe});force_keyframe=false;}
        } // Expired/unaccepted raw input is dropped, with keyframe intent kept.
      }
    }
    Command command;
    {
      std::unique_lock lock(shared->mutex);
      if(shared->commands.empty()){
        shared->changed.wait_for(lock,std::chrono::milliseconds(8));continue;
      }
      command=std::move(shared->commands.front());shared->commands.pop_front();
    }
    if(GetTickCount64()-command.received>100){terminal_status=HRESULT_FROM_WIN32(ERROR_TIMEOUT);break;}
    HRESULT result=E_INVALIDARG;bool just_configured=false;
    if(command.kind==configure&&!configured&&command.id==1){
      auto& bytes=command.bytes;
      uint64_t generation=integer(bytes,32,8);
      Configuration config{static_cast<uint32_t>(integer(bytes,40,4)),static_cast<uint32_t>(integer(bytes,44,4)),
          static_cast<uint32_t>(integer(bytes,48,4)),static_cast<uint32_t>(integer(bytes,52,4)),
          static_cast<uint32_t>(integer(bytes,56,4)),bytes[60]!=0};
      if(!nonzero(std::span(bytes).first(16))||!nonzero(std::span(bytes).subspan(16,16))||
          !generation||generation>1000000||bytes[60]>1||!config.remaining_ms||config.remaining_ms>1800000||
          !((config.width==1280&&config.height==720)||(config.width==1920&&config.height==1080))||
          !(config.fps==30||config.fps==60)||config.bitrate<500000||config.bitrate>30000000){
        status(shared,command.id,E_INVALIDARG);break;
      }
      uint64_t expires=command.received+config.remaining_ms;
      now=GetTickCount64();
      if(now>=expires){status(shared,command.id,MF_E_SHUTDOWN);break;}
      config.remaining_ms=static_cast<uint32_t>(expires-now);shared->deadline.store(expires);
      if(shared->version==3){
        if(config.require_hardware){status(shared,command.id,E_INVALIDARG);break;}
        result=Decoder::create({config.width,config.height,config.fps,config.remaining_ms},decoder);
        if(FAILED(result)){status(shared,command.id,result);break;}
        configured=true;just_configured=true;
        std::vector<uint8_t> receipt;append(receipt,0,4);receipt.insert(receipt.end(),bytes.begin(),bytes.end());
        receipt.push_back(0); // No hardware-decoding observation is claimed.
        if(!send(shared,status_reply,command.id,std::move(receipt)))break;
      }else{
      if(shared->game){
        uint64_t window=integer(bytes,61,8);
        if(!window){status(shared,command.id,E_INVALIDARG);break;}
        result=WindowCapture::confirm_and_create(shared->game,reinterpret_cast<HWND>(static_cast<uintptr_t>(window)),
            {config.width,config.height,config.fps,config.remaining_ms},capture,[shared]{
              return !shared->broken.load()&&!shared->reader_done.load()&&!shared->stop_id.load()&&GetTickCount64()<shared->deadline.load();
            });
        if(FAILED(result)){if(shared->stop_id.load())continue;status(shared,command.id,result);break;}
        now=GetTickCount64();
        if(shared->stop_id.load())continue;
        if(now>=expires){status(shared,command.id,MF_E_SHUTDOWN);break;}
        config.remaining_ms=static_cast<uint32_t>(expires-now);
      }
      result=Encoder::create(config,encoder);
      if(SUCCEEDED(result)){
        configured=true;just_configured=true;raw_size=config.width*config.height*3/2;
        // Receipt echoes the immutable private binding and requested profile;
        // final byte classifies the selected registered MFT, not measured GPU use.
        std::vector<uint8_t> receipt;append(receipt,static_cast<uint32_t>(result),4);
        receipt.insert(receipt.end(),bytes.begin(),bytes.end());receipt.push_back(encoder->hardware_mft()?1:0);
        if(!send(shared,status_reply,command.id,std::move(receipt)))break;
      }else{status(shared,command.id,result);break;}
      }
    }else if(command.kind==frame&&configured&&shared->version==3){
      uint64_t pts=integer(command.bytes,0,8),received=integer(command.bytes,8,8);
      if(pts>static_cast<uint64_t>(INT64_MAX)){status(shared,command.id,E_INVALIDARG);break;}
      result=decoder->submit(std::span(command.bytes).subspan(16),static_cast<int64_t>(pts),received);
      if(!status(shared,command.id,result)||FAILED(result))break;
    }else if(command.kind==frame&&configured&&!shared->game){
      if(command.bytes.size()!=static_cast<size_t>(raw_size)+9||command.bytes[8]>1){status(shared,command.id,E_INVALIDARG);break;}
      uint64_t pts=integer(command.bytes,0,8);
      if(pts>static_cast<uint64_t>(INT64_MAX)){status(shared,command.id,E_INVALIDARG);break;}
      result=encoder->submit(std::span(command.bytes).subspan(9),static_cast<int64_t>(pts),command.bytes[8]!=0);
      if(!status(shared,command.id,result)||FAILED(result))break;
    }else if(command.kind==poll&&configured&&shared->version==3){
      DecodedFrame image;result=decoder->take(image);
      if(result==S_OK){
        if(image.received_tick_ms>GetTickCount64()||GetTickCount64()-image.received_tick_ms>100){terminal_status=MF_E_SHUTDOWN;break;}
        std::vector<uint8_t> payload;payload.reserve(32+image.nv12.size());
        append(payload,static_cast<uint64_t>(image.timestamp_100ns),8);
        append(payload,image.width,4);append(payload,image.height,4);append(payload,image.stride,4);
        append(payload,image.received_tick_ms,8);append(payload,image.nv12.size(),4);
        payload.insert(payload.end(),image.nv12.begin(),image.nv12.end());
        if(!send(shared,decoded_reply,command.id,std::move(payload),false,image.received_tick_ms+100))break;
      }else if(!status(shared,command.id,result)||FAILED(result))break;
    }else if(command.kind==poll&&configured){
      Packet packet;result=encoder->take(packet);
      if(result==S_OK){
        std::vector<uint8_t> payload;payload.reserve(29+packet.bytes.size()+packet.sequence_header.size());
        append(payload,static_cast<uint64_t>(packet.timestamp_100ns),8);append(payload,packet.keyframe?1:0,1);
        append(payload,packet.bytes.size(),4);append(payload,packet.sequence_header.size(),4);
        uint64_t media_deadline=UINT64_MAX;
        if(shared->game){
          if(captured.empty()||captured.front().pts!=packet.timestamp_100ns||
              (captured.front().keyframe&&!packet.keyframe)||GetTickCount64()-captured.front().received>=250){terminal_status=E_UNEXPECTED;break;}
          auto source=captured.front();captured.pop_front();media_deadline=source.received+250;
          append(payload,source.sequence,8);append(payload,GetTickCount64()-source.received,4);
        }
        payload.insert(payload.end(),packet.bytes.begin(),packet.bytes.end());
        payload.insert(payload.end(),packet.sequence_header.begin(),packet.sequence_header.end());
        if(!send(shared,packet_reply,command.id,std::move(payload),false,media_deadline))break;
      }else if(!status(shared,command.id,result)||FAILED(result))break;
    }else if(command.kind==keyframe&&configured&&shared->game){
      force_keyframe=true;std::vector<uint8_t> receipt;append(receipt,0,4);append(receipt,capture_sequence+1,8);
      if(!send(shared,status_reply,command.id,std::move(receipt)))break;
    }else{status(shared,command.id,E_INVALIDARG);break;}
    // Keepalive only proves the private parent transport is responsive. It is
    // not independent evidence of live game/capture/document/pair authority.
    parent_deadline=just_configured?GetTickCount64()+500:command.received+500;
  }
  if(encoder)encoder->close_delivery();
  if(capture)capture->close_delivery();
  if(decoder)decoder->close_delivery();
  if(!explicit_stop)shared->fail();
  shared->finished.store(true);shared->changed.notify_all();
  if(explicit_stop){
    std::unique_lock lock(shared->mutex);
    shared->changed.wait_for(lock,std::chrono::milliseconds(500),[&]{return shared->writer_done.load();});
  }
  // Destructor may call the OS; only the external process supervisor can bound
  // a stuck driver. No exit code is a substitute for the actual Stop receipt.
  return explicit_stop&&SUCCEEDED(terminal_status)&&shared->writer_done.load()&&!shared->broken.load()?0:2;
}
int run(HANDLE game=nullptr,bool decode=false){
  HANDLE input=GetStdHandle(STD_INPUT_HANDLE),output=GetStdHandle(STD_OUTPUT_HANDLE);
  if(!input||!output||input==INVALID_HANDLE_VALUE||output==INVALID_HANDLE_VALUE||
      GetFileType(input)!=FILE_TYPE_PIPE||GetFileType(output)!=FILE_TYPE_PIPE)return 2;
  if(!SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32))return 2;
  HRESULT initialized=CoInitializeEx(nullptr,COINIT_MULTITHREADED);
  if(FAILED(initialized))return 2;
  int result=2;
  std::shared_ptr<Shared> shared;
  try{
    shared=std::make_shared<Shared>(input,output,game,decode);
    std::thread([shared]{try{reader(shared);}catch(...){shared->fail();shared->reader_done.store(true);}}).detach();
    std::thread([shared]{try{writer(shared);}catch(...){shared->fail();shared->writer_done.store(true);}}).detach();
    result=actor(shared);
  }catch(...){if(shared)shared->fail();result=2;}
  CoUninitialize();return result;
}
}
int wmain(int argc,wchar_t** argv){
  if(argc==2&&std::wcscmp(argv[1],L"--stdio-video-v1")==0)return run();
  if(argc==2&&std::wcscmp(argv[1],L"--stdio-decode-v3")==0)return run(nullptr,true);
  if(argc!=3||std::wcscmp(argv[1],L"--stdio-owned-video-v2")!=0)return 2;
  size_t length=std::wcslen(argv[2]);
  if(!length||length>20||argv[2][0]==L'0')return 2;
  for(size_t i=0;i<length;++i)if(argv[2][i]<L'0'||argv[2][i]>L'9')return 2;
  errno=0;wchar_t* end=nullptr;unsigned long long raw=std::wcstoull(argv[2],&end,10);
  if(errno||!end||*end||raw>UINTPTR_MAX)return 2;
  HANDLE game=reinterpret_cast<HANDLE>(static_cast<uintptr_t>(raw));DWORD flags=0;
  if(!game||game==INVALID_HANDLE_VALUE||!GetHandleInformation(game,&flags)||!(flags&HANDLE_FLAG_INHERIT)||!GetProcessId(game))return 2;
  int result=run(game);CloseHandle(game);return result;
}
