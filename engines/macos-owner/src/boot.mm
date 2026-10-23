#include "boot.hpp"
#include <array>
#include <sys/sysctl.h>
namespace omniplay::macos {
std::optional<std::string> boot_session(){
  std::array<char,37> raw{};size_t bytes=raw.size();
  if(sysctlbyname("kern.bootsessionuuid",raw.data(),&bytes,nullptr,0)!=0||bytes!=raw.size()||raw[36]!=0)return std::nullopt;
  bool nonzero=false;
  for(size_t index=0;index<36;++index){
    char& byte=raw[index];if(byte>='A'&&byte<='F')byte=byte-'A'+'a';
    if(index==8||index==13||index==18||index==23){if(byte!='-')return std::nullopt;}
    else{if(!((byte>='0'&&byte<='9')||(byte>='a'&&byte<='f')))return std::nullopt;nonzero|=byte!='0';}
  }
  if(!nonzero)return std::nullopt;return std::string(raw.data(),36);
}
}
