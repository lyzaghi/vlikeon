#pragma once
#include "n64-memory-io.h"
#include <array>
#include <cstring>
#include <memory>
#include <string>
#include <utility>

namespace om_n64_text {
// Read and close before publishing any text to a parser. Failure leaves the
// caller's previous text intact; optional settings cannot accept a partial read.
inline bool read(const char* path,std::size_t maximum,std::string& output)noexcept{
  try{
    if(maximum>4u*1024u*1024u)return false;
    std::unique_ptr<OmniN64File,int(*)(OmniN64File*)> file(om_n64_fopen(path,"rb"),om_n64_fclose);
    if(!file)return false;std::string candidate;std::array<char,4096> block{};
    for(;;){const auto count=om_n64_fread(block.data(),1,block.size(),file.get());
      if(om_n64_ferror(file.get())||count>maximum-candidate.size()||std::memchr(block.data(),0,count))return false;
      candidate.append(block.data(),count);if(count<block.size())break;
    }
    if(om_n64_fclose(file.release())!=0)return false;output=std::move(candidate);return true;
  }catch(...){return false;}
}
}
