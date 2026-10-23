// Authored fixture only, not executed. Creates media-type dictionaries, never
// instantiates a codec or accesses a capture/device/driver.
#include "vlikeon_decode_type.hpp"
#include <wrl/client.h>
#include <stdexcept>
using Microsoft::WRL::ComPtr;
using vlikeon::video::detail::decode_type;
namespace {
void require(bool value){if(!value)throw std::runtime_error("decoder media-type fixture failed");}
void ok(HRESULT status){require(SUCCEEDED(status));}
ComPtr<IMFMediaType> type(UINT32 height=1080){
  ComPtr<IMFMediaType> result;ok(MFCreateMediaType(&result));
  ok(result->SetGUID(MF_MT_MAJOR_TYPE,MFMediaType_Video));ok(result->SetGUID(MF_MT_SUBTYPE,MFVideoFormat_NV12));
  ok(MFSetAttributeSize(result.Get(),MF_MT_FRAME_SIZE,1920,height));
  ok(MFSetAttributeRatio(result.Get(),MF_MT_PIXEL_ASPECT_RATIO,1,1));
  ok(result->SetUINT32(MF_MT_DEFAULT_STRIDE,1920));ok(result->SetUINT32(MF_MT_INTERLACE_MODE,MFVideoInterlace_Progressive));
  ok(result->SetUINT32(MF_MT_VIDEO_PRIMARIES,MFVideoPrimaries_BT709));
  ok(result->SetUINT32(MF_MT_TRANSFER_FUNCTION,MFVideoTransFunc_709));
  ok(result->SetUINT32(MF_MT_YUV_MATRIX,MFVideoTransferMatrix_BT709));
  ok(result->SetUINT32(MF_MT_VIDEO_NOMINAL_RANGE,MFNominalRange_16_235));
  ok(result->SetUINT32(MF_MT_VIDEO_CHROMA_SITING,MFVideoChromaSubsampling_MPEG1));return result;
}
void aperture(IMFMediaType* type,const GUID& key,LONG x=0){
  MFVideoArea area{};area.OffsetX.value=static_cast<short>(x);area.Area={1920,1080};
  ok(type->SetBlob(key,reinterpret_cast<const UINT8*>(&area),sizeof(area)));
}
bool admitted(IMFMediaType* value,bool actual=true){uint32_t height=0,stride=0;return SUCCEEDED(decode_type(value,1920,1080,actual,height,stride));}
void cases(){
  require(admitted(type().Get())); // Exact geometry/colour.
  {auto value=type(1088);require(!admitted(value.Get()));aperture(value.Get(),MF_MT_MINIMUM_DISPLAY_APERTURE);require(admitted(value.Get()));}
  for(const GUID* key:{&MF_MT_MINIMUM_DISPLAY_APERTURE,&MF_MT_GEOMETRIC_APERTURE,&MF_MT_PAN_SCAN_APERTURE}){
    auto value=type();aperture(value.Get(),*key,1);require(!admitted(value.Get()));
  }
  {auto value=type();aperture(value.Get(),MF_MT_MINIMUM_DISPLAY_APERTURE);aperture(value.Get(),MF_MT_GEOMETRIC_APERTURE,1);require(!admitted(value.Get()));}
  {auto value=type();uint8_t truncated=0;ok(value->SetBlob(MF_MT_MINIMUM_DISPLAY_APERTURE,&truncated,1));require(!admitted(value.Get()));}
  {auto value=type();ok(value->DeleteItem(MF_MT_TRANSFER_FUNCTION));require(admitted(value.Get(),false));require(!admitted(value.Get(),true));}
  {auto value=type();ok(value->SetUINT32(MF_MT_VIDEO_NOMINAL_RANGE,MFNominalRange_0_255));require(!admitted(value.Get(),false));}
  {auto value=type();ok(MFSetAttributeRatio(value.Get(),MF_MT_PIXEL_ASPECT_RATIO,4,3));require(!admitted(value.Get()));}
  for(const GUID* key:{&MF_MT_PAN_SCAN_ENABLED,&MF_MT_VIDEO_ROTATION,&MF_MT_VIDEO_3D}){
    auto value=type();ok(value->SetUINT32(*key,1));require(!admitted(value.Get()));
  }
  {auto value=type();ok(value->SetUINT32(MF_MT_DEFAULT_STRIDE,1921));require(!admitted(value.Get()));}
  {auto value=type();aperture(value.Get(),MF_MT_MINIMUM_DISPLAY_APERTURE,1);uint32_t height=7,stride=9;
    require(FAILED(decode_type(value.Get(),1920,1080,true,height,stride)));require(height==7&&stride==9);}
}
}
int main(){
  HRESULT initialized=CoInitializeEx(nullptr,COINIT_MULTITHREADED);if(FAILED(initialized))return 2;
  int result=0;try{cases();}catch(...){result=1;}CoUninitialize();return result;
}
