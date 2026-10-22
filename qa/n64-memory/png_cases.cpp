// Authored actual C encoder/store/codec cases; execute only within item 16.
#include "memory_files.hpp"
#include "n64-memory-png.h"
#include <png.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <cstring>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static OmniWritableFilesV1 underlying{};static std::atomic<unsigned> fault{0};
static int fault_write(void*,std::uint64_t handle,std::uint64_t offset,const void* data,std::uint64_t count,std::uint64_t* actual){return underlying.write(underlying.context,handle,offset,data,fault==1?std::min<std::uint64_t>(count,1):count,actual);}
static int fault_flush(void*,std::uint64_t handle){return fault==2?OMNI_FILES_INTERNAL:underlying.flush(underlying.context,handle);}
static int fault_rename(void*,const char* from,const char* to,std::uint32_t replace){return fault==3?OMNI_FILES_NO_SPACE:underlying.rename(underlying.context,from,to,replace);}
static int fault_close(void*,std::uint64_t handle){return fault==4?OMNI_FILES_INTERNAL:underlying.close(underlying.context,handle);}

struct Case {
  omni::MemoryFiles store;OmniWritableFilesV1 api;
  Case():api(store.interface()){
    underlying=api;api.write=fault_write;api.flush=fault_flush;api.rename=fault_rename;api.close=fault_close;
    require(om_n64_files_install(&api)==1,"install actual PNG scratch store");fault=0;
  }
  ~Case(){fault=0;om_n64_files_install(nullptr);}
  std::vector<unsigned char> bytes(const char* path){
    OmniFilesStatV1 shape{};require(underlying.stat(underlying.context,path,&shape)==0&&shape.bytes<=OMNI_FILES_MAX_TRANSFER,"stored PNG image");
    std::uint64_t handle=0,actual=0;std::vector<unsigned char> result(static_cast<std::size_t>(shape.bytes));
    require(underlying.open(underlying.context,path,OMNI_FILES_READ,&handle)==0&&underlying.read(underlying.context,handle,0,result.data(),shape.bytes,&actual)==0&&actual==shape.bytes&&underlying.close(underlying.context,handle)==0,"complete committed PNG bytes");return result;
  }
  OmniFilesUsageV1 usage(){OmniFilesUsageV1 result{};require(underlying.usage(underlying.context,&result)==0,"actual PNG store usage");return result;}
};
static const std::array<unsigned char,20> rgba={255,0,0,255, 0,255,0,128, 0x91,0x92,0x93,0x94, 0,0,255,255, 255,255,255,0};
static const std::vector<unsigned char> expected_rgba={255,0,0,255,0,255,0,128,0,0,255,255,255,255,255,0};

static bool encode(const char* path,unsigned channels=4,bool flip=false,std::uint32_t codec=OM_N64_PNG_CODEC_LIMIT){
  auto* file=om_n64_fopen(path,"wb");require(file,"open PNG transaction");
  const auto wrote=om_n64_png_encode(file,rgba.data(),rgba.size(),2,2,12,channels,flip?1:0,codec);
  const auto closed=om_n64_fclose(file);return wrote&&closed==0;
}
static std::vector<unsigned char> decode(const std::vector<unsigned char>& bytes){
  png_image image{};image.version=PNG_IMAGE_VERSION;
  require(png_image_begin_read_from_memory(&image,bytes.data(),bytes.size())!=0,"separate read API accepts encoder PNG");
  require(image.width==2&&image.height==2,"PNG dimensions from separate read API");
  image.format=PNG_FORMAT_RGBA;std::vector<unsigned char> result(PNG_IMAGE_SIZE(image));
  const auto okay=png_image_finish_read(&image,nullptr,result.data(),0,nullptr);png_image_free(&image);
  require(okay!=0,"PNG pixels decoded completely");return result;
}

static void exact_pixels_stride_flip_and_atomic_visibility(){
  Case value;const char* path="scratch/png/dump.png";require(encode(path),"RGBA PNG publication");
  const auto retained=value.bytes(path);require(decode(retained)==expected_rgba,"independent RGBA/alpha fixture, row padding omitted");
  auto* output=om_n64_fopen(path,"wb");require(output&&om_n64_png_encode(output,rgba.data(),rgba.size(),2,2,12,4,1,OM_N64_PNG_CODEC_LIMIT)==1,"flipped borrowed stream encode");
  require(value.bytes(path)==retained,"encoder flush does not publish a partial filename");
  require(om_n64_fclose(output)==0,"successful stream close publishes PNG");
  auto flipped=expected_rgba;std::rotate(flipped.begin(),flipped.begin()+8,flipped.end());require(decode(value.bytes(path))==flipped,"bottom-up RGB source orientation preserved");
  const std::array<unsigned char,13> rgb={255,0,0,0,255,0,0x91,0,0,255,255,255,255};
  output=om_n64_fopen(path,"wb");require(output&&om_n64_png_encode(output,rgb.data(),rgb.size(),2,2,7,3,0,OM_N64_PNG_CODEC_LIMIT)==1&&om_n64_fclose(output)==0,"RGB PNG with odd byte stride");
  auto opaque=expected_rgba;opaque[7]=opaque[15]=255;require(decode(value.bytes(path))==opaque,"RGB output retains pixels and implicit opaque alpha");
  require(value.usage().open_handles==0,"PNG image/stream cleanup");
}

static void bad_extent_budget_and_each_publication_fault(){
  Case value;const char* path="scratch/png/retained.png";require(encode(path),"seed retained PNG");const auto retained=value.bytes(path);const auto baseline=value.usage();
  for(unsigned point=0;point<8;++point){
    auto* file=om_n64_fopen(path,"wb");require(file,"bad-input writer");
    const auto bytes=point==0?19u:static_cast<unsigned>(rgba.size());
    const auto width=point==1?0u:point==2?std::numeric_limits<unsigned>::max():2u;
    const auto height=point==3?std::numeric_limits<unsigned>::max():2u;
    const auto stride=point==4?7u:12u;
    const auto channels=point==5?2u:4u;
    const auto flip=point==6?2u:0u;
    const auto budget=point==7?1u:OM_N64_PNG_CODEC_LIMIT;
    require(!om_n64_png_encode(file,rgba.data(),bytes,width,height,stride,channels,flip,budget)&&om_n64_fclose(file)!=0,"bad source/codec budget aborts writer");
    require(value.bytes(path)==retained,"invalid extent/format/codec budget preserves previous PNG");
  }
  for(unsigned point=1;point<=3;++point){fault=point;require(!encode(path),"actual PNG callback/publication fault propagated");fault=0;
    require(value.bytes(path)==retained,"failed PNG transaction preserves prior bytes");
    const auto after=value.usage();require(after.files==baseline.files&&after.open_handles==baseline.open_handles&&after.scratch_allocated_bytes==baseline.scratch_allocated_bytes,"PNG error cleanup retires private stream/pages");
  }
  for(unsigned n=0;n<64;++n)require(!encode(path,4,false,1),"repeat denied codec allocation");
  require(encode(path)&&decode(value.bytes(path))==expected_rgba,"later healthy encoder works after all allocation/I/O failures");
}

static void unknown_close_retains_actor_lifetime(){
  // Intentionally last: an unconfirmed close permanently poisons this bridge.
  Case value;const char* path="scratch/png/retained.png";require(encode(path),"terminal close fixture");const auto retained=value.bytes(path);
  fault=4;require(!encode(path),"failed stream close cannot claim PNG publication");fault=0;
  require(value.bytes(path)==retained&&!om_n64_files_install(nullptr)&&value.usage().open_handles==1,"old PNG and unknown handle debt survive until actor retirement");
}
static void actual_scratch_page_quota(){
  Case value;const char* path="scratch/png/quota.png";require(encode(path),"seed quota PNG");const auto retained=value.bytes(path);
  std::uint64_t filler=0;require(underlying.open(underlying.context,"scratch/png/filler",OMNI_FILES_WRITE|OMNI_FILES_CREATE,&filler)==0&&underlying.resize(underlying.context,filler,64ull*1024*1024-4096)==0,"fill actual remaining scratch pages");
  const auto before=value.usage();require(!encode(path)&&value.bytes(path)==retained,"actual page-quota fault preserves old PNG");
  const auto after=value.usage();require(before.scratch_allocated_bytes==after.scratch_allocated_bytes&&before.files==after.files&&before.open_handles==after.open_handles,"quota rollback retains only original pages/handles");
  require(underlying.close(underlying.context,filler)==0&&underlying.remove(underlying.context,"scratch/png/filler")==0&&encode(path)&&decode(value.bytes(path))==expected_rgba,"PNG retry succeeds after actual quota debt release");
}
int main(){try{exact_pixels_stride_flip_and_atomic_visibility();bad_extent_budget_and_each_publication_fault();actual_scratch_page_quota();unknown_close_retains_actor_lifetime();
  std::cout<<"N64 memory PNG cases passed\n";return 0;
}catch(const std::exception& error){fault=0;std::cerr<<error.what()<<'\n';return 1;}}
