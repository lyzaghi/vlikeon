#include "vlikeon_capture.hpp"
#include <d3d11.h>
#include <d3d11_4.h>
#include <dxgi1_6.h>
#include <dwmapi.h>
#include <mferror.h>
#include <roapi.h>
#include <windows.graphics.capture.interop.h>
#include <windows.graphics.directx.direct3d11.interop.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.Capture.h>
#include <winrt/Windows.Graphics.DirectX.Direct3D11.h>
#include <wrl/client.h>
#include <atomic>
#include <chrono>
#include <cstring>
#include <deque>
#include <mutex>
#include <optional>
#include <string>
#include <utility>

using Microsoft::WRL::ComPtr;
namespace capture=winrt::Windows::Graphics::Capture;
namespace graphics=winrt::Windows::Graphics;
namespace directx=winrt::Windows::Graphics::DirectX;
namespace vlikeon::video {
namespace {
struct OwnedFrame{
  capture::Direct3D11CaptureFrame value{nullptr};
  ULONGLONG received;
  std::shared_ptr<std::atomic<bool>> close_fault;
  OwnedFrame(capture::Direct3D11CaptureFrame frame,std::shared_ptr<std::atomic<bool>> fault):value(std::move(frame)),received(GetTickCount64()),close_fault(std::move(fault)){}
  OwnedFrame(const OwnedFrame&)=delete;
  OwnedFrame& operator=(const OwnedFrame&)=delete;
  OwnedFrame(OwnedFrame&& other)noexcept:value(std::exchange(other.value,nullptr)),received(other.received),close_fault(std::move(other.close_fault)){}
  ~OwnedFrame(){if(value){try{value.Close();}catch(...){close_fault->store(true);}}}
};
struct Source{
  HANDLE process=nullptr;DWORD pid=0;HWND window=nullptr;HMONITOR monitor=nullptr;
  ULONGLONG deadline=0;graphics::SizeInt32 size{};
  std::function<bool()> host_alive;
  std::atomic<bool> closed{false};
  std::shared_ptr<std::atomic<bool>> close_fault=std::make_shared<std::atomic<bool>>(false);
  std::mutex mutex;std::deque<OwnedFrame> frames;
  bool live()const{
    DWORD actual=0;DWORD cloaked=0;
    return !closed.load()&&!close_fault->load()&&host_alive&&host_alive()&&GetTickCount64()<deadline&&
        WaitForSingleObject(process,0)==WAIT_TIMEOUT&&IsWindow(window)&&IsWindowVisible(window)&&!IsIconic(window)&&
        GetAncestor(window,GA_ROOT)==window&&GetWindowThreadProcessId(window,&actual)&&actual==pid&&
        MonitorFromWindow(window,MONITOR_DEFAULTTONULL)==monitor&&
        SUCCEEDED(DwmGetWindowAttribute(window,DWMWA_CLOAKED,&cloaked,sizeof(cloaked)))&&!cloaked;
  }
  void fence(){closed.store(true);std::lock_guard lock(mutex);frames.clear();}
  ~Source(){frames.clear();if(process)CloseHandle(process);}
};
bool bounded_size(graphics::SizeInt32 size){return size.Width>0&&size.Height>0&&size.Width<=8192&&size.Height<=8192&&uint64_t(size.Width)*size.Height<=16*1024*1024;}
HRESULT exception_status(){try{throw;}catch(const winrt::hresult_error& error){return error.code();}catch(const std::bad_alloc&){return E_OUTOFMEMORY;}catch(...){return E_FAIL;}}
std::wstring window_label(HWND window){
  wchar_t text[121]{};int length=GetWindowTextW(window,text,121);
  std::wstring label=length>0?std::wstring(text,static_cast<size_t>(length)):L"Game window";
  for(auto& character:label)if(character<32||character==127)character=L' ';
  return label;
}
}
struct WindowCapture::Impl{
  DWORD thread=GetCurrentThreadId();bool ro=false,frame_event=false,closed_event=false;
  CaptureConfiguration config{};std::shared_ptr<Source> source;
  capture::GraphicsCaptureItem item{nullptr};capture::Direct3D11CaptureFramePool pool{nullptr};
  capture::GraphicsCaptureSession session{nullptr};
  directx::Direct3D11::IDirect3DDevice wrapped{nullptr};
  winrt::event_token frame_token{},closed_token{};
  ComPtr<IDXGIOutput6> monitor_output;
  ComPtr<ID3D11Device> device;ComPtr<ID3D11DeviceContext> context;
  ComPtr<ID3D11VideoDevice> video;ComPtr<ID3D11VideoContext> video_context;
  ComPtr<ID3D11VideoProcessorEnumerator> enumerator;ComPtr<ID3D11VideoProcessor> processor;
  ComPtr<ID3D11Texture2D> scaled,staging;ComPtr<ID3D11VideoProcessorOutputView> output_view;
  std::optional<int64_t> origin,last_pts;
  std::optional<std::chrono::steady_clock::time_point> last_delivery;
  bool owner()const{return thread==GetCurrentThreadId();}
  bool sdr()const{DXGI_OUTPUT_DESC1 description{};return monitor_output&&SUCCEEDED(monitor_output->GetDesc1(&description))&&
      description.Monitor==source->monitor&&description.ColorSpace==DXGI_COLOR_SPACE_RGB_FULL_G22_NONE_P709;}
  void fence(){if(source)source->fence();}
  HRESULT stop(){
    fence();if(!owner())return RPC_E_WRONG_THREAD;
    try{
      if(frame_event){pool.FrameArrived(frame_token);frame_event=false;}
      if(closed_event){item.Closed(closed_token);closed_event=false;}
      if(session)session.Close();if(pool)pool.Close();
      if(source&&source->close_fault->load())return E_FAIL;
      session=nullptr;pool=nullptr;item=nullptr;wrapped=nullptr;
      output_view.Reset();staging.Reset();scaled.Reset();processor.Reset();enumerator.Reset();
      video_context.Reset();video.Reset();context.Reset();device.Reset();monitor_output.Reset();
      return S_OK;
    }catch(...){return exception_status();}
  }
  ~Impl(){
    stop();session=nullptr;pool=nullptr;item=nullptr;wrapped=nullptr;
    output_view.Reset();staging.Reset();scaled.Reset();processor.Reset();enumerator.Reset();
    video_context.Reset();video.Reset();context.Reset();device.Reset();monitor_output.Reset();source.reset();
    if(ro)RoUninitialize();
  }
  void create_gpu(){
    ComPtr<IDXGIFactory1> factory;winrt::check_hresult(CreateDXGIFactory1(IID_PPV_ARGS(&factory)));
    ComPtr<IDXGIAdapter1> chosen;
    for(UINT a=0;a<64&&!chosen;++a){
      ComPtr<IDXGIAdapter1> adapter;HRESULT result=factory->EnumAdapters1(a,&adapter);
      if(result==DXGI_ERROR_NOT_FOUND)break;winrt::check_hresult(result);
      for(UINT o=0;o<32;++o){
        ComPtr<IDXGIOutput> output;result=adapter->EnumOutputs(o,&output);
        if(result==DXGI_ERROR_NOT_FOUND)break;winrt::check_hresult(result);
        DXGI_OUTPUT_DESC description{};winrt::check_hresult(output->GetDesc(&description));
        if(description.Monitor==source->monitor){winrt::check_hresult(output.As(&monitor_output));chosen=adapter;break;}
      }
    }
    if(!chosen||!sdr())winrt::throw_hresult(DXGI_ERROR_UNSUPPORTED);
    D3D_FEATURE_LEVEL levels[]{D3D_FEATURE_LEVEL_11_1,D3D_FEATURE_LEVEL_11_0};
    winrt::check_hresult(D3D11CreateDevice(chosen.Get(),D3D_DRIVER_TYPE_UNKNOWN,nullptr,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT|D3D11_CREATE_DEVICE_VIDEO_SUPPORT,levels,2,D3D11_SDK_VERSION,&device,nullptr,&context));
    ComPtr<ID3D11Multithread> protected_context;winrt::check_hresult(context.As(&protected_context));
    protected_context->SetMultithreadProtected(TRUE);
    if(!protected_context->GetMultithreadProtected())winrt::throw_hresult(E_FAIL);
    ComPtr<IDXGIDevice> dxgi;winrt::check_hresult(device.As(&dxgi));
    winrt::com_ptr<IInspectable> inspectable;
    winrt::check_hresult(CreateDirect3D11DeviceFromDXGIDevice(dxgi.Get(),inspectable.put()));
    wrapped=inspectable.as<directx::Direct3D11::IDirect3DDevice>();
    winrt::check_hresult(device.As(&video));winrt::check_hresult(context.As(&video_context));
    D3D11_VIDEO_PROCESSOR_CONTENT_DESC content{};
    content.InputFrameFormat=D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE;
    content.InputFrameRate={config.fps,1};content.OutputFrameRate={config.fps,1};
    content.InputWidth=source->size.Width;content.InputHeight=source->size.Height;
    content.OutputWidth=config.width;content.OutputHeight=config.height;content.Usage=D3D11_VIDEO_USAGE_PLAYBACK_NORMAL;
    winrt::check_hresult(video->CreateVideoProcessorEnumerator(&content,&enumerator));
    UINT support=0;winrt::check_hresult(enumerator->CheckVideoProcessorFormat(DXGI_FORMAT_B8G8R8A8_UNORM,&support));
    if((support&(D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT|D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT))!=
        (D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_INPUT|D3D11_VIDEO_PROCESSOR_FORMAT_SUPPORT_OUTPUT))winrt::throw_hresult(DXGI_ERROR_UNSUPPORTED);
    D3D11_VIDEO_PROCESSOR_CAPS caps{};winrt::check_hresult(enumerator->GetVideoProcessorCaps(&caps));
    if(!caps.RateConversionCapsCount)winrt::throw_hresult(DXGI_ERROR_UNSUPPORTED);
    winrt::check_hresult(video->CreateVideoProcessor(enumerator.Get(),0,&processor));
    D3D11_TEXTURE2D_DESC texture{};texture.Width=config.width;texture.Height=config.height;
    texture.MipLevels=1;texture.ArraySize=1;texture.Format=DXGI_FORMAT_B8G8R8A8_UNORM;
    texture.SampleDesc.Count=1;texture.Usage=D3D11_USAGE_DEFAULT;texture.BindFlags=D3D11_BIND_RENDER_TARGET;
    winrt::check_hresult(device->CreateTexture2D(&texture,nullptr,&scaled));
    texture.Usage=D3D11_USAGE_STAGING;texture.BindFlags=0;texture.CPUAccessFlags=D3D11_CPU_ACCESS_READ;
    winrt::check_hresult(device->CreateTexture2D(&texture,nullptr,&staging));
    D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC view{};view.ViewDimension=D3D11_VPOV_DIMENSION_TEXTURE2D;
    winrt::check_hresult(video->CreateVideoProcessorOutputView(scaled.Get(),enumerator.Get(),&view,&output_view));
    RECT full{0,0,static_cast<LONG>(config.width),static_cast<LONG>(config.height)};
    RECT from{0,0,source->size.Width,source->size.Height};
    uint64_t width=config.width,height=uint64_t(source->size.Height)*width/source->size.Width;
    if(height>config.height){height=config.height;width=uint64_t(source->size.Width)*height/source->size.Height;}
    if(!width||!height)winrt::throw_hresult(E_INVALIDARG);
    LONG x=static_cast<LONG>((config.width-width)/2),y=static_cast<LONG>((config.height-height)/2);
    RECT to{x,y,x+static_cast<LONG>(width),y+static_cast<LONG>(height)};
    video_context->VideoProcessorSetOutputTargetRect(processor.Get(),TRUE,&full);
    video_context->VideoProcessorSetStreamSourceRect(processor.Get(),0,TRUE,&from);
    video_context->VideoProcessorSetStreamDestRect(processor.Get(),0,TRUE,&to);
    video_context->VideoProcessorSetStreamFrameFormat(processor.Get(),0,D3D11_VIDEO_FRAME_FORMAT_PROGRESSIVE);
    video_context->VideoProcessorSetStreamAutoProcessingMode(processor.Get(),0,FALSE);
    D3D11_VIDEO_COLOR black{};black.RGBA.A=1.0f;
    video_context->VideoProcessorSetOutputBackgroundColor(processor.Get(),FALSE,&black);
    video_context->VideoProcessorSetOutputAlphaFillMode(processor.Get(),D3D11_VIDEO_PROCESSOR_ALPHA_FILL_MODE_OPAQUE,0);
    D3D11_VIDEO_PROCESSOR_COLOR_SPACE rgb{};rgb.Usage=1;rgb.RGB_Range=0;
    rgb.YCbCr_Matrix=1;rgb.Nominal_Range=D3D11_VIDEO_PROCESSOR_NOMINAL_RANGE_0_255;
    video_context->VideoProcessorSetStreamColorSpace(processor.Get(),0,&rgb);
    video_context->VideoProcessorSetOutputColorSpace(processor.Get(),&rgb);
  }
};
WindowCapture::WindowCapture():impl_(std::make_unique<Impl>()){}
WindowCapture::~WindowCapture()=default;
HRESULT WindowCapture::confirm_and_create(HANDLE process,HWND window,CaptureConfiguration config,std::unique_ptr<WindowCapture>& result,std::function<bool()> host_alive){
  ULONGLONG started=GetTickCount64();
  if(result||!host_alive||!process||process==INVALID_HANDLE_VALUE||!window||
      !((config.width==1280&&config.height==720)||(config.width==1920&&config.height==1080))||
      !(config.fps==30||config.fps==60)||!config.remaining_ms||config.remaining_ms>1800000)return E_INVALIDARG;
  try{
    APTTYPE type;APTTYPEQUALIFIER qualifier;winrt::check_hresult(CoGetApartmentType(&type,&qualifier));
    if(type!=APTTYPE_MTA)return RPC_E_WRONG_THREAD;
    auto instance=std::unique_ptr<WindowCapture>(new WindowCapture());auto& self=*instance->impl_;
    winrt::check_hresult(RoInitialize(RO_INIT_MULTITHREADED));self.ro=true;self.config=config;
    self.source=std::make_shared<Source>();auto source=self.source;source->host_alive=std::move(host_alive);
    source->window=window;source->monitor=MonitorFromWindow(window,MONITOR_DEFAULTTONULL);source->deadline=started+config.remaining_ms;
    if(!DuplicateHandle(GetCurrentProcess(),process,GetCurrentProcess(),&source->process,
        PROCESS_QUERY_LIMITED_INFORMATION|SYNCHRONIZE,FALSE,0))winrt::throw_last_error();
    source->pid=GetProcessId(source->process);
    if(source->pid<=1||source->pid==GetCurrentProcessId()||!source->live()||!capture::GraphicsCaptureSession::IsSupported())return E_ACCESSDENIED;
    // Retain the original native capture item BEFORE the dialog. A Closed event
    // fences it even if Windows reuses the numeric HWND while consent is open.
    auto interop=winrt::get_activation_factory<capture::GraphicsCaptureItem,IGraphicsCaptureItemInterop>();
    winrt::check_hresult(interop->CreateForWindow(window,winrt::guid_of<capture::GraphicsCaptureItem>(),winrt::put_abi(self.item)));
    self.closed_token=self.item.Closed([source](auto const&,auto const&){source->fence();});self.closed_event=true;
    std::wstring question=L"Allow Vlikeon to share this game window?\n\n"+window_label(window)+
        L"\n\nOnly this window is selected. No microphone, camera or system audio is requested. Stop sharing from the host to end capture.";
    if(MessageBoxW(nullptr,question.c_str(),L"Vlikeon - Share game window",MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2|MB_SETFOREGROUND)!=IDYES)return HRESULT_FROM_WIN32(ERROR_CANCELLED);
    if(!source->live())return MF_E_SHUTDOWN;
    source->size=self.item.Size();if(!bounded_size(source->size))return E_INVALIDARG;
    self.create_gpu();
    self.pool=capture::Direct3D11CaptureFramePool::CreateFreeThreaded(self.wrapped,directx::DirectXPixelFormat::B8G8R8A8UIntNormalized,2,source->size);
    self.frame_token=self.pool.FrameArrived([source](auto const& sender,auto const&){
      try{
        auto frame=sender.TryGetNextFrame();if(!frame)return;
        OwnedFrame owned(std::move(frame),source->close_fault);
        auto size=owned.value.ContentSize();
        if(!source->live()||size.Width!=source->size.Width||size.Height!=source->size.Height){source->fence();return;}
        std::lock_guard lock(source->mutex);if(source->closed.load())return;
        if(source->frames.size()==2)source->frames.pop_front();
        source->frames.push_back(std::move(owned));
      }catch(...){source->fence();}
    });self.frame_event=true;
    self.session=self.pool.CreateCaptureSession(self.item);
    // Keep normal OS capture indication. No border-suppression permission/API.
    if(!source->live()||!self.sdr())return MF_E_SHUTDOWN;
    self.session.StartCapture();if(!source->live())return MF_E_SHUTDOWN;
    result=std::move(instance);return S_OK;
  }catch(...){return exception_status();}
}
HRESULT WindowCapture::take(BgraFrame& output){
  auto& self=*impl_;if(!self.owner())return RPC_E_WRONG_THREAD;
  try{
    auto source=self.source;
    if(!source||!source->live()||!self.sdr()){self.fence();return MF_E_SHUTDOWN;}
    auto now=std::chrono::steady_clock::now();
    if(self.last_delivery&&now-*self.last_delivery<std::chrono::nanoseconds((1000000000ULL+self.config.fps-1)/self.config.fps))return S_FALSE;
    std::optional<OwnedFrame> latest;
    {
      std::lock_guard lock(source->mutex);if(source->frames.empty())return S_FALSE;
      latest.emplace(std::move(source->frames.back()));source->frames.clear();
    }
    if(GetTickCount64()-latest->received>100)return S_FALSE;
    ULONGLONG received=latest->received;
    int64_t pts=latest->value.SystemRelativeTime().count();
    if(pts<0||(self.last_pts&&pts<=*self.last_pts)){self.fence();return E_UNEXPECTED;}
    auto access=latest->value.Surface().as<Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
    ComPtr<ID3D11Texture2D> texture;winrt::check_hresult(access->GetInterface(IID_PPV_ARGS(&texture)));
    D3D11_TEXTURE2D_DESC description{};texture->GetDesc(&description);
    if(description.Format!=DXGI_FORMAT_B8G8R8A8_UNORM||description.Width<static_cast<UINT>(source->size.Width)||
        description.Height<static_cast<UINT>(source->size.Height)||description.Width>8192||description.Height>8192||
        uint64_t(description.Width)*description.Height>16*1024*1024||description.ArraySize!=1||description.MipLevels!=1||description.SampleDesc.Count!=1){self.fence();return E_INVALIDARG;}
    ComPtr<ID3D11Device> texture_device;texture->GetDevice(&texture_device);
    if(texture_device.Get()!=self.device.Get()){self.fence();return E_INVALIDARG;}
    D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC view{};view.ViewDimension=D3D11_VPIV_DIMENSION_TEXTURE2D;
    ComPtr<ID3D11VideoProcessorInputView> input_view;
    winrt::check_hresult(self.video->CreateVideoProcessorInputView(texture.Get(),self.enumerator.Get(),&view,&input_view));
    D3D11_VIDEO_PROCESSOR_STREAM stream{};stream.Enable=TRUE;stream.pInputSurface=input_view.Get();
    winrt::check_hresult(self.video_context->VideoProcessorBlt(self.processor.Get(),self.output_view.Get(),0,1,&stream));
    self.context->CopyResource(self.staging.Get(),self.scaled.Get());
    BgraFrame frame{self.config.width,self.config.height,self.config.width*4,0,received,{}};
    frame.bytes.resize(static_cast<size_t>(frame.stride)*frame.height);
    D3D11_MAPPED_SUBRESOURCE mapped{};winrt::check_hresult(self.context->Map(self.staging.Get(),0,D3D11_MAP_READ,0,&mapped));
    if(!mapped.pData||mapped.RowPitch<frame.stride||mapped.RowPitch>frame.stride+65536){self.context->Unmap(self.staging.Get(),0);self.fence();return E_UNEXPECTED;}
    for(size_t row=0;row<frame.height;++row)std::memcpy(frame.bytes.data()+row*frame.stride,
        static_cast<const uint8_t*>(mapped.pData)+row*mapped.RowPitch,frame.stride);
    self.context->Unmap(self.staging.Get(),0);
    if(!source->live()||!self.sdr()||GetTickCount64()-latest->received>100){self.fence();return MF_E_SHUTDOWN;}
    for(size_t alpha=3;alpha<frame.bytes.size();alpha+=4)if(frame.bytes[alpha]!=255){self.fence();return E_UNEXPECTED;}
    latest.reset(); // Check native frame-close failure before transferring bytes.
    if(!source->live()||!self.sdr()||GetTickCount64()-received>100){self.fence();return MF_E_SHUTDOWN;}
    if(!self.origin)self.origin=pts;
    frame.timestamp_us=static_cast<uint64_t>(pts-*self.origin)/10;
    self.last_pts=pts;self.last_delivery=std::chrono::steady_clock::now();
    output=std::move(frame);return S_OK;
  }catch(...){self.fence();return exception_status();}
}
void WindowCapture::close_delivery(){impl_->fence();}
HRESULT WindowCapture::stop(){return impl_->stop();}
}
