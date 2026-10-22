#pragma once
#include <cstdint>
#include <string>

// Deterministic UTF-8, independent of the process locale. Actual scratch path
// admission remains in the MemoryFiles table. Never substitute an OS path.
inline bool om_n64_utf8(const wchar_t* wide,std::string& output,std::size_t maximum_bytes=1024,std::size_t maximum_units=1024){
  if(!wide||!maximum_bytes||maximum_bytes>16384||!maximum_units||maximum_units>4096)return false;std::string value;
  for(std::size_t n=0;n<=maximum_units;++n){
    std::uint32_t unit=static_cast<std::uint32_t>(wide[n]);
    if(!unit){output.swap(value);return true;}
    if(sizeof(wchar_t)==2&&unit>=0xd800&&unit<=0xdbff){
      if(n==maximum_units)return false;const auto low=static_cast<std::uint32_t>(wide[++n]);
      if(low<0xdc00||low>0xdfff)return false;unit=0x10000+((unit-0xd800)<<10)+(low-0xdc00);
    }else if(unit>=0xd800&&unit<=0xdfff)return false;
    if(unit>0x10ffff)return false;
    if(unit<0x80)value.push_back(static_cast<char>(unit));
    else if(unit<0x800){value.push_back(static_cast<char>(0xc0|(unit>>6)));value.push_back(static_cast<char>(0x80|(unit&63)));}
    else if(unit<0x10000){value.push_back(static_cast<char>(0xe0|(unit>>12)));value.push_back(static_cast<char>(0x80|((unit>>6)&63)));value.push_back(static_cast<char>(0x80|(unit&63)));}
    else{value.push_back(static_cast<char>(0xf0|(unit>>18)));value.push_back(static_cast<char>(0x80|((unit>>12)&63)));value.push_back(static_cast<char>(0x80|((unit>>6)&63)));value.push_back(static_cast<char>(0x80|(unit&63)));}
    if(value.size()>maximum_bytes)return false;
  }return false;
}
inline std::string om_n64_cache_path(const wchar_t* directory,const wchar_t* leaf){
  std::string parent,name;if(!om_n64_utf8(directory,parent)||!om_n64_utf8(leaf,name)||name.empty()||name.find_first_of("/\\:")!=std::string::npos)return std::string();
  if(parent.empty())return std::string();if(parent.back()!='/'&&parent.back()!='\\')parent.push_back('/');
  parent+=name;if(parent.size()>1024)return std::string();return parent;
}
