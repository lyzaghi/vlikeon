#include "vlikeon_colour.hpp"
#include <algorithm>
#include <array>
#include <cmath>
#include <new>
#include <utility>

namespace vlikeon::video {
HRESULT to_nv12(const BgraFrame& source,std::vector<uint8_t>& result){
  if(!((source.width==1280&&source.height==720)||(source.width==1920&&source.height==1080))||
      source.stride<source.width*4||source.stride>source.width*4+4096||source.stride%4||
      source.bytes.size()!=uint64_t(source.stride)*source.height)return E_INVALIDARG;
  try{
    static const std::array<double,256> transfer=[] {
      std::array<double,256> values{};
      for(size_t i=0;i<values.size();++i){
        double srgb=static_cast<double>(i)/255.0;
        double linear=srgb<=0.04045?srgb/12.92:std::pow((srgb+0.055)/1.055,2.4);
        values[i]=linear<0.018?4.5*linear:1.099*std::pow(linear,0.45)-0.099;
      }return values;
    }();
    auto quantize=[](double value,double low,double high){return static_cast<uint8_t>(std::clamp(std::round(value),low,high));};
    size_t luma=static_cast<size_t>(source.width)*source.height;
    std::vector<uint8_t> bytes(luma+luma/2);
    for(size_t row=0;row<source.height;row+=2){for(size_t column=0;column<source.width;column+=2){
      double cb=0,cr=0;
      for(size_t dy=0;dy<2;++dy){for(size_t dx=0;dx<2;++dx){
        size_t offset=(row+dy)*source.stride+(column+dx)*4;
        if(source.bytes[offset+3]!=255)return E_INVALIDARG;
        double b=transfer[source.bytes[offset]],g=transfer[source.bytes[offset+1]],r=transfer[source.bytes[offset+2]];
        double y=0.2126*r+0.7152*g+0.0722*b;
        bytes[(row+dy)*source.width+column+dx]=quantize(16+219*y,16,235);
        cb+=(b-y)/1.8556;cr+=(r-y)/1.5748;
      }}
      size_t uv=luma+(row/2)*source.width+column;
      bytes[uv]=quantize(128+224*cb/4,16,240);bytes[uv+1]=quantize(128+224*cr/4,16,240);
    }}
    result=std::move(bytes);return S_OK;
  }catch(const std::bad_alloc&){return E_OUTOFMEMORY;}catch(...){return E_FAIL;}
}
}
