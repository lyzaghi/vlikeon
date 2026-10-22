// Owned persistent storage for the separately prepared N64 candidate.
#include "n64-save-files.h"
#include "n64-memory-io.h"
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <limits>

extern "C" int om_n64_save_path(const char* path){
    return path && std::strncmp(path,"save",4)==0 && (path[4]=='/' || path[4]=='\\');
}
namespace {
int open_error(){return errno==ENOENT?OM_N64_SAVE_MISSING:OM_N64_SAVE_OPEN;}
struct File {
    OmniN64File* value;
    explicit File(const char* path,const char* mode):value(om_n64_save_fopen(path,mode)){}
    File(const File&)=delete;File& operator=(const File&)=delete;
    ~File(){if(value){om_n64_abort(value);om_n64_fclose(value);}}
    bool close(){auto* owned=value;value=nullptr;return om_n64_fclose(owned)==0;}
};
int extent(File& file,size_t& bytes){
    if(om_n64_fseek(file.value,0,SEEK_END)!=0)return OM_N64_SAVE_SIZE;
    const long end=om_n64_ftell(file.value);
    if(end<0 || static_cast<unsigned long>(end)>OM_N64_SAVE_MAX_BYTES || om_n64_fseek(file.value,0,SEEK_SET)!=0)return OM_N64_SAVE_SIZE;
    bytes=static_cast<size_t>(end);return OM_N64_SAVE_OK;
}
}
extern "C" int om_n64_save_size(const char* path,size_t* output){
    if(!output)return OM_N64_SAVE_SIZE;
    File file(path,"rb");if(!file.value)return open_error();
    size_t bytes=0;const int result=extent(file,bytes);if(result)return result;
    if(!file.close())return OM_N64_SAVE_READ;
    *output=bytes;return OM_N64_SAVE_OK;
}
extern "C" int om_n64_save_load(const char* path,void** output,size_t* length){
    if(!output || !length)return OM_N64_SAVE_SIZE;
    File file(path,"rb");if(!file.value)return open_error();
    size_t bytes=0;const int result=extent(file,bytes);if(result)return result;
    void* candidate=std::malloc(bytes?bytes:1);if(!candidate)return OM_N64_SAVE_MEMORY;
    const bool read=om_n64_fread(candidate,1,bytes,file.value)==bytes && !om_n64_ferror(file.value);
    const bool closed=file.close();
    if(!read || !closed){std::free(candidate);return OM_N64_SAVE_READ;}
    *output=candidate;*length=bytes;return OM_N64_SAVE_OK;
}
extern "C" int om_n64_save_read(const char* path,void* output,size_t expected){
    if((!output && expected) || expected>OM_N64_SAVE_MAX_BYTES)return OM_N64_SAVE_SIZE;
    File file(path,"rb");if(!file.value)return open_error();
    size_t bytes=0;const int result=extent(file,bytes);if(result)return result;
    if(bytes!=expected)return OM_N64_SAVE_SIZE;
    void* candidate=std::malloc(bytes?bytes:1);if(!candidate)return OM_N64_SAVE_MEMORY;
    const bool read=om_n64_fread(candidate,1,bytes,file.value)==bytes && !om_n64_ferror(file.value);
    const bool closed=file.close();
    if(!read || !closed){std::free(candidate);return OM_N64_SAVE_READ;}
    if(bytes)std::memcpy(output,candidate,bytes);std::free(candidate);return OM_N64_SAVE_OK;
}
extern "C" int om_n64_save_write(const char* path,const void* data,size_t bytes){
    if((!data && bytes) || bytes>OM_N64_SAVE_MAX_BYTES)return OM_N64_SAVE_SIZE;
    File file(path,"wb");if(!file.value)return open_error();
    if(om_n64_fwrite(data,1,bytes,file.value)!=bytes || om_n64_ferror(file.value))return OM_N64_SAVE_WRITE;
    return file.close()?OM_N64_SAVE_OK:OM_N64_SAVE_WRITE;
}
extern "C" int om_n64_save_patch(const char* path,const void* data,size_t bytes,size_t offset){
    if((!data && bytes) || offset>OM_N64_SAVE_MAX_BYTES || bytes>OM_N64_SAVE_MAX_BYTES-offset || offset>static_cast<size_t>(std::numeric_limits<long>::max()))return OM_N64_SAVE_SIZE;
    File file(path,"rb+");
    if(!file.value){if(errno!=ENOENT)return open_error();file.value=om_n64_save_fopen(path,"wb");if(!file.value)return open_error();}
    if(om_n64_fseek(file.value,static_cast<long>(offset),SEEK_SET)!=0 || om_n64_fwrite(data,1,bytes,file.value)!=bytes || om_n64_ferror(file.value))return OM_N64_SAVE_WRITE;
    return file.close()?OM_N64_SAVE_OK:OM_N64_SAVE_WRITE;
}
