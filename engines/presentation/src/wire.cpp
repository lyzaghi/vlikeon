#include "wire.hpp"
#include <algorithm>
#include <cstring>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#else
#include <csignal>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

namespace omni::presentation {
static constexpr std::array<std::uint8_t,8> magic{'O','M','P','R','E','S','0','2'};
std::uint16_t u16(std::span<const std::uint8_t> b,std::size_t at){check(at<=b.size()&&b.size()-at>=2);return std::uint16_t(b[at])|(std::uint16_t(b[at+1])<<8);}
std::uint32_t u32(std::span<const std::uint8_t> b,std::size_t at){check(at<=b.size()&&b.size()-at>=4);return std::uint32_t(b[at])|(std::uint32_t(b[at+1])<<8)|(std::uint32_t(b[at+2])<<16)|(std::uint32_t(b[at+3])<<24);}
std::uint64_t u64(std::span<const std::uint8_t> b,std::size_t at){return std::uint64_t(u32(b,at))|(std::uint64_t(u32(b,at+4))<<32);}
void put16(std::span<std::uint8_t>b,std::size_t at,std::uint16_t v){check(at<=b.size()&&b.size()-at>=2);for(unsigned i=0;i<2;i++)b[at+i]=std::uint8_t(v>>(8*i));}
void put32(std::span<std::uint8_t>b,std::size_t at,std::uint32_t v){check(at<=b.size()&&b.size()-at>=4);for(unsigned i=0;i<4;i++)b[at+i]=std::uint8_t(v>>(8*i));}
void put64(std::span<std::uint8_t>b,std::size_t at,std::uint64_t v){check(at<=b.size()&&b.size()-at>=8);for(unsigned i=0;i<8;i++)b[at+i]=std::uint8_t(v>>(8*i));}
Session session_from_text(std::span<const char> text){
  check(text.size()==36);Session result{};std::size_t out=0;auto digit=[](char c)->unsigned{if(c>='0'&&c<='9')return unsigned(c-'0');if(c>='a'&&c<='f')return unsigned(c-'a'+10);throw Failure();};
  for(std::size_t i=0;i<text.size();){if(i==8||i==13||i==18||i==23){check(text[i]=='-');i++;continue;}check(i+1<text.size()&&out<result.size());result[out++]=std::uint8_t(digit(text[i])*16+digit(text[i+1]));i+=2;}check(out==16&&std::any_of(result.begin(),result.end(),[](auto b){return b!=0;}));return result;
}
std::size_t body_limit(Kind kind,Direction direction){
  if(direction==Direction::ActorToHost){switch(kind){case Kind::Video:return 32u+kMaxFrame;case Kind::Audio:return 16u+kMaxAudioFrames*4u;case Kind::State:case Kind::Rumble:return 8;case Kind::Heartbeat:case Kind::Stop:return 0;default:throw Failure();}}
  switch(kind){case Kind::Ready:return 24;case Kind::Input:return kInputBytes;case Kind::Metrics:return 32;default:throw Failure();}
}
void validate(const Message& message,Direction direction){
  check(message.sequence>0&&message.sequence<=kMaxSequence&&message.epoch>0&&message.epoch<=kMaxSequence&&message.body.size()<=body_limit(message.kind,direction));const auto b=std::span<const std::uint8_t>(message.body);
  switch(message.kind){
    case Kind::Video:{check(b.size()>=32);const auto w=u32(b,0),h=u32(b,4),pitch=u32(b,8);check(w>0&&w<=kMaxDimension&&h>0&&h<=kMaxDimension&&pitch==w*4&&u32(b,12)==1&&u32(b,16)>0&&u32(b,16)<=kMaxDimension*10&&u32(b,20)==0&&u64(b,24)<=kMaxSequence&&b.size()==32u+std::size_t(pitch)*h);break;}
    case Kind::Audio:{check(b.size()>=16);const auto rate=u32(b,0),frames=u32(b,4);check(rate>=8000&&rate<=192000&&frames>0&&frames<=kMaxAudioFrames&&u16(b,8)==2&&u16(b,10)==1&&u32(b,12)==0&&b.size()==16u+std::size_t(frames)*4);break;}
    case Kind::State:check(b.size()==8&&b[0]<=1&&b[1]<=2&&u16(b,2)<=100&&u32(b,4)==0);break;
    case Kind::Rumble:check(b.size()==8&&b[0]<4&&b[1]==0&&u16(b,6)<=250);break;
    case Kind::Heartbeat:case Kind::Stop:check(b.empty());break;
    case Kind::Ready:check(b.size()==24&&u32(b,0)==kProtocol&&u32(b,4)>0&&u32(b,4)<=kMaxDimension&&u32(b,8)>0&&u32(b,8)<=kMaxDimension&&u32(b,12)==192000&&u32(b,16)==kMaxAudioFrames&&u32(b,20)<=1);break;
    case Kind::Input:{check(b.size()==kInputBytes);for(unsigned port=0;port<4;port++){const auto at=64u+port*20u;check(b[at]<=1&&b[at+1]<=1&&b[at+2]==0&&b[at+3]==0&&(u32(b,at+4)&0xf8000000u)==0);if(!b[at])check(std::all_of(b.begin()+at+1,b.begin()+at+20,[](auto v){return v==0;}));}check(b[144]<=1&&b[145]<=1&&b[146]<=1&&b[147]==0&&(!b[146]||b[145]));if(!b[144])check(std::all_of(b.begin(),b.end(),[](auto v){return v==0;}));break;}
    case Kind::Metrics:check(b.size()==32&&u32(b,24)<=4&&u32(b,28)<=1);break;
    default:throw Failure();
  }
}
bool Duplex::initialize(){
  if(input_||output_)return false;
#ifdef _WIN32
  if(_setmode(_fileno(stdin),_O_BINARY)==-1||_setmode(_fileno(stdout),_O_BINARY)==-1)return false;const auto in=_dup(_fileno(stdin)),out=_dup(_fileno(stdout));if(in<0||out<0)return false;if(!SetHandleInformation(reinterpret_cast<HANDLE>(_get_osfhandle(in)),HANDLE_FLAG_INHERIT,0)||!SetHandleInformation(reinterpret_cast<HANDLE>(_get_osfhandle(out)),HANDLE_FLAG_INHERIT,0))return false;input_=_fdopen(in,"rb");output_=_fdopen(out,"wb");if(!input_||!output_||!std::freopen("NUL","rb",stdin)||!std::freopen("NUL","wb",stdout)||!std::freopen("NUL","wb",stderr))return false;SetStdHandle(STD_INPUT_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stdin))));SetStdHandle(STD_OUTPUT_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stdout))));SetStdHandle(STD_ERROR_HANDLE,reinterpret_cast<HANDLE>(_get_osfhandle(_fileno(stderr))));
#else
  if(std::signal(SIGPIPE,SIG_IGN)==SIG_ERR)return false;const auto in=dup(fileno(stdin)),out=dup(fileno(stdout));if(in<0||out<0||fcntl(in,F_SETFD,FD_CLOEXEC)<0||fcntl(out,F_SETFD,FD_CLOEXEC)<0)return false;input_=fdopen(in,"rb");output_=fdopen(out,"wb");if(!input_||!output_||!std::freopen("/dev/null","rb",stdin)||!std::freopen("/dev/null","wb",stdout)||!std::freopen("/dev/null","wb",stderr))return false;
#endif
  return true;
}
bool Duplex::initialize_inherited(std::uintptr_t input,std::uintptr_t output){
  if(input_||output_||input==output)return false;
#ifdef _WIN32
  const auto source_in=reinterpret_cast<HANDLE>(input),source_out=reinterpret_cast<HANDLE>(output);if(!source_in||!source_out||GetFileType(source_in)!=FILE_TYPE_PIPE||GetFileType(source_out)!=FILE_TYPE_PIPE)return false;HANDLE in=nullptr,out=nullptr;const auto process=GetCurrentProcess();if(!DuplicateHandle(process,source_in,process,&in,0,FALSE,DUPLICATE_SAME_ACCESS))return false;if(!DuplicateHandle(process,source_out,process,&out,0,FALSE,DUPLICATE_SAME_ACCESS)){CloseHandle(in);return false;}
  const int in_fd=_open_osfhandle(reinterpret_cast<intptr_t>(in),_O_RDONLY|_O_BINARY|_O_NOINHERIT),out_fd=_open_osfhandle(reinterpret_cast<intptr_t>(out),_O_WRONLY|_O_BINARY|_O_NOINHERIT);if(in_fd<0||out_fd<0){if(in_fd>=0)_close(in_fd);else CloseHandle(in);if(out_fd>=0)_close(out_fd);else CloseHandle(out);return false;}input_=_fdopen(in_fd,"rb");output_=_fdopen(out_fd,"wb");if(!input_)_close(in_fd);if(!output_)_close(out_fd);if(!input_||!output_)return false;CloseHandle(source_in);CloseHandle(source_out);
#else
  if(input<3||output<3||input>1048575||output>1048575||std::signal(SIGPIPE,SIG_IGN)==SIG_ERR)return false;const auto source_in=static_cast<int>(input),source_out=static_cast<int>(output);struct stat in_info{},out_info{};if(fstat(source_in,&in_info)<0||fstat(source_out,&out_info)<0||!S_ISFIFO(in_info.st_mode)||!S_ISFIFO(out_info.st_mode)||(fcntl(source_in,F_GETFL)&O_ACCMODE)!=O_RDONLY||(fcntl(source_out,F_GETFL)&O_ACCMODE)!=O_WRONLY)return false;
  const auto in=fcntl(source_in,F_DUPFD_CLOEXEC,3);if(in<0)return false;const auto out=fcntl(source_out,F_DUPFD_CLOEXEC,3);if(out<0){close(in);return false;}input_=fdopen(in,"rb");output_=fdopen(out,"wb");if(!input_)close(in);if(!output_)close(out);if(!input_||!output_)return false;close(source_in);close(source_out);
#endif
  return true;
}
bool Duplex::read(Message& message,Direction direction){
  std::array<std::uint8_t,kHeaderBytes> h{};const auto count=std::fread(h.data(),1,h.size(),input_);if(count==0&&std::feof(input_))return false;check(count==h.size()&&std::equal(magic.begin(),magic.end(),h.begin())&&u16(h,8)==kProtocol&&u32(h,12)==0&&std::equal(session_.begin(),session_.end(),h.begin()+16)&&u32(h,52)==0);message.kind=Kind(u16(h,10));message.sequence=u64(h,32);message.epoch=u64(h,40);const auto size=u32(h,48);check(message.sequence>received_&&message.sequence<=kMaxSequence&&message.epoch>0&&message.epoch<=kMaxSequence&&size<=body_limit(message.kind,direction)&&(message.kind!=Kind::Video||size<=32+video_bytes_));message.body.resize(size);check(!size||std::fread(message.body.data(),1,size,input_)==size);validate(message,direction);received_=message.sequence;return true;
}
void Duplex::write(Kind kind,std::uint64_t epoch,std::span<const std::uint8_t> body,Direction direction){
  check(sent_<kMaxSequence&&body.size()<=body_limit(kind,direction)&&(kind!=Kind::Video||body.size()<=32+video_bytes_));Message message{kind,sent_+1,epoch,Bytes(body.begin(),body.end())};validate(message,direction);std::array<std::uint8_t,kHeaderBytes> h{};std::copy(magic.begin(),magic.end(),h.begin());put16(h,8,kProtocol);put16(h,10,std::uint16_t(kind));std::copy(session_.begin(),session_.end(),h.begin()+16);put64(h,32,message.sequence);put64(h,40,epoch);put32(h,48,std::uint32_t(body.size()));check(std::fwrite(h.data(),1,h.size(),output_)==h.size()&&(body.empty()||std::fwrite(body.data(),1,body.size(),output_)==body.size())&&std::fflush(output_)==0);sent_=message.sequence;
}
}
