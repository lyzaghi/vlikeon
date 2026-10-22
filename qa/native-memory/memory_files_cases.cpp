// Authored cases only. No compiler, store operation or test has run.
#include "memory_files.hpp"
#include <array>
#include <cstring>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

using omni::MemoryFiles;
static void require(bool okay,const char* message){if(!okay)throw std::runtime_error(message);}
static std::uint64_t create(OmniWritableFilesV1& api,const char* path){std::uint64_t handle=0;require(api.open(api.context,path,OMNI_FILES_READ|OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_EXCLUSIVE,&handle)==0&&handle!=0,"create");return handle;}
static OmniFilesUsageV1 usage(OmniWritableFilesV1& api){OmniFilesUsageV1 value{};require(api.usage(api.context,&value)==0,"usage");return value;}
static std::vector<std::uint8_t> read(OmniWritableFilesV1& api,std::uint64_t handle,std::uint64_t offset,std::size_t size){std::vector<std::uint8_t> value(size);std::uint64_t actual=0;require(api.read(api.context,handle,offset,value.data(),size,&actual)==0&&actual==size,"read");return value;}
static void write(OmniWritableFilesV1& api,std::uint64_t handle,std::uint64_t offset,const std::vector<std::uint8_t>& value){std::uint64_t actual=0;require(api.write(api.context,handle,offset,value.data(),value.size(),&actual)==0&&actual==value.size(),"write");}

static void sparse_rounding_zero_and_live_orphan_debt(){
  MemoryFiles store;auto api=store.interface();auto handle=create(api,"scratch/a");
  write(api,handle,8193,{7,8,9});const auto bytes=read(api,handle,0,8196);
  require(bytes[8193]==7&&bytes[8194]==8&&bytes[8195]==9,"sparse tail");
  for(std::size_t at=0;at<8193;++at)require(bytes[at]==0,"sparse zero fill");
  require(usage(api).scratch_allocated_bytes==12288,"page-rounded payload");
  require(api.resize(api.context,handle,8194)==0&&api.resize(api.context,handle,8196)==0,"shrink regrow");
  require(read(api,handle,8193,3)==std::vector<std::uint8_t>({7,0,0}),"truncated bytes cannot reappear");
  require(api.resize(api.context,handle,64ull*1024*1024)==0,"scratch capacity");
  require(api.remove(api.context,"scratch/a")==0,"unlink");auto replacement=create(api,"scratch/a");
  require(api.resize(api.context,replacement,1)==OMNI_FILES_NO_SPACE,"unlinked open payload retains quota");
  require(api.close(api.context,handle)==0&&api.resize(api.context,replacement,1)==0,"last handle releases orphan debt");
  require(usage(api).scratch_allocated_bytes==4096,"replacement page");
  require(api.close(api.context,replacement)==0,"close replacement");
}

static void path_handles_and_revision_bound_listing(){
  MemoryFiles store;auto api=store.interface();require(api.mkdir(api.context,"save/MD",1)==0,"root");
  for(const char* invalid:{"../save/a","save/../a","save/NUL.txt","C:/a","save/a:stream","save/a.","save//a"}){
    std::uint64_t handle=99;require(api.open(api.context,invalid,OMNI_FILES_WRITE|OMNI_FILES_CREATE,&handle)==OMNI_FILES_INVALID&&handle==0,"path admission");
  }
  auto original=create(api,"save/MD/One");write(api,original,0,{1,2,3});std::uint64_t duplicate=99;
  require(api.open(api.context,"SAVE/md/one",OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_EXCLUSIVE,&duplicate)==OMNI_FILES_EXISTS&&duplicate==0,"case alias");
  require(api.rename(api.context,"save/MD","save/Renamed",0)==0,"directory move");
  require(read(api,original,0,3)==std::vector<std::uint8_t>({1,2,3}),"open handle survives rename");
  std::array<OmniFilesEntryV1,1> entries{};std::uint32_t count=0;std::uint64_t revision=0;
  require(api.list(api.context,"save",0,0,entries.data(),1,&count,&revision)==0&&count==1&&std::string(entries[0].name)=="Renamed","first batch");
  require(api.mkdir(api.context,"save/other",0)==0,"concurrent listing mutation");
  require(api.list(api.context,"save",revision,1,entries.data(),1,&count,&revision)==OMNI_FILES_STALE,"stale listing rejected");
  require(api.close(api.context,original)==0,"close original");
  std::uint64_t reopened=0;require(api.open(api.context,"save/Renamed/One",OMNI_FILES_READ,&reopened)==0&&reopened!=original,"opaque handles never reused");
  std::uint64_t actual=0;const std::uint8_t byte=4;
  require(api.write(api.context,reopened,0,&byte,1,&actual)==OMNI_FILES_DENIED&&actual==0,"readonly handle");
  require(api.write(api.context,reopened,std::numeric_limits<std::uint64_t>::max(),&byte,1,&actual)==OMNI_FILES_DENIED,"rights precede offset");
  require(api.close(api.context,reopened)==0,"close readonly");
}

static void atomic_subtree_replacement_and_save_envelope(){
  MemoryFiles store;auto api=store.interface();require(api.mkdir(api.context,"save/MD",1)==0,"save root");
  auto original=create(api,"save/MD/old");write(api,original,0,{9,8,7});
  const auto before=store.capture("save/MD");const std::vector<std::uint8_t> next{1,3,5,7};
  const std::array<omni::MemoryFileEntry,2> entries{{{"empty",{},true},{"new",next,false}}};
  require(store.replace("save/MD",entries)==OMNI_FILES_BUSY,"live handles block replacement");
  require(store.capture("save/MD")==before,"busy replacement unchanged");require(api.close(api.context,original)==0,"retire old handle");
  const std::array<omni::MemoryFileEntry,2> duplicates{{{"new",next,false},{"NEW",next,false}}};
  require(store.replace("save/MD",duplicates)==OMNI_FILES_EXISTS,"duplicate staging rejection");require(store.capture("save/MD")==before,"rejected staging unchanged");
  auto scratch=create(api,"scratch/retained");write(api,scratch,0,{4,2});require(api.close(api.context,scratch)==0,"close scratch");
  require(store.replace("save/MD",entries)==0,"replace committed");
  OmniFilesStatV1 shape{};require(api.stat(api.context,"save/MD/old",&shape)==OMNI_FILES_NOT_FOUND,"old retired");
  require(api.stat(api.context,"save/MD/new",&shape)==0&&shape.bytes==4&&shape.allocated_bytes==4096,"new payload accounting");
  require(api.stat(api.context,"save/MD/empty",&shape)==0&&shape.kind==OMNI_FILES_DIRECTORY,"empty directory preserved");
  require(api.stat(api.context,"scratch/retained",&shape)==0&&shape.bytes==2,"scratch retained separately");
  const auto captured=store.capture("save/MD");require(captured.size()>12&&!std::memcmp(captured.data(),"OPSF0002",8)&&captured[8]==2,"existing archive identity");
  const std::array<omni::MemoryFileEntry,0> empty{};require(store.replace("save/MD",empty)==0,"empty replacement");
  const auto value=usage(api);require(value.save_allocated_bytes==0&&value.scratch_allocated_bytes==4096,"counter transfer and retirement");
}

static void failed_writes_and_metadata_caps(){
  MemoryFiles store;auto api=store.interface();auto handle=create(api,"scratch/preserved");write(api,handle,0,{6,5,4});
  const auto before=usage(api);std::uint64_t actual=55;const std::uint8_t byte=7;
  require(api.write(api.context,handle,std::numeric_limits<std::uint64_t>::max(),&byte,1,&actual)==OMNI_FILES_NO_SPACE&&actual==0,"overflow refused");
  require(api.write(api.context,handle,0,&byte,OMNI_FILES_MAX_TRANSFER+1,&actual)==OMNI_FILES_INVALID,"oversized transfer");
  require(read(api,handle,0,3)==std::vector<std::uint8_t>({6,5,4})&&usage(api).revision==before.revision,"failed writes unchanged");
  std::vector<std::uint64_t> handles{handle};
  for(unsigned n=1;n<256;++n){std::uint64_t opened=0;require(api.open(api.context,"scratch/preserved",OMNI_FILES_READ,&opened)==0,"handle within cap");handles.push_back(opened);}
  std::uint64_t refused=9;require(api.open(api.context,"scratch/preserved",OMNI_FILES_READ,&refused)==OMNI_FILES_NO_SPACE&&refused==0,"handle cap");
  for(auto item:handles)require(api.close(api.context,item)==0,"close handles");
  for(unsigned n=1;n<4096;++n){const auto name="scratch/f"+std::to_string(n);const auto created=create(api,name.c_str());require(api.close(api.context,created)==0,"close metadata fixture");}
  require(api.open(api.context,"scratch/overflow",OMNI_FILES_WRITE|OMNI_FILES_CREATE,&refused)==OMNI_FILES_NO_SPACE,"file count cap");
}

int main(){try{sparse_rounding_zero_and_live_orphan_debt();path_handles_and_revision_bound_listing();atomic_subtree_replacement_and_save_envelope();failed_writes_and_metadata_caps();std::cout<<"memory-files contract cases passed\n";return 0;}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
