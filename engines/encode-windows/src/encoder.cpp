#include "vlikeon_encoder.hpp"
#include <mfapi.h>
#include <mferror.h>
#include <codecapi.h>
#include <cstring>
#include <limits>

using Microsoft::WRL::ComPtr;
namespace vlikeon::video {
namespace {
constexpr DWORD max_packet=2*1024*1024;
constexpr HRESULT timed_out=HRESULT_FROM_WIN32(ERROR_TIMEOUT);
#define VLK_TRY(call) do { HRESULT vlk_status_=(call); if(FAILED(vlk_status_)) return vlk_status_; } while(false)
HRESULT set_uint(ICodecAPI* api,const GUID& key,ULONG value){
  VARIANT setting;VariantInit(&setting);setting.vt=VT_UI4;setting.ulVal=value;
  return api->SetValue(&key,&setting);
}
HRESULT set_low_delay(ICodecAPI* api){
  VARIANT setting;VariantInit(&setting);setting.vt=VT_BOOL;setting.boolVal=VARIANT_TRUE;
  VLK_TRY(api->SetValue(&CODECAPI_AVLowLatencyMode,&setting));
  return set_uint(api,CODECAPI_AVEncMPVDefaultBPictureCount,0);
}
HRESULT make_type(const Configuration& c,bool encoded,IMFMediaType** result){
  ComPtr<IMFMediaType> type;VLK_TRY(MFCreateMediaType(&type));
  VLK_TRY(type->SetGUID(MF_MT_MAJOR_TYPE,MFMediaType_Video));
  VLK_TRY(type->SetGUID(MF_MT_SUBTYPE,encoded?MFVideoFormat_H264:MFVideoFormat_NV12));
  VLK_TRY(MFSetAttributeSize(type.Get(),MF_MT_FRAME_SIZE,c.width,c.height));
  VLK_TRY(MFSetAttributeRatio(type.Get(),MF_MT_FRAME_RATE,c.fps,1));
  VLK_TRY(MFSetAttributeRatio(type.Get(),MF_MT_PIXEL_ASPECT_RATIO,1,1));
  VLK_TRY(type->SetUINT32(MF_MT_INTERLACE_MODE,MFVideoInterlace_Progressive));
  if(encoded){
    VLK_TRY(type->SetUINT32(MF_MT_AVG_BITRATE,c.bitrate));
    VLK_TRY(type->SetUINT32(MF_MT_MPEG2_PROFILE,eAVEncH264VProfile_Main));
    VLK_TRY(type->SetUINT32(MF_MT_MPEG2_LEVEL,eAVEncH264VLevel4_2));
  }else{
    VLK_TRY(type->SetUINT32(MF_MT_DEFAULT_STRIDE,c.width));
    VLK_TRY(type->SetUINT32(MF_MT_SAMPLE_SIZE,c.width*c.height*3/2));
    VLK_TRY(type->SetUINT32(MF_MT_FIXED_SIZE_SAMPLES,TRUE));
  }
  return type.CopyTo(result);
}
HRESULT sample_buffer(DWORD size,DWORD alignment,IMFSample** result){
  if(!size||size>max_packet*2||alignment>65536||
      (alignment && (alignment&(alignment-1))))return E_INVALIDARG;
  ComPtr<IMFSample> sample;ComPtr<IMFMediaBuffer> buffer;
  VLK_TRY(MFCreateSample(&sample));
  VLK_TRY(MFCreateAlignedMemoryBuffer(size,alignment?alignment-1:0,&buffer));
  VLK_TRY(sample->AddBuffer(buffer.Get()));return sample.CopyTo(result);
}
}
HRESULT Encoder::create(const Configuration& c,std::unique_ptr<Encoder>& result){
  if(result)return E_INVALIDARG;
  if(!((c.width==1280&&c.height==720)||(c.width==1920&&c.height==1080))||
      !(c.fps==30||c.fps==60)||c.bitrate<500000||c.bitrate>30000000||
      !c.remaining_ms||c.remaining_ms>1800000)return E_INVALIDARG;
  APTTYPE apartment;APTTYPEQUALIFIER qualifier;
  VLK_TRY(CoGetApartmentType(&apartment,&qualifier));
  if(apartment!=APTTYPE_MTA)return RPC_E_WRONG_THREAD;
  auto encoder=std::unique_ptr<Encoder>(new Encoder(c));
  encoder->deadline_=GetTickCount64()+c.remaining_ms;
  VLK_TRY(MFStartup(MF_VERSION,MFSTARTUP_FULL));encoder->mf_started_=true;
  MFT_REGISTER_TYPE_INFO input{MFMediaType_Video,MFVideoFormat_NV12};
  MFT_REGISTER_TYPE_INFO output{MFMediaType_Video,MFVideoFormat_H264};
  IMFActivate** candidates=nullptr;UINT32 count=0;
  UINT32 flags=MFT_ENUM_FLAG_HARDWARE|MFT_ENUM_FLAG_SORTANDFILTER;
  if(!c.require_hardware)flags|=MFT_ENUM_FLAG_SYNCMFT|MFT_ENUM_FLAG_ASYNCMFT;
  HRESULT status=MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER,flags,&input,&output,&candidates,&count);
  if(SUCCEEDED(status)){
    status=MF_E_TOPO_CODEC_NOT_FOUND;
    // Bounded attempts; release every enumerated activation, even those omitted.
    for(UINT32 i=0;i<count&&i<64;++i){
      if(GetTickCount64()>=encoder->deadline_){status=MF_E_SHUTDOWN;break;}
      status=encoder->configure(candidates[i]);
      if(SUCCEEDED(status))break;
      HRESULT stopped=encoder->stop();
      if(FAILED(stopped)){status=stopped;break;}
      // Candidate retry is setup-only, before input/output or external exposure.
      {std::lock_guard lock(encoder->output_mutex_);encoder->closed_=false;}
    }
  }
  if(candidates){for(UINT32 i=0;i<count;++i)if(candidates[i])candidates[i]->Release();CoTaskMemFree(candidates);}
  if(FAILED(status))return status;
  VLK_TRY(encoder->live());result=std::move(encoder);return S_OK;
}
HRESULT Encoder::configure(IMFActivate* activation){
  activation_=activation;
  VLK_TRY(activation_->ActivateObject(IID_PPV_ARGS(&transform_)));
  UINT32 hardware_length=0;
  hardware_=SUCCEEDED(activation_->GetStringLength(MFT_ENUM_HARDWARE_URL_Attribute,&hardware_length))&&hardware_length>0;
  if(config_.require_hardware&&!hardware_)return MF_E_TOPO_CODEC_NOT_FOUND;
  ComPtr<IMFAttributes> attributes;
  VLK_TRY(transform_->GetAttributes(&attributes));
  async_=MFGetAttributeUINT32(attributes.Get(),MF_TRANSFORM_ASYNC,FALSE)!=FALSE;
  if(async_){
    VLK_TRY(attributes->SetUINT32(MF_TRANSFORM_ASYNC_UNLOCK,TRUE));
    VLK_TRY(transform_.As(&events_));
  }
  DWORD inputs=0,outputs=0;VLK_TRY(transform_->GetStreamCount(&inputs,&outputs));
  if(inputs!=1||outputs!=1)return MF_E_INVALIDSTREAMNUMBER;
  HRESULT ids=transform_->GetStreamIDs(1,&input_id_,1,&output_id_);
  if(ids==E_NOTIMPL){input_id_=0;output_id_=0;}else if(FAILED(ids))return ids;
  VLK_TRY(transform_.As(&codec_));VLK_TRY(set_low_delay(codec_.Get()));
  ComPtr<IMFMediaType> input,output;
  VLK_TRY(make_type(config_,true,&output));VLK_TRY(make_type(config_,false,&input));
  VLK_TRY(transform_->SetOutputType(output_id_,output.Get(),0));
  VLK_TRY(transform_->SetInputType(input_id_,input.Get(),0));
  VLK_TRY(transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,0));
  streaming_=true;
  VLK_TRY(transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM,0));
  return S_OK;
}
HRESULT Encoder::live(){
  if(!owner())return RPC_E_WRONG_THREAD;
  std::lock_guard lock(output_mutex_);
  if(closed_||GetTickCount64()>=deadline_){closed_=true;output_.clear();output_bytes_=0;return MF_E_SHUTDOWN;}
  return S_OK;
}
HRESULT Encoder::terminal(HRESULT status){close_delivery();return FAILED(status)?status:E_UNEXPECTED;}
void Encoder::close_delivery(){std::lock_guard lock(output_mutex_);closed_=true;output_.clear();output_bytes_=0;}
HRESULT Encoder::submit(std::span<const uint8_t> bytes,int64_t pts,bool keyframe){
  VLK_TRY(live());
  Clock submitted=GetTickCount64();
  if(!transform_)return terminal(MF_E_SHUTDOWN);
  const DWORD length=config_.width*config_.height*3/2;
  if(bytes.size()!=length||pts<0||pts<=last_input_||pts>std::numeric_limits<int64_t>::max()-10000000)return E_INVALIDARG;
  VLK_TRY(poll());
  if(pending_.size()>=2||(async_&&!input_credits_))return S_FALSE;
  MFT_INPUT_STREAM_INFO requirements{};
  VLK_TRY(transform_->GetInputStreamInfo(input_id_,&requirements));
  DWORD capacity=length>requirements.cbSize?length:requirements.cbSize;
  DWORD alignment=requirements.cbAlignment>16?requirements.cbAlignment:16;
  ComPtr<IMFSample> sample;ComPtr<IMFMediaBuffer> buffer;
  VLK_TRY(sample_buffer(capacity,alignment,&sample));VLK_TRY(sample->GetBufferByIndex(0,&buffer));
  BYTE* target=nullptr;VLK_TRY(buffer->Lock(&target,nullptr,nullptr));
  std::memcpy(target,bytes.data(),length);
  HRESULT unlocked=buffer->Unlock();if(FAILED(unlocked))return terminal(unlocked);
  VLK_TRY(buffer->SetCurrentLength(length));VLK_TRY(sample->SetSampleTime(pts));
  VLK_TRY(sample->SetSampleDuration(10000000/config_.fps));
  bool force=first_||keyframe;
  if(force){HRESULT status=set_uint(codec_.Get(),CODECAPI_AVEncVideoForceKeyFrame,1);if(FAILED(status))return terminal(status);}
  VLK_TRY(live()); // copy/property calls do not extend the original authority.
  if(GetTickCount64()-submitted>100)return terminal(timed_out);
  try{pending_.push_back(Pending{pts,submitted,force});}catch(const std::bad_alloc&){return terminal(E_OUTOFMEMORY);}
  HRESULT status=transform_->ProcessInput(input_id_,sample.Get(),0);
  if(status==MF_E_NOTACCEPTING){pending_.pop_back();return S_FALSE;}
  if(FAILED(status))return terminal(status);
  if(async_)--input_credits_;
  last_input_=pts;first_=false;
  VLK_TRY(live());return poll();
}
HRESULT Encoder::poll(){
  VLK_TRY(live());if(!transform_)return terminal(MF_E_SHUTDOWN);
  if(!pending_.empty()&&GetTickCount64()-pending_.front().received>100)return terminal(timed_out);
  if(!async_){
    for(unsigned i=0;i<8;++i){HRESULT status=read_output();if(status==MF_E_TRANSFORM_NEED_MORE_INPUT)return live();if(FAILED(status))return terminal(status);}
    return terminal(E_UNEXPECTED);
  }
  for(unsigned i=0;i<64;++i){
    ComPtr<IMFMediaEvent> event;
    HRESULT status=events_->GetEvent(MF_EVENT_FLAG_NO_WAIT,&event);
    if(status==MF_E_NO_EVENTS_AVAILABLE)return live();
    if(FAILED(status))return terminal(status);
    MediaEventType type;HRESULT event_status;
    status=event->GetType(&type);if(FAILED(status))return terminal(status);
    status=event->GetStatus(&event_status);if(FAILED(status))return terminal(status);
    if(FAILED(event_status))return terminal(event_status);
    if(type==METransformNeedInput){
      UINT32 id=0;status=event->GetUINT32(MF_EVENT_MFT_INPUT_STREAM_ID,&id);
      if(FAILED(status)||id!=input_id_||input_credits_>=64)return terminal(E_UNEXPECTED);
      ++input_credits_;
    }else if(type==METransformHaveOutput){
      status=read_output();if(FAILED(status))return terminal(status);
    }else{return terminal(E_UNEXPECTED);}
    VLK_TRY(live());
  }
  // Continue on a later tick without a blocking GetEvent or an unbounded loop.
  return S_OK;
}
HRESULT Encoder::read_output(){
  VLK_TRY(live());
  MFT_OUTPUT_STREAM_INFO info{};VLK_TRY(transform_->GetOutputStreamInfo(output_id_,&info));
  ComPtr<IMFSample> allocated;
  if(!(info.dwFlags&MFT_OUTPUT_STREAM_PROVIDES_SAMPLES)){
    if(info.cbSize>max_packet)return E_INVALIDARG;
    VLK_TRY(sample_buffer(info.cbSize?info.cbSize:max_packet,info.cbAlignment,&allocated));
  }
  MFT_OUTPUT_DATA_BUFFER output{};output.dwStreamID=output_id_;output.pSample=allocated.Get();
  DWORD flags=0;HRESULT status=transform_->ProcessOutput(0,1,&output,&flags);
  // Adopt a provider-created sample exactly once, including on failure paths.
  ComPtr<IMFSample> sample;
  if(output.pSample==allocated.Get())sample=allocated;
  else if(output.pSample)sample.Attach(output.pSample);
  if(output.pEvents)output.pEvents->Release();
  if(FAILED(status))return status;
  if(flags||(output.dwStatus&~static_cast<DWORD>(MFT_OUTPUT_DATA_BUFFER_INCOMPLETE))||!sample||pending_.empty())return E_UNEXPECTED;
  LONGLONG pts=0;DWORD size=0;
  VLK_TRY(sample->GetSampleTime(&pts));VLK_TRY(sample->GetTotalLength(&size));
  Pending pending=pending_.front();
  if(pts!=pending.pts||!size||size>max_packet||GetTickCount64()-pending.received>100)return timed_out;
  bool keyframe=MFGetAttributeUINT32(sample.Get(),MFSampleExtension_CleanPoint,FALSE)!=FALSE;
  if(pending.keyframe&&!keyframe)return E_UNEXPECTED;
  ComPtr<IMFMediaBuffer> buffer;VLK_TRY(sample->ConvertToContiguousBuffer(&buffer));
  BYTE* data=nullptr;DWORD current=0;VLK_TRY(buffer->Lock(&data,nullptr,&current));
  Packet packet;packet.timestamp_100ns=pts;packet.keyframe=keyframe;
  try{if(current==size)packet.bytes.assign(data,data+current);}
  catch(const std::bad_alloc&){buffer->Unlock();return E_OUTOFMEMORY;}
  HRESULT unlocked=buffer->Unlock();if(FAILED(unlocked))return unlocked;
  if(current!=size)return E_UNEXPECTED;
  if(keyframe){
    ComPtr<IMFMediaType> type;VLK_TRY(transform_->GetOutputCurrentType(output_id_,&type));
    UINT32 length=0;HRESULT header=type->GetBlobSize(MF_MT_MPEG_SEQUENCE_HEADER,&length);
    if(SUCCEEDED(header)){
      if(length>65536)return E_INVALIDARG;
      try{packet.sequence_header.resize(length);}catch(const std::bad_alloc&){return E_OUTOFMEMORY;}
      UINT32 copied=0;
      VLK_TRY(type->GetBlob(MF_MT_MPEG_SEQUENCE_HEADER,packet.sequence_header.data(),length,&copied));
      if(copied!=length)return E_UNEXPECTED;
    }else if(header!=MF_E_ATTRIBUTENOTFOUND)return header;
  }
  VLK_TRY(live());
  std::lock_guard lock(output_mutex_);
  size_t budget=packet.bytes.size()+packet.sequence_header.size();
  if(closed_||GetTickCount64()>=deadline_||output_.size()>=4||output_bytes_+budget>4*1024*1024){
    closed_=true;output_.clear();output_bytes_=0;return MF_E_SHUTDOWN;
  }
  try{output_.push_back(Ready{std::move(packet),GetTickCount64()});}catch(const std::bad_alloc&){return E_OUTOFMEMORY;}
  pending_.pop_front();output_bytes_+=budget;return S_OK;
}
HRESULT Encoder::take(Packet& packet){
  VLK_TRY(poll());std::lock_guard lock(output_mutex_);
  if(closed_||GetTickCount64()>=deadline_){closed_=true;output_.clear();output_bytes_=0;return MF_E_SHUTDOWN;}
  if(output_.empty())return S_FALSE;
  if(GetTickCount64()-output_.front().received>100){closed_=true;output_.clear();output_bytes_=0;return timed_out;}
  packet=std::move(output_.front().packet);output_.pop_front();
  output_bytes_-=packet.bytes.size()+packet.sequence_header.size();return S_OK;
}
HRESULT Encoder::output_type(IMFMediaType** type){
  if(!type)return E_POINTER;*type=nullptr;VLK_TRY(live());
  return transform_?transform_->GetOutputCurrentType(output_id_,type):MF_E_SHUTDOWN;
}
HRESULT Encoder::stop(){
  close_delivery();if(!owner())return RPC_E_WRONG_THREAD;
  if(transform_&&streaming_){
    VLK_TRY(transform_->ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH,0));
    VLK_TRY(transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM,0));
    VLK_TRY(transform_->ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING,0));
  }
  if(activation_)VLK_TRY(activation_->ShutdownObject());
  codec_.Reset();events_.Reset();transform_.Reset();activation_.Reset();
  pending_.clear();input_credits_=0;async_=false;hardware_=false;streaming_=false;
  return S_OK;
}
Encoder::~Encoder(){
  // Failed stop is never exposed as a successful teardown receipt. The service
  // must retain this object when explicit stop fails; destruction is best effort.
  stop();codec_.Reset();events_.Reset();transform_.Reset();activation_.Reset();
  if(mf_started_)MFShutdown();
}
#undef VLK_TRY
}
