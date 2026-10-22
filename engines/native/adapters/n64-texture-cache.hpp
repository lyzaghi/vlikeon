#pragma once
#include "n64-memory-gzip.h"
#include "n64-memory-io.h"
#include <zlib.h>
#include <algorithm>
#include <array>
#include <cstdint>
#include <limits>
#include <map>
#include <memory>
#include <set>
#include <utility>
#include <vector>

// Pinned GLideNHQ memory-cache wire layout. No graphics API or OS file access.
namespace om_n64_texture {
constexpr std::uint32_t version=0x08000000,compressed=0x80000000;
constexpr std::uint32_t maximum_records=65536,maximum_dimension=16384;
constexpr std::uint64_t maximum_bytes=256ull*1024*1024;
constexpr std::uint64_t maximum_file_bytes=64ull*1024*1024;
struct Shape {
  std::uint32_t width=0,height=0,format=0;
  std::uint16_t texture_format=0,pixel_type=0,n64_format_size=0;
  std::uint8_t hires=0;
};
struct Record {std::uint64_t checksum=0;Shape shape;std::vector<unsigned char> data;};
struct Image {bool legacy=false;std::vector<Record> records;};
using Key=std::pair<std::uint64_t,std::uint16_t>;
using ShapeBytes=std::uint64_t(*)(const Shape&);
inline bool valid_shape(const Shape& shape,ShapeBytes bytes,std::uint64_t& expanded){
  if(!bytes||!shape.width||!shape.height||shape.width>maximum_dimension||shape.height>maximum_dimension||shape.hires>1)return false;
  expanded=bytes(shape);return expanded>0&&expanded<=maximum_bytes;
}
inline bool valid_payload(const Shape& shape,const unsigned char* data,std::size_t count,ShapeBytes bytes){
  std::uint64_t expected=0;if(!data||!count||count>maximum_bytes||!valid_shape(shape,bytes,expected))return false;
  if(!(shape.format&compressed))return count==expected;
  z_stream stream{};if(inflateInit(&stream)!=Z_OK)return false;
  struct End{z_stream* stream;~End(){inflateEnd(stream);}}end{&stream};
  std::array<unsigned char,16384> output{};stream.next_in=const_cast<Bytef*>(data);stream.avail_in=static_cast<uInt>(count);
  std::uint64_t total=0;
  for(;;){stream.next_out=output.data();stream.avail_out=static_cast<uInt>(output.size());const auto before=stream.avail_in;
    const auto status=inflate(&stream,Z_NO_FLUSH);const auto actual=output.size()-stream.avail_out;
    if(actual>expected-total)return false;total+=actual;
    if(status==Z_STREAM_END)return !stream.avail_in&&total==expected;
    if(status!=Z_OK||(!actual&&before==stream.avail_in))return false;
  }
}
class Gzip {
  OmniN64GzFile file_=nullptr;
public:
  explicit Gzip(const char* path,bool write):file_(om_n64_gzopen(path,write?"wb1":"rb")){}
  ~Gzip(){if(file_){om_n64_gzabort(file_);om_n64_gzclose(file_);}}
  Gzip(const Gzip&)=delete;Gzip& operator=(const Gzip&)=delete;
  explicit operator bool()const{return file_!=nullptr;}
  int read(void* data,unsigned count){return om_n64_gzread(file_,data,count);}
  bool bytes(void* data,std::uint32_t count){return read(data,count)==static_cast<int>(count);}
  template<class T>bool field(T& value){return bytes(&value,static_cast<std::uint32_t>(sizeof(value)));}
  bool put(const void* data,std::uint32_t count){return om_n64_gzwrite(file_,data,count)==static_cast<int>(count);}
  template<class T>bool field_out(const T& value){return put(&value,static_cast<std::uint32_t>(sizeof(value)));}
  bool eof()const{return om_n64_gzeof(file_)!=0;}
  bool close(){auto* prior=file_;file_=nullptr;return prior&&om_n64_gzclose(prior)==Z_OK;}
};
template<class Reader>bool shape_in(Reader& input,Shape& shape,bool legacy,std::uint32_t& length){
  return input.field(shape.width)&&input.field(shape.height)&&input.field(shape.format)&&
    input.field(shape.texture_format)&&input.field(shape.pixel_type)&&input.field(shape.hires)&&
    (legacy||input.field(shape.n64_format_size))&&input.field(length);
}
template<class Writer>bool shape_out(Writer& output,const Shape& shape,std::uint32_t length,bool legacy=false){
  return output.field_out(shape.width)&&output.field_out(shape.height)&&output.field_out(shape.format)&&
    output.field_out(shape.texture_format)&&output.field_out(shape.pixel_type)&&output.field_out(shape.hires)&&
    (legacy||output.field_out(shape.n64_format_size))&&output.field_out(length);
}
inline bool read_gzip(const char* path,std::int32_t config,bool force,ShapeBytes bytes,Image& result)noexcept{
  try{
    Gzip input(path,false);if(!input)return false;Image image;std::uint32_t first=0;std::int32_t actual=0;
    if(!input.field(first))return false;image.legacy=first!=version;
    if(image.legacy)actual=static_cast<std::int32_t>(first);else if(!input.field(actual))return false;
    if(actual!=config&&!force)return false;
    std::uint64_t stored=0,expanded=0;std::set<std::pair<std::uint64_t,std::uint16_t>> keys;
    for(;;){Record record;const auto got=input.read(&record.checksum,sizeof(record.checksum));
      if(got==0&&input.eof())break;if(got!=sizeof(record.checksum)||!record.checksum||image.records.size()>=maximum_records)return false;
      std::uint32_t length=0;std::uint64_t decoded=0;
      if(!shape_in(input,record.shape,image.legacy,length)||!length||length>maximum_bytes-stored||
         !valid_shape(record.shape,bytes,decoded)||decoded>maximum_bytes-expanded||
         !keys.emplace(record.checksum,record.shape.n64_format_size).second)return false;
      record.data.resize(length);if(!input.bytes(record.data.data(),length)||!valid_payload(record.shape,record.data.data(),length,bytes))return false;
      stored+=length;expanded+=decoded;image.records.push_back(std::move(record));
    }
    if(!input.close()||image.records.empty())return false;result=std::move(image);return true;
  }catch(...){return false;}
}

class File {
  OmniN64File* file_=nullptr;bool writer_=false;
public:
  File(const char* path,bool writer):file_(om_n64_fopen(path,writer?"w+b":"rb")),writer_(writer){}
  ~File(){if(file_){if(writer_)om_n64_abort(file_);om_n64_fclose(file_);}}
  File(const File&)=delete;File& operator=(const File&)=delete;
  explicit operator bool()const{return file_!=nullptr;}
  bool bytes(void* data,std::uint32_t count){return om_n64_fread(data,1,count,file_)==count&&!om_n64_ferror(file_);}
  template<class T>bool field(T& value){return bytes(&value,static_cast<std::uint32_t>(sizeof(value)));}
  bool put(const void* data,std::uint32_t count){return om_n64_fwrite(data,1,count,file_)==count&&!om_n64_ferror(file_);}
  template<class T>bool field_out(const T& value){return put(&value,static_cast<std::uint32_t>(sizeof(value)));}
  bool seek(std::uint64_t offset){return offset<=maximum_file_bytes&&om_n64_fseek(file_,static_cast<long>(offset),SEEK_SET)==0;}
  std::int64_t tell(){return om_n64_ftell(file_);}
  std::int64_t size(){const auto prior=tell();if(prior<0||om_n64_fseek(file_,0,SEEK_END))return -1;const auto bytes=tell();return seek(static_cast<std::uint64_t>(prior))?bytes:-1;}
  bool healthy()const{return file_&&!om_n64_ferror(file_);}
  bool close(){auto* old=file_;file_=nullptr;return old&&om_n64_fclose(old)==0;}
};
struct IndexEntry {Shape shape;std::uint64_t offset=0;std::uint32_t length=0;};
struct Index {bool legacy=false;std::map<Key,IndexEntry> records;std::uint64_t stored=0;};
inline bool read_index(File& input,std::int32_t config,bool force,ShapeBytes bytes,Index& result)noexcept{
  try{
    const auto total=input.size();if(total<12||static_cast<std::uint64_t>(total)>maximum_file_bytes||!input.seek(0))return false;
    Index image;std::uint32_t first=0;std::int32_t actual=0,count=0;std::int64_t position=0;
    if(!input.field(first))return false;image.legacy=first!=version;
    if(image.legacy)actual=static_cast<std::int32_t>(first);else if(!input.field(actual))return false;
    const std::uint64_t initial=image.legacy?12:16;
    if((actual!=config&&!force)||!input.field(position)||position<static_cast<std::int64_t>(initial)||position>total-4||!input.seek(position)||!input.field(count)||
       count<=0||static_cast<std::uint32_t>(count)>maximum_records||static_cast<std::uint64_t>(total-position-4)!=std::uint64_t(count)*16)return false;
    struct At {Key key;std::uint64_t offset;};std::vector<At> ordered;ordered.reserve(static_cast<std::size_t>(count));std::set<Key> unique;
    for(std::int32_t n=0;n<count;++n){std::uint64_t checksum=0,packed=0;
      if(!input.field(checksum)||!input.field(packed)||!checksum)return false;
      const auto offset=packed&0x0000ffffffffffffull;const auto format=static_cast<std::uint16_t>(packed>>48);
      const Key key(checksum,image.legacy?0:format);
      if(offset<initial||offset>=static_cast<std::uint64_t>(position)||(image.legacy&&format)||!unique.insert(key).second)return false;
      ordered.push_back({key,offset});
    }
    std::sort(ordered.begin(),ordered.end(),[](const At& a,const At& b){return a.offset<b.offset;});
    std::uint64_t expected=initial,expanded=0;std::vector<unsigned char> payload;
    for(const auto& item:ordered){
      if(item.offset!=expected||!input.seek(item.offset))return false;IndexEntry entry;entry.offset=item.offset;
      const auto metadata=image.legacy?21u:23u;std::uint64_t decoded=0;
      if(item.offset>static_cast<std::uint64_t>(position)||metadata>static_cast<std::uint64_t>(position)-item.offset||
         !shape_in(input,entry.shape,image.legacy,entry.length)||!entry.length||
         entry.length>static_cast<std::uint64_t>(position)-item.offset-metadata||
         entry.shape.n64_format_size!=item.key.second||!valid_shape(entry.shape,bytes,decoded)||decoded>maximum_bytes-expanded)return false;
      payload.resize(entry.length);if(!input.bytes(payload.data(),entry.length)||!valid_payload(entry.shape,payload.data(),payload.size(),bytes))return false;
      expected=item.offset+metadata+entry.length;expanded+=decoded;image.stored+=entry.length;image.records.emplace(item.key,entry);
    }
    if(expected!=static_cast<std::uint64_t>(position)||!input.healthy())return false;result=std::move(image);return true;
  }catch(...){return false;}
}
}
