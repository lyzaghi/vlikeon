#pragma once
#include <algorithm>
#include <cctype>
#include <cstdint>
#include <istream>
#include <set>
#include <string>
#include <utility>
#include <vector>

// The pinned native-endian GLideN64 cache formats, decoded completely before
// any cache byte reaches GL. This header has no OS file or graphics API path.
namespace om_n64_shader {
constexpr std::uint32_t maximum_records=65536,maximum_text=4096;
constexpr std::uint32_t maximum_binary=16u*1024u*1024u;
constexpr std::uint64_t maximum_image=64ull*1024ull*1024ull;
struct Keys {bool legacy_hwl=false,hwl=false;std::vector<std::uint64_t> values;};
struct Record {std::uint64_t mux=0;std::int32_t inputs=0;std::uint32_t format=0;std::vector<char> binary;};
struct Image {std::vector<Record> records;};

inline bool driver_text(const char* text,std::string& result){
  if(!text)return false;std::size_t n=0;while(n<=maximum_text&&text[n])++n;
  if(n>maximum_text)return false;result.assign(text,n);return true;
}
inline bool trailing_text(std::istream& in){
  if(in.fail())return false;if(in.eof())return true;
  for(;;){const auto byte=in.peek();if(byte==std::char_traits<char>::eof())return !in.bad();
    if(!std::isspace(static_cast<unsigned char>(byte)))return false;in.get();}
}
inline bool read_keys(std::istream& in,Keys& result)noexcept{
  try{
    Keys candidate;std::uint32_t version=0,count=0,hwl=0;
    if(!(in>>std::hex>>version)||(version!=4&&version!=5))return false;
    candidate.legacy_hwl=version==4;
    if(candidate.legacy_hwl){if(!(in>>std::hex>>hwl)||hwl>1)return false;candidate.hwl=hwl!=0;}
    if(!(in>>std::hex>>count)||count>maximum_records)return false;
    candidate.values.reserve(count);
    for(std::uint32_t n=0;n<count;++n){std::uint64_t key=0;if(!(in>>std::hex>>key))return false;candidate.values.push_back(key);}
    if(!trailing_text(in))return false;
    auto ordered=candidate.values;std::sort(ordered.begin(),ordered.end());
    if(std::adjacent_find(ordered.begin(),ordered.end())!=ordered.end())return false;
    result=std::move(candidate);return true;
  }catch(...){return false;}
}

class Input {
  std::istream& in_;std::uint64_t remaining_=maximum_image;
public:
  explicit Input(std::istream& in):in_(in){}
  bool bytes(void* data,std::uint32_t count){
    if(count>remaining_)return false;remaining_-=count;
    return count==0||static_cast<bool>(in_.read(static_cast<char*>(data),count));
  }
  template<class T>bool field(T& value){return bytes(&value,static_cast<std::uint32_t>(sizeof(value)));}
  bool text(const std::string& expected){
    std::uint32_t count=0;if(!field(count)||count>maximum_text||count>remaining_||count!=expected.size())return false;
    std::string actual(count,'\0');return bytes(count?&actual[0]:nullptr,count)&&actual==expected;
  }
  bool record(Record& record){
    std::int32_t length=0;
    if(!field(record.mux)||!field(record.inputs)||!field(record.format)||!field(length)||length<=0||
       static_cast<std::uint32_t>(length)>maximum_binary||static_cast<std::uint32_t>(length)>remaining_)return false;
    record.binary.resize(static_cast<std::size_t>(length));return bytes(record.binary.data(),static_cast<std::uint32_t>(length));
  }
};
inline bool read_image(std::istream& in,std::uint32_t format,std::uint32_t options,
                       const std::string& renderer,const std::string& gl_version,Image& result)noexcept{
  try{
    if(renderer.size()>maximum_text||gl_version.size()>maximum_text)return false;
    Input reader(in);std::uint32_t actual=0,count=0;
    if(!reader.field(actual)||actual!=format||!reader.field(actual)||actual!=options||
       !reader.text(renderer)||!reader.text(gl_version)||!reader.field(count)||count>maximum_records)return false;
    Image candidate;candidate.records.reserve(count);std::set<std::uint64_t> keys;
    for(std::uint32_t n=0;n<count;++n){Record record;if(!reader.record(record)||!keys.insert(record.mux).second)return false;candidate.records.push_back(std::move(record));}
    if(in.peek()!=std::char_traits<char>::eof()||in.bad())return false;
    result=std::move(candidate);return true;
  }catch(...){return false;}
}
}
