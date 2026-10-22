// Source only. Link against the same pinned zlib implementation as the core.
#include "n64-memory-gzip.h"
#include "n64-memory-io.h"
#include <zlib.h>
#include <algorithm>
#include <array>
#include <cerrno>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <limits>
#include <memory>

struct OmniN64Gzip {
  OmniN64File* file=nullptr;
  z_stream codec{};
  std::array<unsigned char,16384> buffer{};
  std::uint64_t decoded=0;
  int status=Z_OK;
  bool write=false,initialized=false,finished=false;
};
namespace {
constexpr std::uint64_t decoded_limit=256ull*1024*1024;
int fail(OmniN64Gzip* file,int status){
  if(file){if(file->status==Z_OK)file->status=status;om_n64_abort(file->file);}
  errno=status==Z_MEM_ERROR?ENOMEM:status==Z_BUF_ERROR?EOVERFLOW:EIO;return -1;
}
bool mode(const char* raw,bool& write,int& level){
  if(!raw)return false;std::size_t n=0;while(n<=3&&raw[n])++n;
  if(n==1&&raw[0]=='r')return true;
  if(n==2&&std::memcmp(raw,"rb",2)==0)return true;
  if(n==1&&raw[0]=='w'){write=true;return true;}
  if((n==2||n==3)&&raw[0]=='w'&&raw[1]=='b'){
    if(n==3){if(raw[2]<'0'||raw[2]>'9')return false;level=raw[2]-'0';}write=true;return true;
  }return false;
}
bool emit(OmniN64Gzip* file,std::size_t count){
  if(om_n64_fwrite(file->buffer.data(),1,count,file->file)!=count){fail(file,Z_ERRNO);return false;}return true;
}
bool finish_input(OmniN64Gzip* file){
  if(file->codec.avail_in||om_n64_fgetc(file->file)!=EOF||om_n64_ferror(file->file)){fail(file,Z_DATA_ERROR);return false;}
  file->finished=true;return true;
}
}
extern "C" OmniN64GzFile om_n64_gzopen(const char* name,const char* requested){
  bool write=false;int level=Z_DEFAULT_COMPRESSION;
  if(!mode(requested,write,level)){errno=EINVAL;return nullptr;}
  std::unique_ptr<OmniN64Gzip> file;
  try{file.reset(new OmniN64Gzip);}catch(...){errno=ENOMEM;return nullptr;}
  file->write=write;file->file=om_n64_fopen(name,write?"wb":"rb");if(!file->file)return nullptr;
  const auto status=write?deflateInit2(&file->codec,level,Z_DEFLATED,MAX_WBITS+16,8,Z_DEFAULT_STRATEGY):inflateInit2(&file->codec,MAX_WBITS+16);
  if(status!=Z_OK){fail(file.get(),status);om_n64_fclose(file->file);return nullptr;}
  file->initialized=true;return file.release();
}
extern "C" int om_n64_gzread(OmniN64GzFile file,void* data,unsigned bytes){
  if(!file||file->write||(!data&&bytes)||bytes>static_cast<unsigned>(std::numeric_limits<int>::max()))return fail(file,Z_STREAM_ERROR);
  if(file->status!=Z_OK)return -1;if(!bytes||file->finished)return 0;
  auto* out=static_cast<unsigned char*>(data);unsigned total=0;
  while(total<bytes&&!file->finished){
    if(!file->codec.avail_in){
      const auto got=om_n64_fread(file->buffer.data(),1,file->buffer.size(),file->file);
      if(!got)return fail(file,om_n64_ferror(file->file)?Z_ERRNO:Z_DATA_ERROR);
      file->codec.next_in=file->buffer.data();file->codec.avail_in=static_cast<uInt>(got);
    }
    // At the exact decoded limit a one-byte probe may consume a footer but can
    // never hand an additional decoded byte to the caller.
    unsigned char probe=0;const auto room=decoded_limit-file->decoded;
    const auto amount=static_cast<uInt>(std::min<std::uint64_t>(bytes-total,room?room:1));
    file->codec.next_out=room?out+total:&probe;file->codec.avail_out=amount;
    const auto before=file->codec.avail_in;const auto status=inflate(&file->codec,Z_NO_FLUSH);
    const auto actual=amount-file->codec.avail_out;
    if(actual>room)return fail(file,Z_BUF_ERROR);file->decoded+=actual;total+=actual;
    if(status==Z_STREAM_END){if(!finish_input(file))return -1;break;}
    if(status!=Z_OK&&status!=Z_BUF_ERROR)return fail(file,status);
    if(!actual&&file->codec.avail_in==before)return fail(file,Z_DATA_ERROR);
  }
  return static_cast<int>(total);
}
extern "C" int om_n64_gzwrite(OmniN64GzFile file,const void* data,unsigned bytes){
  if(!file||!file->write||(!data&&bytes)||bytes>static_cast<unsigned>(std::numeric_limits<int>::max())){fail(file,Z_STREAM_ERROR);return 0;}
  if(file->status!=Z_OK||file->finished)return 0;
  if(bytes>decoded_limit-file->decoded){fail(file,Z_BUF_ERROR);return 0;}
  file->codec.next_in=const_cast<Bytef*>(static_cast<const Bytef*>(data));file->codec.avail_in=bytes;
  while(file->codec.avail_in){
    file->codec.next_out=file->buffer.data();file->codec.avail_out=static_cast<uInt>(file->buffer.size());
    const auto before=file->codec.avail_in;const auto status=deflate(&file->codec,Z_NO_FLUSH);
    const auto actual=file->buffer.size()-file->codec.avail_out;
    if(status!=Z_OK){fail(file,status);return 0;}if(!emit(file,actual))return 0;
    if(!actual&&before==file->codec.avail_in){fail(file,Z_STREAM_ERROR);return 0;}
  }
  file->decoded+=bytes;return static_cast<int>(bytes);
}
extern "C" int64_t om_n64_gzseek(OmniN64GzFile file,int64_t offset,int origin){
  if(!file||file->write||file->status!=Z_OK||(origin!=SEEK_CUR&&origin!=SEEK_SET))return fail(file,Z_STREAM_ERROR);
  const auto base=origin==SEEK_CUR?static_cast<int64_t>(file->decoded):int64_t(0);
  if(offset<0||static_cast<std::uint64_t>(offset)>decoded_limit-static_cast<std::uint64_t>(base))return fail(file,Z_STREAM_ERROR);
  const auto target=base+offset;if(target<static_cast<int64_t>(file->decoded))return fail(file,Z_STREAM_ERROR);
  std::array<unsigned char,16384> skipped{};
  while(file->decoded<static_cast<std::uint64_t>(target)){
    const auto bytes=static_cast<unsigned>(std::min<std::uint64_t>(skipped.size(),target-file->decoded));
    if(om_n64_gzread(file,skipped.data(),bytes)!=static_cast<int>(bytes))return fail(file,Z_DATA_ERROR);
  }return static_cast<int64_t>(file->decoded);
}
extern "C" int om_n64_gzeof(OmniN64GzFile file){return file&&file->finished&&file->status==Z_OK;}
extern "C" void om_n64_gzabort(OmniN64GzFile file){if(file)fail(file,Z_STREAM_ERROR);}
extern "C" int om_n64_gzclose(OmniN64GzFile raw){
  if(!raw){errno=EINVAL;return Z_STREAM_ERROR;}std::unique_ptr<OmniN64Gzip> file(raw);
  if(file->initialized){
    if(file->write){
      while(file->status==Z_OK&&!file->finished){
        file->codec.next_in=Z_NULL;file->codec.avail_in=0;file->codec.next_out=file->buffer.data();file->codec.avail_out=static_cast<uInt>(file->buffer.size());
        const auto status=deflate(&file->codec,Z_FINISH);const auto actual=file->buffer.size()-file->codec.avail_out;
        if(status!=Z_OK&&status!=Z_STREAM_END){fail(file.get(),status);break;}
        if(!emit(file.get(),actual))break;file->finished=status==Z_STREAM_END;
        if(!actual&&!file->finished){fail(file.get(),Z_STREAM_ERROR);break;}
      }
      const auto status=deflateEnd(&file->codec);if(status!=Z_OK&&file->status==Z_OK)fail(file.get(),status);
    }else{
      std::array<unsigned char,16384> skipped{};
      while(file->status==Z_OK&&!file->finished)if(om_n64_gzread(file.get(),skipped.data(),static_cast<unsigned>(skipped.size()))<0)break;
      const auto status=inflateEnd(&file->codec);if(status!=Z_OK&&file->status==Z_OK)fail(file.get(),status);
    }
  }
  if(file->status!=Z_OK)om_n64_abort(file->file);
  if(om_n64_fclose(file->file)!=0&&file->status==Z_OK)file->status=Z_ERRNO;
  return file->status;
}
