// Bounded scratch/save stdio for pinned N64 adapters. No host filesystem calls.
#include "n64-memory-io.h"
#include <algorithm>
#include <array>
#include <atomic>
#include <cerrno>
#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <memory>
#include <mutex>
#include <stdexcept>
#include <string>
#include <vector>

struct OmniN64File {
  std::mutex mutex;std::uint64_t handle=0,position=0,length=0,limit=0;
  std::string destination,temporary;bool temporary_owned=false,readable=false,writable=false,error=false,eof=false,append=false;
};
namespace {
OmniWritableFilesV1 files{};std::mutex lifecycle;
std::atomic<std::uint32_t> live{0},live_writers{0};std::atomic<bool> unresolved_close{false};std::uint64_t serial=1;
constexpr std::uint64_t maximum_file=64ull*1024*1024,maximum_save_file=256ull*1024*1024;
constexpr std::uint32_t maximum_streams=64;

std::string path(const char* raw,bool root=false,bool save=false){
  if(!raw)throw std::invalid_argument("path");std::size_t length=0;while(length<=1024&&raw[length])++length;
  if(root&&length==7&&std::memcmp(raw,"scratch",7)==0)return "scratch";
  if(length<(save?6u:9u)||length>1024)throw std::invalid_argument("path");
  std::string value(raw,length);std::replace(value.begin(),value.end(),'\\','/');
  if((save?value.compare(0,5,"save/"):value.compare(0,8,"scratch/"))||value.back()=='/'||value.find(':')!=std::string::npos)throw std::invalid_argument("path");
  // The authoritative store repeats UTF-8, aliases, devices and depth checks.
  for(std::size_t at=0;at<value.size();){const auto end=value.find('/',at);const auto part=value.substr(at,end==std::string::npos?std::string::npos:end-at);
    if(part.empty()||part=="."||part=="..")throw std::invalid_argument("path");if(end==std::string::npos)break;at=end+1;
  }return value;
}
int failure(int code){errno=code==OMNI_FILES_NO_SPACE?ENOSPC:code==OMNI_FILES_NO_MEMORY?ENOMEM:code==OMNI_FILES_NOT_FOUND?ENOENT:code==OMNI_FILES_EXISTS?EEXIST:code==OMNI_FILES_BUSY?EBUSY:code==OMNI_FILES_INVALID?EINVAL:code==OMNI_FILES_DENIED?EACCES:EIO;return -1;}
bool valid_table(const OmniWritableFilesV1* value){return value&&value->version==1&&value->struct_bytes==sizeof(*value)&&value->context&&value->stat&&value->mkdir&&value->open&&value->close&&value->read&&value->write&&value->resize&&value->flush&&value->remove&&value->rename&&value->list&&value->usage;}
bool mode(const char* raw,bool& read,bool& write,bool& append,bool& preserve){
  if(!raw)return false;std::size_t length=0;while(length<=3&&raw[length])++length;if(!length||length>3)return false;const std::string value(raw,length);
  read=value=="r"||value=="rb"||value=="r+"||value=="rb+"||value=="r+b"||value=="w+"||value=="wb+"||value=="w+b"||value=="a+"||value=="ab+"||value=="a+b";
  write=value=="w"||value=="wb"||value=="a"||value=="ab"||value=="r+"||value=="rb+"||value=="r+b"||value=="w+"||value=="wb+"||value=="w+b"||value=="a+"||value=="ab+"||value=="a+b";
  append=write&&value[0]=='a';preserve=write&&(append||value[0]=='r');return read||write;
}
void abandon(OmniN64File& stream){
  if(stream.handle){if(files.close(files.context,stream.handle))unresolved_close=true;stream.handle=0;}
  if(stream.temporary_owned)files.remove(files.context,stream.temporary.c_str());
}
void mark_error(OmniN64File* stream)noexcept{if(!stream)return;try{std::lock_guard<std::mutex> lock(stream->mutex);stream->error=true;}catch(...){}}
int copy_previous(OmniN64File& target,bool required){
  OmniFilesStatV1 shape{};const auto observed=files.stat(files.context,target.destination.c_str(),&shape);
  if(observed==OMNI_FILES_NOT_FOUND)return required?observed:OMNI_FILES_OK;
  if(observed||shape.kind!=OMNI_FILES_REGULAR||shape.bytes>target.limit)return observed?observed:OMNI_FILES_INVALID;
  std::uint64_t source=0;auto result=files.open(files.context,target.destination.c_str(),OMNI_FILES_READ,&source);if(result)return result;if(!source)return OMNI_FILES_INTERNAL;
  std::array<std::uint8_t,16384> buffer{};
  for(std::uint64_t offset=0;offset<shape.bytes;){const auto bytes=std::min<std::uint64_t>(buffer.size(),shape.bytes-offset);std::uint64_t actual=0;
    result=files.read(files.context,source,offset,buffer.data(),bytes,&actual);if(result||actual!=bytes){result=result?result:OMNI_FILES_INTERNAL;break;}
    result=files.write(files.context,target.handle,offset,buffer.data(),bytes,&actual);if(result||actual!=bytes){result=result?result:OMNI_FILES_INTERNAL;break;}offset+=bytes;
  }
  const auto closed=files.close(files.context,source);if(closed)unresolved_close=true;return result?result:closed;
}
}
extern "C" int om_n64_files_install(const OmniWritableFilesV1* value){try{std::lock_guard<std::mutex> lock(lifecycle);if(live.load()!=0||unresolved_close.load())return 0;if(!value){files={};return 1;}if(!valid_table(value))return 0;files=*value;return 1;}catch(...){return 0;}}
extern "C" int om_n64_files_installed(void){try{std::lock_guard<std::mutex> lock(lifecycle);return files.context!=nullptr;}catch(...){return 0;}}
extern "C" int om_n64_files_idle(void){try{std::lock_guard<std::mutex> lock(lifecycle);return live.load()==0&&!unresolved_close.load();}catch(...){return 0;}}
extern "C" int om_n64_files_writes_settled(void){try{std::lock_guard<std::mutex> lock(lifecycle);return live_writers.load()==0&&!unresolved_close.load();}catch(...){return 0;}}
extern "C" int om_n64_mkdir(const char* raw){try{const auto name=path(raw,true);std::lock_guard<std::mutex> lock(lifecycle);if(!files.context||unresolved_close.load()){errno=EPERM;return -1;}const auto result=files.mkdir(files.context,name.c_str(),1);return result?failure(result):0;}catch(const std::bad_alloc&){errno=ENOMEM;return -1;}catch(...){errno=EINVAL;return -1;}}
static OmniN64File* open_memory_file(const char* raw,const char* requested,bool save){
  try{
    const auto name=path(raw,false,save);bool read=false,write=false,append=false,preserve=false;
    if(!mode(requested,read,write,append,preserve)){errno=EINVAL;return nullptr;}
    std::lock_guard<std::mutex> lock(lifecycle);if(!files.context||unresolved_close.load()){errno=EPERM;return nullptr;}if(live.load()>=maximum_streams){errno=EMFILE;return nullptr;}
    std::unique_ptr<OmniN64File> stream(new OmniN64File);stream->destination=name;stream->readable=read;stream->writable=write;stream->append=append;stream->limit=save?maximum_save_file:maximum_file;
    int result=0;
    if(write){
      if(serial==std::numeric_limits<std::uint64_t>::max()){errno=EMFILE;return nullptr;}
      const auto parent=name.substr(0,name.rfind('/'));result=files.mkdir(files.context,parent.c_str(),1);if(result){failure(result);return nullptr;}
      // Temporary spelling is independent of the requested leaf length. The
      // store bounds names and counters; exclusive creation never adopts data.
      stream->temporary=parent+"/.omn64-"+std::to_string(serial++);
      result=files.open(files.context,stream->temporary.c_str(),OMNI_FILES_READ|OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_EXCLUSIVE,&stream->handle);
      if(!result){if(!stream->handle)result=OMNI_FILES_INTERNAL;else stream->temporary_owned=true;}
      if(!result&&preserve)result=copy_previous(*stream,requested[0]=='r');
    }else{result=files.open(files.context,name.c_str(),OMNI_FILES_READ,&stream->handle);if(!result&&!stream->handle)result=OMNI_FILES_INTERNAL;}
    if(!result){OmniFilesStatV1 shape{};result=files.stat(files.context,write?stream->temporary.c_str():name.c_str(),&shape);if(!result&&(shape.kind!=OMNI_FILES_REGULAR||shape.bytes>stream->limit))result=OMNI_FILES_INVALID;if(!result){stream->length=shape.bytes;if(append)stream->position=shape.bytes;}}
    if(result){abandon(*stream);failure(result);return nullptr;}
    live.fetch_add(1);if(write)live_writers.fetch_add(1);return stream.release();
  }catch(const std::bad_alloc&){errno=ENOMEM;return nullptr;}catch(...){errno=EINVAL;return nullptr;}
}
extern "C" OmniN64File* om_n64_fopen(const char* raw,const char* requested){return open_memory_file(raw,requested,false);}
extern "C" OmniN64File* om_n64_save_fopen(const char* raw,const char* requested){return open_memory_file(raw,requested,true);}
extern "C" size_t om_n64_fread(void* output,size_t size,size_t count,OmniN64File* stream){
  if(!stream||(!output&&size&&count)||(size&&count>std::numeric_limits<size_t>::max()/size)){mark_error(stream);errno=EINVAL;return 0;}if(!size||!count)return 0;
  try{std::lock_guard<std::mutex> lock(stream->mutex);if(!stream->readable){stream->error=true;errno=EBADF;return 0;}const auto bytes=size*count;std::uint64_t done=0;
    while(done<bytes){const auto amount=std::min<std::uint64_t>(OMNI_FILES_MAX_TRANSFER,bytes-done);std::uint64_t actual=0;const auto result=files.read(files.context,stream->handle,stream->position,static_cast<std::uint8_t*>(output)+done,amount,&actual);
      if(result||actual>amount){stream->error=true;failure(result?result:OMNI_FILES_INTERNAL);break;}stream->position+=actual;done+=actual;if(actual<amount){stream->eof=true;break;}
    }return static_cast<size_t>(done/size);
  }catch(...){mark_error(stream);errno=EIO;return 0;}
}
extern "C" size_t om_n64_fwrite(const void* input,size_t size,size_t count,OmniN64File* stream){
  if(!stream||(!input&&size&&count)||(size&&count>std::numeric_limits<size_t>::max()/size)){mark_error(stream);errno=EINVAL;return 0;}if(!size||!count)return 0;
  try{std::lock_guard<std::mutex> lock(stream->mutex);if(!stream->writable){stream->error=true;errno=EBADF;return 0;}if(stream->error){errno=EIO;return 0;}const auto bytes=size*count;
    if(stream->append)stream->position=stream->length;
    if(stream->position>stream->limit||bytes>stream->limit-stream->position){stream->error=true;errno=ENOSPC;return 0;}std::uint64_t done=0;
    while(done<bytes){const auto amount=std::min<std::uint64_t>(OMNI_FILES_MAX_TRANSFER,bytes-done);std::uint64_t actual=0;const auto result=files.write(files.context,stream->handle,stream->position,static_cast<const std::uint8_t*>(input)+done,amount,&actual);
      if(result||actual!=amount){stream->error=true;failure(result?result:OMNI_FILES_INTERNAL);break;}stream->position+=actual;stream->length=std::max(stream->length,stream->position);done+=actual;
    }return static_cast<size_t>(done/size);
  }catch(...){mark_error(stream);errno=EIO;return 0;}
}
extern "C" int om_n64_fseek(OmniN64File* stream,long offset,int origin){
  if(!stream){errno=EINVAL;return -1;}
  try{std::lock_guard<std::mutex> lock(stream->mutex);std::uint64_t base=0;
    if(origin==SEEK_CUR)base=stream->position;else if(origin==SEEK_END)base=stream->length;
    else if(origin!=SEEK_SET){stream->error=true;errno=EINVAL;return -1;}
    const auto negative=offset<0;const auto amount=negative?std::uint64_t(-(offset+1))+1:std::uint64_t(offset);
    if((negative&&amount>base)||(!negative&&(base>stream->limit||amount>stream->limit-base))){stream->error=true;errno=EINVAL;return -1;}
    stream->position=negative?base-amount:base+amount;stream->eof=false;return 0;
  }catch(...){mark_error(stream);errno=EIO;return -1;}
}
extern "C" long om_n64_ftell(OmniN64File* stream){if(!stream){errno=EINVAL;return -1;}try{std::lock_guard<std::mutex> lock(stream->mutex);if(stream->position>static_cast<std::uint64_t>(std::numeric_limits<long>::max())){errno=EOVERFLOW;return -1;}return static_cast<long>(stream->position);}catch(...){errno=EIO;return -1;}}
extern "C" void om_n64_rewind(OmniN64File* stream){try{if(om_n64_fseek(stream,0,SEEK_SET)==0){std::lock_guard<std::mutex> lock(stream->mutex);if(!stream->writable)stream->error=false;}}catch(...){mark_error(stream);errno=EIO;}}
extern "C" int om_n64_fflush(OmniN64File* stream){if(!stream){errno=EINVAL;return EOF;}try{std::lock_guard<std::mutex> lock(stream->mutex);if(stream->error){errno=EIO;return EOF;}const auto result=files.flush(files.context,stream->handle);if(result){stream->error=true;return failure(result);}return 0;}catch(...){mark_error(stream);errno=EIO;return EOF;}}
extern "C" int om_n64_ferror(OmniN64File* stream){if(!stream)return 1;try{std::lock_guard<std::mutex> lock(stream->mutex);return stream->error;}catch(...){return 1;}}
extern "C" void om_n64_abort(OmniN64File* stream){mark_error(stream);}
extern "C" int om_n64_feof(OmniN64File* stream){if(!stream)return 0;try{std::lock_guard<std::mutex> lock(stream->mutex);return stream->eof;}catch(...){return 0;}}
extern "C" int om_n64_fgetc(OmniN64File* stream){unsigned char value=0;return om_n64_fread(&value,1,1,stream)==1?value:EOF;}
extern "C" char* om_n64_fgets(char* output,int capacity,OmniN64File* stream){if(!output||capacity<=0||!stream){mark_error(stream);errno=EINVAL;return nullptr;}if(capacity==1){output[0]=0;return output;}int count=0;while(count<capacity-1){const auto value=om_n64_fgetc(stream);if(value==EOF)break;output[count++]=static_cast<char>(value);if(value=='\n')break;}if(!count)return nullptr;output[count]=0;return output;}
extern "C" int om_n64_fputs(const char* value,OmniN64File* stream){if(!value){mark_error(stream);errno=EINVAL;return EOF;}const auto bytes=std::strlen(value);return om_n64_fwrite(value,1,bytes,stream)==bytes?0:EOF;}
extern "C" int om_n64_fprintf(OmniN64File* stream,const char* format,...){
  if(!stream||!format){mark_error(stream);errno=EINVAL;return -1;}va_list args,copy;va_start(args,format);va_copy(copy,args);const auto length=std::vsnprintf(nullptr,0,format,copy);va_end(copy);
  if(length<0||length>1024*1024){va_end(args);mark_error(stream);errno=EOVERFLOW;return -1;}
  std::vector<char> text;
  try{text.resize(static_cast<std::size_t>(length)+1);}catch(...){va_end(args);mark_error(stream);errno=ENOMEM;return -1;}
  const auto actual=std::vsnprintf(text.data(),text.size(),format,args);va_end(args);
  if(actual!=length){mark_error(stream);errno=EIO;return -1;}
  return om_n64_fwrite(text.data(),1,length,stream)==static_cast<size_t>(length)?length:-1;
}
extern "C" int om_n64_fclose(OmniN64File* raw){
  if(!raw){errno=EINVAL;return EOF;}std::unique_ptr<OmniN64File> stream(raw);bool good=false;
  try{std::lock_guard<std::mutex> lock(stream->mutex);good=!stream->error;
    if(stream->writable&&good){const auto result=files.flush(files.context,stream->handle);if(result)good=false;}
    const auto closed=files.close(files.context,stream->handle);stream->handle=0;if(closed){unresolved_close=true;good=false;}
    if(stream->writable&&good){std::lock_guard<std::mutex> publication(lifecycle);good=files.rename(files.context,stream->temporary.c_str(),stream->destination.c_str(),1)==OMNI_FILES_OK;}
    if(stream->writable&&!good)files.remove(files.context,stream->temporary.c_str());
  }catch(...){try{abandon(*stream);}catch(...){}good=false;}
  if(stream->writable)live_writers.fetch_sub(1);live.fetch_sub(1);if(!good){errno=EIO;return EOF;}return 0;
}
