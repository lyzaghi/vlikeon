#include "vlikeon_decoder.hpp"
#include <mfapi.h>
#include <mferror.h>
#include <wmcodecdsp.h>
#include <codecapi.h>
#include <cstring>
#include <new>
#include <utility>

namespace vlikeon::video {
using Microsoft::WRL::ComPtr;
namespace {
#define VLK_DEC_TRY(call) do{HRESULT vlk_status_=(call);if(FAILED(vlk_status_))return vlk_status_;}while(false)
constexpr HRESULT timed_out=HRESULT_FROM_WIN32(ERROR_TIMEOUT);
HRESULT sample(DWORD size,DWORD alignment,IMFSample** result){
  if(!size||size>16*1024*1024||alignment>65536||(alignment&&(alignment&(alignment-1))))return E_INVALIDARG;
  ComPtr<IMFSample> value;ComPtr<IMFMediaBuffer> buffer;
  VLK_DEC_TRY(MFCreateSample(&value));VLK_DEC_TRY(MFCreateAlignedMemoryBuffer(size,alignment?alignment-1:0,&buffer));
  VLK_DEC_TRY(value->AddBuffer(buffer.Get()));return value.CopyTo(result);
}
// S_OK only after actual locked memory extents and every NV12 row are bounded.
HRESULT copy_rows(BYTE* scan,LONG pitch,BYTE* start,DWORD length,uint32_t width,
    uint32_t visible_height,uint32_t coded_height,std::vector<uint8_t>& output){
  if(!scan||!start||pitch<static_cast<LONG>(width)||pitch>static_cast<LONG>(width+4096)||
      length>16*1024*1024)return E_INVALIDARG;
  uintptr_t base=reinterpret_cast<uintptr_t>(start),first=reinterpret_cast<uintptr_t>(scan);
  if(first<base||first-base>length)return E_INVALIDARG;
  size_t offset=static_cast<size_t>(first-base);
  auto row=[&](size_t index,size_t destination)->bool{
    uint64_t source=uint64_t(offset)+uint64_t(index)*static_cast<uint32_t>(pitch);
    if(source>length||width>length-source)return false;
    std::memcpy(output.data()+destination,start+static_cast<size_t>(source),width);return true;
  };
  for(size_t y=0;y<visible_height;++y)if(!row(y,y*width))return E_INVALIDARG;
  size_t luma=static_cast<size_t>(width)*visible_height;
  // UV starts after the coded surface's full Y plane, not the visible crop.
  for(size_t y=0;y<visible_height/2;++y)if(!row(coded_height+y,luma+y*width))return E_INVALIDARG;
  return S_OK;
}
}
HRESULT Decoder::create(DecoderConfiguration c,std::unique_ptr<Decoder>& output){
  if(output||!((c.width==1280&&c.height==720)||(c.width==1920&&c.height==1080))||
      !(c.fps==30||c.fps==60)||!c.remaining_ms||c.remaining_ms>1800000)return E_INVALIDARG;
  APTTYPE apartment;APTTYPEQUALIFIER qualifier;VLK_DEC_TRY(CoGetApartmentType(&apartment,&qualifier));
  if(apartment!=APTTYPE_MTA)return RPC_E_WRONG_THREAD;
  try{
    auto decoder=std::unique_ptr<Decoder>(new Decoder(c));decoder->deadline_=GetTickCount64()+c.remaining_ms;
    VLK_DEC_TRY(MFStartup(MF_VERSION,MFSTARTUP_FULL));decoder->mf_started_=true;
    VLK_DEC_TRY(CoCreateInstance(CLSID_CMSH264DecoderMFT,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&decoder->transform_)));
    DWORD inputs=0,outputs=0;VLK_DEC_TRY(decoder->transform_->GetStreamCount(&inputs,&outputs));
    if(inputs!=1||outputs!=1)return MF_E_INVALIDSTREAMNUMBER;
    DWORD input=0,result=0;HRESULT ids=decoder->transform_->GetStreamIDs(1,&input,1,&result);
    if((FAILED(ids)&&ids!=E_NOTIMPL)||(SUCCEEDED(ids)&&(input||result)))return MF_E_INVALIDSTREAMNUMBER;
    ComPtr<IMFAttributes> attributes;VLK_DEC_TRY(decoder->transform_->GetAttributes(&attributes));
    if(MFGetAttributeUINT32(attributes.Get(),MF_TRANSFORM_ASYNC,FALSE))return E_NOTIMPL;
    // The Microsoft decoder's low-latency property is UINT32, unlike encoders.
    VLK_DEC_TRY(attributes->SetUINT32(CODECAPI_AVLowLatencyMode,TRUE));
    VLK_DEC_TRY(attributes->SetUINT32(CODECAPI_AVDecVideoMaxCodedWidth,c.width));
    VLK_DEC_TRY(attributes->SetUINT32(CODECAPI_AVDecVideoMaxCodedHeight,(c.height+15)&~15u));
    ComPtr<IMFMediaType> type;VLK_DEC_TRY(MFCreateMediaType(&type));
    VLK_DEC_TRY(type->SetGUID(MF_MT_MAJOR_TYPE,MFMediaType_Video));VLK_DEC_TRY(type->SetGUID(MF_MT_SUBTYPE,MFVideoFormat_H264));
    VLK_DEC_TRY(MFSetAttributeSize(type.Get(),MF_MT_FRAME_SIZE,c.width,c.height));
    VLK_DEC_TRY(MFSetAttributeRatio(type.Get(),MF_MT_FRAME_RATE,c.fps,1));
    VLK_DEC_TRY(MFSetAttributeRatio(type.Get(),MF_MT_PIXEL_ASPECT_RATIO,1,1));
    VLK_DEC_TRY(type->SetUINT32(MF_MT_INTERLACE_MODE,MFVideoInterlace_Progressive));
    VLK_DEC_TRY(decoder->transform_->SetInputType(0,type.Get(),0));
    VLK_DEC_TRY(decoder->select_output());
    VLK_DEC_TRY(decoder->transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,0));decoder->streaming_=true;
    VLK_DEC_TRY(decoder->transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM,0));
    VLK_DEC_TRY(decoder->live());output=std::move(decoder);return S_OK;
  }catch(const std::bad_alloc&){return E_OUTOFMEMORY;}catch(...){return E_FAIL;}
}
HRESULT Decoder::select_output(){
  for(DWORD i=0;i<64;++i){
    ComPtr<IMFMediaType> type;HRESULT result=transform_->GetOutputAvailableType(0,i,&type);
    if(result==MF_E_NO_MORE_TYPES)break;VLK_DEC_TRY(result);
    GUID subtype{};VLK_DEC_TRY(type->GetGUID(MF_MT_SUBTYPE,&subtype));if(subtype!=MFVideoFormat_NV12)continue;
    UINT32 width=0,height=0;VLK_DEC_TRY(MFGetAttributeSize(type.Get(),MF_MT_FRAME_SIZE,&width,&height));
    if(width!=config_.width||height<config_.height||height>((config_.height+15)&~15u)||height%2)continue;
    if(height!=config_.height){
      MFVideoArea visible{};UINT32 length=0;
      result=type->GetBlob(MF_MT_MINIMUM_DISPLAY_APERTURE,reinterpret_cast<UINT8*>(&visible),sizeof(visible),&length);
      if(FAILED(result)||length!=sizeof(visible)||visible.OffsetX.value||visible.OffsetX.fract||visible.OffsetY.value||visible.OffsetY.fract||
          visible.Area.cx!=static_cast<LONG>(config_.width)||visible.Area.cy!=static_cast<LONG>(config_.height))continue;
    }
    UINT32 interlace=MFGetAttributeUINT32(type.Get(),MF_MT_INTERLACE_MODE,MFVideoInterlace_Unknown);
    if(interlace!=MFVideoInterlace_Progressive)continue;
    UINT32 stride=MFGetAttributeUINT32(type.Get(),MF_MT_DEFAULT_STRIDE,0);
    if(stride<width||stride>width+4096)continue;
    result=transform_->SetOutputType(0,type.Get(),0);
    if(SUCCEEDED(result)){coded_height_=height;stride_=stride;return S_OK;}
  }return MF_E_INVALIDMEDIATYPE;
}
HRESULT Decoder::live(){
  if(thread_!=GetCurrentThreadId())return RPC_E_WRONG_THREAD;
  std::lock_guard lock(output_mutex_);ULONGLONG now=GetTickCount64();
  if(closed_||now>=deadline_||(!frames_.empty()&&now-frames_.front().received_tick_ms>100)){
    closed_=true;frames_.clear();return MF_E_SHUTDOWN;
  }return S_OK;
}
HRESULT Decoder::terminal(HRESULT result){close_delivery();return FAILED(result)?result:E_UNEXPECTED;}
void Decoder::close_delivery(){std::lock_guard lock(output_mutex_);closed_=true;frames_.clear();}
HRESULT Decoder::submit(std::span<const uint8_t> bytes,int64_t pts,uint64_t received){
  VLK_DEC_TRY(live());ULONGLONG now=GetTickCount64();
  if(!transform_||bytes.empty()||bytes.size()>2*1024*1024||pts<0||pts<=last_input_||pts>18000000000LL||received>now||now-received>100)return terminal(E_INVALIDARG);
  VLK_DEC_TRY(poll());if(pending_.size()>=2)return S_FALSE;
  ComPtr<IMFSample> input;ComPtr<IMFMediaBuffer> buffer;
  VLK_DEC_TRY(sample(static_cast<DWORD>(bytes.size()),16,&input));VLK_DEC_TRY(input->GetBufferByIndex(0,&buffer));
  BYTE* memory=nullptr;DWORD capacity=0;VLK_DEC_TRY(buffer->Lock(&memory,&capacity,nullptr));
  if(!memory||capacity<bytes.size()){buffer->Unlock();return terminal(E_UNEXPECTED);}
  std::memcpy(memory,bytes.data(),bytes.size());
  HRESULT unlocked=buffer->Unlock();if(FAILED(unlocked))return terminal(unlocked);
  VLK_DEC_TRY(buffer->SetCurrentLength(static_cast<DWORD>(bytes.size())));VLK_DEC_TRY(input->SetSampleTime(pts));
  VLK_DEC_TRY(input->SetSampleDuration(10000000/config_.fps));
  VLK_DEC_TRY(live());if(GetTickCount64()-received>100)return terminal(timed_out);
  try{pending_.push_back(Pending{pts,received});}catch(const std::bad_alloc&){return terminal(E_OUTOFMEMORY);}
  HRESULT result=transform_->ProcessInput(0,input.Get(),0);
  if(result==MF_E_NOTACCEPTING){pending_.pop_back();return S_FALSE;}
  if(FAILED(result))return terminal(result);last_input_=pts;return poll();
}
HRESULT Decoder::poll(){
  VLK_DEC_TRY(live());if(!transform_)return terminal(MF_E_SHUTDOWN);
  if(!pending_.empty()&&GetTickCount64()-pending_.front().received>100)return terminal(timed_out);
  for(unsigned i=0;i<8;++i){
    HRESULT result=read_output();
    if(result==MF_E_TRANSFORM_NEED_MORE_INPUT)return live();
    if(result==MF_E_TRANSFORM_STREAM_CHANGE){result=select_output();if(SUCCEEDED(result))continue;}
    if(FAILED(result))return terminal(result);
  }return terminal(E_UNEXPECTED);
}
HRESULT Decoder::read_output(){
  VLK_DEC_TRY(live());MFT_OUTPUT_STREAM_INFO info{};VLK_DEC_TRY(transform_->GetOutputStreamInfo(0,&info));
  ComPtr<IMFSample> allocated;
  if(!(info.dwFlags&MFT_OUTPUT_STREAM_PROVIDES_SAMPLES)){
    if(info.cbSize>16*1024*1024)return E_INVALIDARG;
    DWORD minimum=stride_*(coded_height_+coded_height_/2);
    VLK_DEC_TRY(sample(info.cbSize>minimum?info.cbSize:minimum,info.cbAlignment,&allocated));
  }
  MFT_OUTPUT_DATA_BUFFER output{};output.pSample=allocated.Get();DWORD flags=0;
  HRESULT result=transform_->ProcessOutput(0,1,&output,&flags);
  ComPtr<IMFSample> decoded;if(output.pSample==allocated.Get())decoded=allocated;else if(output.pSample)decoded.Attach(output.pSample);
  if(output.pEvents)output.pEvents->Release();if(FAILED(result))return result;
  if(flags||(output.dwStatus&~static_cast<DWORD>(MFT_OUTPUT_DATA_BUFFER_INCOMPLETE))||!decoded||pending_.empty())return E_UNEXPECTED;
  LONGLONG pts=0;VLK_DEC_TRY(decoded->GetSampleTime(&pts));
  auto pending=pending_.front();
  if(pts!=pending.pts||GetTickCount64()-pending.received>100||MFGetAttributeUINT32(decoded.Get(),MFSampleExtension_Interlaced,FALSE))return E_UNEXPECTED;
  DecodedFrame frame{config_.width,config_.height,config_.width,pts,pending.received,{}};
  VLK_DEC_TRY(copy_frame(decoded.Get(),frame));VLK_DEC_TRY(live());
  std::lock_guard lock(output_mutex_);
  if(closed_||GetTickCount64()>=deadline_||GetTickCount64()-pending.received>100||frames_.size()>=2){closed_=true;frames_.clear();return MF_E_SHUTDOWN;}
  try{frames_.push_back(std::move(frame));}catch(const std::bad_alloc&){return E_OUTOFMEMORY;}
  pending_.pop_front();return S_OK;
}
HRESULT Decoder::copy_frame(IMFSample* decoded,DecodedFrame& frame){
  DWORD count=0;VLK_DEC_TRY(decoded->GetBufferCount(&count));if(count!=1)return E_INVALIDARG;
  ComPtr<IMFMediaBuffer> buffer;VLK_DEC_TRY(decoded->GetBufferByIndex(0,&buffer));
  try{frame.nv12.resize(static_cast<size_t>(frame.width)*frame.height*3/2);}catch(const std::bad_alloc&){return E_OUTOFMEMORY;}
  ComPtr<IMF2DBuffer2> two;HRESULT queried=buffer.As(&two);
  if(SUCCEEDED(queried)){
    BYTE* scan=nullptr;BYTE* base=nullptr;LONG pitch=0;DWORD length=0;
    VLK_DEC_TRY(two->Lock2DSize(MF2DBuffer_LockFlags_Read,&scan,&pitch,&base,&length));
    HRESULT copied=copy_rows(scan,pitch,base,length,frame.width,frame.height,coded_height_,frame.nv12);
    HRESULT unlocked=two->Unlock2D();return FAILED(copied)?copied:unlocked;
  }
  if(queried!=E_NOINTERFACE)return queried;
  BYTE* base=nullptr;DWORD current=0;VLK_DEC_TRY(buffer->Lock(&base,nullptr,&current));
  HRESULT copied=copy_rows(base,static_cast<LONG>(stride_),base,current,frame.width,frame.height,coded_height_,frame.nv12);
  HRESULT unlocked=buffer->Unlock();return FAILED(copied)?copied:unlocked;
}
HRESULT Decoder::take(DecodedFrame& output){
  VLK_DEC_TRY(poll());std::lock_guard lock(output_mutex_);ULONGLONG now=GetTickCount64();
  if(closed_||now>=deadline_){closed_=true;frames_.clear();return MF_E_SHUTDOWN;}
  if(frames_.empty())return S_FALSE;
  if(now-frames_.front().received_tick_ms>100){closed_=true;frames_.clear();return timed_out;}
  output=std::move(frames_.front());frames_.pop_front();return S_OK;
}
HRESULT Decoder::stop(){
  close_delivery();if(thread_!=GetCurrentThreadId())return RPC_E_WRONG_THREAD;
  if(transform_&&streaming_){
    VLK_DEC_TRY(transform_->ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH,0));
    VLK_DEC_TRY(transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM,0));
    VLK_DEC_TRY(transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING,0));
  }
  ComPtr<IMFShutdown> shutdown;
  if(transform_){HRESULT queried=transform_.As(&shutdown);if(SUCCEEDED(queried))VLK_DEC_TRY(shutdown->Shutdown());else if(queried!=E_NOINTERFACE)return queried;}
  transform_.Reset();pending_.clear();streaming_=false;return S_OK;
}
Decoder::~Decoder(){stop();transform_.Reset();if(mf_started_)MFShutdown();}
#undef VLK_DEC_TRY
}
