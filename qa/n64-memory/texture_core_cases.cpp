// Link the exact prepared adapted GLideNHQ TxCache.cpp, never a cache mock.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "TxCache.h"
#include <algorithm>
#include <atomic>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static OmniWritableFilesV1 base{};
static std::atomic<unsigned> fault{0};
static int renamed(void*,const char* from,const char* to,std::uint32_t replace){return fault==1?OMNI_FILES_NO_SPACE:base.rename(base.context,from,to,replace);}
static int written(void*,std::uint64_t handle,std::uint64_t offset,const void* data,std::uint64_t bytes,std::uint64_t* actual){return base.write(base.context,handle,offset,data,fault==2?std::min<std::uint64_t>(bytes,1):bytes,actual);}
static int fault_read(void*,std::uint64_t handle,std::uint64_t offset,void* data,std::uint64_t bytes,std::uint64_t* actual){if(fault==3){*actual=0;return OMNI_FILES_INTERNAL;}return base.read(base.context,handle,offset,data,bytes,actual);}
static int opened(void*,const char* path,std::uint32_t mode,std::uint64_t* handle){if(fault==4&&mode==OMNI_FILES_READ){*handle=0;return OMNI_FILES_NO_MEMORY;}return base.open(base.context,path,mode,handle);}

struct Runtime {
  omni::MemoryFiles store;OmniWritableFilesV1 api;
  Runtime():api(store.interface()){
    base=api;api.rename=renamed;api.write=written;api.read=fault_read;api.open=opened;
    require(om_n64_files_install(&api)==1,"install actual texture table");fault=0;
  }
  ~Runtime(){fault=0;om_n64_files_install(nullptr);}
  std::vector<std::uint8_t> bytes(const char* path){
    OmniFilesStatV1 shape{};require(base.stat(base.context,path,&shape)==0,"stored cache file");std::uint64_t handle=0,actual=0;
    std::vector<std::uint8_t> value(static_cast<std::size_t>(shape.bytes));require(base.open(base.context,path,OMNI_FILES_READ,&handle)==0,"read cache image");
    require(shape.bytes<=OMNI_FILES_MAX_TRANSFER&&base.read(base.context,handle,0,value.data(),shape.bytes,&actual)==0&&actual==shape.bytes&&base.close(base.context,handle)==0,"complete cache image");return value;
  }
  void replace(const char* path,const std::vector<std::uint8_t>& value){
    std::uint64_t handle=0,actual=0;require(base.open(base.context,path,OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_TRUNCATE,&handle)==0,"corrupt cache fixture");
    require(base.write(base.context,handle,0,value.data(),value.size(),&actual)==0&&actual==value.size()&&base.close(base.context,handle)==0,"publish corrupt fixture");
  }
};
struct Cache:TxCache {
  tx_wstring name;int settings;
  Cache(unsigned options,std::uint64_t limit,const wchar_t* filename):TxCache(options,limit,L"scratch/cache",L"fixture",nullptr),name(filename),settings(static_cast<int>(options)){}
  tx_wstring _getFileName()const override{return name;}
  int _getConfig()const override{return settings;}
  using TxCache::save;using TxCache::load;using TxCache::size;using TxCache::totalSize;using TxCache::clear;
};
static GHQTexInfo info(std::vector<uint8>& value,unsigned format=0x0201){
  GHQTexInfo result;result.data=value.data();result.width=2;result.height=2;result.format=0x8058;
  result.texture_format=0x1908;result.pixel_type=0x1401;result.is_hires_tex=1;result.n64_format_size._formatsize=static_cast<uint16>(format);return result;
}
static void observe(Cache& cache,std::uint64_t key,unsigned format,const std::vector<uint8>& expected){
  GHQTexInfo result;result.width=99;require(cache.get(Checksum(key),N64FormatSize(format&255,format>>8),&result)&&result.width==2&&result.height==2&&
    result.format==0x8058&&result.data&&std::equal(expected.begin(),expected.end(),result.data),"actual cache payload round trip");
}

static void each_actual_cache_mode(unsigned options,const wchar_t* filename,const char* path){
  Runtime runtime;std::vector<uint8> first(16,0x13),second(16,0x91),third(16,0x37);
  auto a=info(first),b=info(second,0x0302),c=info(third,0x0403);
  Cache live(options,1024*1024,filename);require(live.add(Checksum(0x1234),&a)&&live.add(Checksum(0x1234),&b),"same checksum distinct N64 format entries");
  require(!live.add(Checksum(0x1234),&a)&&live.size()==2,"duplicate cannot replace old entry");
  observe(live,0x1234,0x0201,first);observe(live,0x1234,0x0302,second);
  require(live.save(),"complete actual cache save");const auto retained=runtime.bytes(path);
  Cache restored(options,1024*1024,filename);require(restored.load(false)&&restored.size()==2,"load actual serialized cache");
  observe(restored,0x1234,0x0201,first);observe(restored,0x1234,0x0302,second);
  require(live.add(Checksum(0x5678),&c),"stage an additional texture");
  for(unsigned point=1;point<=2;++point){fault=point;require(!live.save(),"cache save propagates publication faults");fault=0;
    require(runtime.bytes(path)==retained,"failed save preserves prior serialized cache");observe(live,0x5678,0x0403,third);
  }
  fault=3;require(!restored.load(false),"cache read failure rejects staged replacement");fault=0;
  require(restored.size()==2,"failed cache load leaves old index");observe(restored,0x1234,0x0201,first);
  if(options&FILE_CACHE_MASK){
    fault=4;require(!live.save(),"reopen fault is distinct from publication");fault=0;
    observe(live,0x1234,0x0201,first);observe(live,0x5678,0x0403,third);
  }
  require(live.save(),"retry full cache publication");Cache latest(options,1024*1024,filename);
  require(latest.load(false)&&latest.size()==3,"complete latest index");observe(latest,0x5678,0x0403,third);
  const auto final=runtime.bytes(path);auto broken=final;broken.pop_back();runtime.replace(path,broken);
  require(!latest.load(false)&&latest.size()==3,"truncated cache cannot replace accepted state");
  // The already accepted file-backed snapshot may share the deliberately
  // mutated test node; only pending memory caches are promised independent of
  // this fixture's out-of-band direct ABI mutation. Restore before observing.
  runtime.replace(path,final);observe(latest,0x1234,0x0201,first);
}

static void exact_lru_and_failed_add(){
  Runtime runtime;Cache cache(0,32,L"lru.htc");std::vector<uint8> bytes(16,0x41);auto value=info(bytes);
  require(cache.add(Checksum(1),&value)&&cache.add(Checksum(2),&value),"fill two-entry LRU");observe(cache,1,0x0201,bytes);
  require(cache.add(Checksum(3),&value)&&cache.size()==2&&cache.totalSize()==32,"evict exact least-recent entry");
  GHQTexInfo missing;require(!cache.get(Checksum(2),N64FormatSize(1,2),&missing),"least-recent entry gone");observe(cache,1,0x0201,bytes);
  value.width=0xffffffffU;require(!cache.add(Checksum(4),&value)&&cache.size()==2,"overflow dimensions leave old cache intact");
  value=info(bytes);value.pixel_type=0x1405;require(!cache.add(Checksum(4),&value),"mismatched GPU upload shape rejected");
  observe(cache,3,0x0201,bytes);observe(cache,1,0x0201,bytes);
}

int main(){try{
  require(TxMemBuf::getInstance()->init(1024,1024),"bounded fixture buffers");
  each_actual_cache_mode(0,L"plain.htc","scratch/cache/plain.htc");
  each_actual_cache_mode(GZ_TEXCACHE,L"packed.htc","scratch/cache/packed.htc");
  each_actual_cache_mode(FILE_TEXCACHE,L"plain.hts","scratch/cache/plain.hts");
  each_actual_cache_mode(FILE_TEXCACHE|GZ_TEXCACHE,L"packed.hts","scratch/cache/packed.hts");
  exact_lru_and_failed_add();TxMemBuf::getInstance()->shutdown();
  std::cout<<"Actual adapted N64 texture cache cases passed\n";return 0;
}catch(const std::exception& error){fault=0;std::cerr<<error.what()<<'\n';return 1;}}
