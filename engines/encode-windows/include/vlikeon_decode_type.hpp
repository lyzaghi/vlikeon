#pragma once
#include <mfapi.h>
#include <mferror.h>
#include <cstdint>
#include <initializer_list>

namespace vlikeon::video::detail {
// Metadata admission, not SPS/VUI parsing. Unknown colour may be tolerated only
// for the pre-input placeholder; an actual decoded frame needs explicit fields.
inline HRESULT decode_type(IMFMediaType* type,uint32_t expected_width,uint32_t expected_height,
    bool actual_frame,uint32_t& coded_height,uint32_t& row_stride){
  if(!type)return E_POINTER;
  GUID major{},subtype{};
  if(FAILED(type->GetGUID(MF_MT_MAJOR_TYPE,&major))||major!=MFMediaType_Video||
      FAILED(type->GetGUID(MF_MT_SUBTYPE,&subtype))||subtype!=MFVideoFormat_NV12)return MF_E_INVALIDMEDIATYPE;
  UINT32 width=0,height=0;
  if(FAILED(MFGetAttributeSize(type,MF_MT_FRAME_SIZE,&width,&height))||width!=expected_width||
      height<expected_height||height>((expected_height+15)&~15u)||height%2)return MF_E_INVALIDMEDIATYPE;
  bool aperture=false;
  // Check every present crop, even for an exact-height surface. Never let
  // geometric/minimum/pan-scan metadata silently disagree about valid pixels.
  for(const GUID* key:{&MF_MT_MINIMUM_DISPLAY_APERTURE,&MF_MT_GEOMETRIC_APERTURE,&MF_MT_PAN_SCAN_APERTURE}){
    UINT32 size=0;HRESULT status=type->GetBlobSize(*key,&size);
    if(status==MF_E_ATTRIBUTENOTFOUND)continue;
    if(FAILED(status)||size!=sizeof(MFVideoArea))return MF_E_INVALIDMEDIATYPE;
    MFVideoArea area{};UINT32 copied=0;
    if(FAILED(type->GetBlob(*key,reinterpret_cast<UINT8*>(&area),sizeof(area),&copied))||copied!=sizeof(area)||
        area.OffsetX.value||area.OffsetX.fract||area.OffsetY.value||area.OffsetY.fract||
        area.Area.cx!=static_cast<LONG>(expected_width)||area.Area.cy!=static_cast<LONG>(expected_height))return MF_E_INVALIDMEDIATYPE;
    // A pan-scan rectangle alone does not establish the full valid image area.
    if(*key!=MF_MT_PAN_SCAN_APERTURE)aperture=true;
  }
  if(height!=expected_height&&!aperture)return MF_E_INVALIDMEDIATYPE;
  auto optional_uint=[&](const GUID& key,UINT32 expected,bool required)->bool{
    UINT32 value=0;HRESULT status=type->GetUINT32(key,&value);
    return status==MF_E_ATTRIBUTENOTFOUND?!required:SUCCEEDED(status)&&value==expected;
  };
  if(!optional_uint(MF_MT_PAN_SCAN_ENABLED,FALSE,false)||!optional_uint(MF_MT_VIDEO_ROTATION,0,false)||
      !optional_uint(MF_MT_VIDEO_3D,FALSE,false)||
      !optional_uint(MF_MT_INTERLACE_MODE,MFVideoInterlace_Progressive,true))return MF_E_INVALIDMEDIATYPE;
  UINT32 numerator=0,denominator=0;
  HRESULT aspect=MFGetAttributeRatio(type,MF_MT_PIXEL_ASPECT_RATIO,&numerator,&denominator);
  if((aspect==MF_E_ATTRIBUTENOTFOUND&&actual_frame)||(aspect!=MF_E_ATTRIBUTENOTFOUND&&
      (FAILED(aspect)||!numerator||numerator!=denominator)))return MF_E_INVALIDMEDIATYPE;
  if(!optional_uint(MF_MT_VIDEO_PRIMARIES,MFVideoPrimaries_BT709,actual_frame)||
      !optional_uint(MF_MT_TRANSFER_FUNCTION,MFVideoTransFunc_709,actual_frame)||
      !optional_uint(MF_MT_YUV_MATRIX,MFVideoTransferMatrix_BT709,actual_frame)||
      !optional_uint(MF_MT_VIDEO_NOMINAL_RANGE,MFNominalRange_16_235,actual_frame)||
      !optional_uint(MF_MT_VIDEO_CHROMA_SITING,MFVideoChromaSubsampling_MPEG1,actual_frame))return MF_E_INVALIDMEDIATYPE;
  UINT32 stride=0;
  if(FAILED(type->GetUINT32(MF_MT_DEFAULT_STRIDE,&stride))||stride<width||stride>width+4096||stride%2)return MF_E_INVALIDMEDIATYPE;
  // Assign outputs only on complete admission; failures cannot poison the old
  // geometry retained by the decoder.
  coded_height=height;row_stride=stride;return S_OK;
}
}
