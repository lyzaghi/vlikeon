#include "engine.hpp"
#include <algorithm>
#include <cstring>
namespace omni {
namespace {
constexpr std::size_t limit=2*1024*1024+128;
constexpr std::array<std::uint32_t,13> eeprom_sizes{0,128,128,256,512,1024,2048,4096,8192,8192,16384,32768,65536};
std::uint32_t number(std::span<const std::uint8_t> data,std::size_t at){
  if(at>data.size()||data.size()-at<4)throw Failure("incompatible_save");
  return std::uint32_t(data[at])|(std::uint32_t(data[at+1])<<8)|(std::uint32_t(data[at+2])<<16)|(std::uint32_t(data[at+3])<<24);
}
}
MdSaveContract MdSaveContract::read(void(*describe)(std::uint32_t*)){
  if(!describe)throw Failure("runtime_crashed");MdSaveContract result;describe(result.words.data());
  const auto& w=result.words;
  if(w[0]!=0x3143444du||w[1]!=1||w[2]>1024*1024||w[3]>1024*1024||w[4]>3||w[5]>=eeprom_sizes.size()||
     (w[2]==0)!=(w[4]==0)||(w[4]==1&&(w[2]<2||w[2]%2))||eeprom_sizes[w[5]]!=w[3])throw Failure("runtime_crashed");
  return result;
}
std::vector<SaveFileShape> MdSaveContract::shapes()const{
  std::vector<SaveFileShape> out;
  if(words[3])out.push_back({"save.eeprom",words[3]});
  if(words[2])out.push_back({"save.ram",words[2]});
  return out;
}
std::span<const std::uint8_t> MdSaveContract::files(std::span<const std::uint8_t> data){
  if(data.size()>limit+32)throw Failure("incompatible_save");
  if(data.size()>=8&&!std::memcmp(data.data(),"OPWH0001",8)){
    if(data.size()<44||number(data,8)!=16||std::uint64_t(number(data,12))+32!=data.size()||
       std::memcmp(data.data()+16,"OPRM0001",8)||number(data,24)||number(data,28))throw Failure("incompatible_save");
    data=data.subspan(32);
  }
  if(data.size()<12||data.size()>limit||std::memcmp(data.data(),"OPSF0002",8))throw Failure("incompatible_save");
  return data;
}
void MdSaveContract::validate(std::span<const std::uint8_t> data)const{
  if(data.size()<12||data.size()>limit||std::memcmp(data.data(),"OPSF0002",8)||number(data,8)>2)throw Failure("incompatible_save");
  const auto expected=shapes();if(number(data,8)!=expected.size())throw Failure("incompatible_save");
  std::array<bool,2> seen{};std::size_t at=12;
  for(unsigned index=0;index<number(data,8);++index){
    const auto kind=number(data,at),name_size=number(data,at+4),size=number(data,at+8);at+=12;
    if(kind||name_size>11||at>data.size()||name_size>data.size()-at)throw Failure("incompatible_save");
    const auto found=std::find_if(expected.begin(),expected.end(),[&](const SaveFileShape& entry){return entry.name.size()==name_size&&!std::memcmp(entry.name.data(),data.data()+at,name_size);});
    if(found==expected.end()||size!=found->bytes)throw Failure("incompatible_save");
    const auto slot=static_cast<std::size_t>(found-expected.begin());if(seen[slot])throw Failure("incompatible_save");seen[slot]=true;at+=name_size;
    if(size>data.size()-at)throw Failure("incompatible_save");at+=size;
  }
  if(at!=data.size())throw Failure("incompatible_save");
}
}
