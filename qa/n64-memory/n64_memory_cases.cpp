// Authored acceptance sources. Never run before the consolidated item-16 pass.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "n64-memory-streams.hpp"
#include "n64-memory-gzip.h"
#include "n64-memory-text.hpp"
#include "n64-shader-cache.hpp"
#include "n64-memory-path.hpp"
#include "n64-texture-cache.hpp"
#include <zlib.h>
#include <algorithm>
#include <array>
#include <atomic>
#include <cerrno>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

static void require(bool okay,const char* why){if(!okay)throw std::runtime_error(why);}
static OmniWritableFilesV1 underlying{};
static std::string collision_path;
static int short_write(void*,std::uint64_t handle,std::uint64_t offset,const void* data,std::uint64_t bytes,std::uint64_t* actual){return underlying.write(underlying.context,handle,offset,data,std::min<std::uint64_t>(bytes,3),actual);}
static int fail_read(void*,std::uint64_t,std::uint64_t,void*,std::uint64_t,std::uint64_t* actual){*actual=0;return OMNI_FILES_INTERNAL;}
static int fail_flush(void*,std::uint64_t){return OMNI_FILES_INTERNAL;}
static int fail_rename(void*,const char*,const char*,std::uint32_t){return OMNI_FILES_NO_SPACE;}
static int uncertain_close(void*,std::uint64_t){return OMNI_FILES_INTERNAL;}
static int collide(void*,const char* path,std::uint32_t mode,std::uint64_t* handle){
  if(mode&OMNI_FILES_EXCLUSIVE){
    collision_path=path;std::uint64_t other=0,actual=0;
    const char marker[]="another owner";
    require(underlying.open(underlying.context,path,mode,&other)==0,"create colliding owner");
    require(underlying.write(underlying.context,other,0,marker,sizeof(marker)-1,&actual)==0&&actual==sizeof(marker)-1,"colliding owner bytes");
    require(underlying.close(underlying.context,other)==0,"close colliding owner");
    *handle=0;return OMNI_FILES_EXISTS;
  }return underlying.open(underlying.context,path,mode,handle);
}

struct Case {
  omni::MemoryFiles store;OmniWritableFilesV1 api;
  Case():api(store.interface()){
    require(api.mkdir(api.context,"scratch/case",1)==0,"scratch directory");
    require(om_n64_files_install(&api)==1&&om_n64_files_installed(),"actual memory-file table");
  }
  ~Case(){om_n64_files_install(nullptr);}
  void raw(const char* path,const std::vector<std::uint8_t>& data){
    std::uint64_t handle=0;require(api.open(api.context,path,OMNI_FILES_READ|OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_TRUNCATE,&handle)==0,"fixture open");
    for(std::size_t at=0;at<data.size();){const auto bytes=std::min<std::size_t>(OMNI_FILES_MAX_TRANSFER,data.size()-at);std::uint64_t actual=0;require(api.write(api.context,handle,at,data.data()+at,bytes,&actual)==0&&actual==bytes,"fixture write");at+=bytes;}
    require(api.close(api.context,handle)==0,"fixture close");
  }
  void raw(const char* path,const std::string& data){raw(path,std::vector<std::uint8_t>(data.begin(),data.end()));}
  std::vector<std::uint8_t> bytes(const char* path){
    OmniFilesStatV1 shape{};require(api.stat(api.context,path,&shape)==0&&shape.kind==OMNI_FILES_REGULAR,"observe committed regular file");
    std::vector<std::uint8_t> data(static_cast<std::size_t>(shape.bytes));std::uint64_t handle=0;
    require(api.open(api.context,path,OMNI_FILES_READ,&handle)==0,"observe open");
    for(std::size_t at=0;at<data.size();){const auto count=std::min<std::size_t>(OMNI_FILES_MAX_TRANSFER,data.size()-at);std::uint64_t actual=0;require(api.read(api.context,handle,at,data.data()+at,count,&actual)==0&&actual==count,"observe complete bytes");at+=count;}
    require(api.close(api.context,handle)==0,"observe close");return data;
  }
  std::string text(const char* path){const auto value=bytes(path);return std::string(value.begin(),value.end());}
  OmniFilesUsageV1 usage(){OmniFilesUsageV1 result{};require(api.usage(api.context,&result)==0,"actual store usage");return result;}
};

static void stdio_modes_formatting_seek_and_atomic_visibility(){
  Case value;require(om_n64_mkdir("scratch")==0&&om_n64_mkdir("scratch\\case\\nested")==0,"root and normalized directory");
  const char* path="scratch/case/config.ini";value.raw(path,std::string("prior"));
  auto* output=om_n64_fopen(path,"wb");require(output,"staged config");
  require(om_n64_fprintf(output,"width=%d\nname=%s\n",640,"N64")==19,"formatted config");
  require(om_n64_fflush(output)==0&&value.text(path)=="prior","flush never publishes a partial image");
  require(om_n64_fclose(output)==0&&value.text(path)=="width=640\nname=N64\n","healthy close publishes exact config");
  auto* input=om_n64_fopen(path,"rb");require(input,"open staged config as input");
  std::array<char,32> line{};require(om_n64_fgets(line.data(),static_cast<int>(line.size()),input)&&std::string(line.data())=="width=640\n","first config line");
  require(om_n64_ftell(input)==10,"line byte position");
  require(om_n64_fseek(input,-4,SEEK_END)==0&&om_n64_fgets(line.data(),static_cast<int>(line.size()),input)&&std::string(line.data())=="N64\n","end-relative seek");
  require(om_n64_fgetc(input)==EOF&&om_n64_feof(input),"read end flag");om_n64_rewind(input);
  require(!om_n64_feof(input)&&om_n64_fgetc(input)=='w'&&om_n64_fclose(input)==0,"rewind input");
  value.raw(path,std::string("ab"));output=om_n64_fopen(path,"r+b");require(output,"copy-on-write update");
  require(om_n64_fseek(output,5,SEEK_SET)==0&&om_n64_fputs("XY",output)==0&&om_n64_fclose(output)==0,"sparse update commit");
  require(value.bytes(path)==std::vector<std::uint8_t>({'a','b',0,0,0,'X','Y'}),"sparse gap is zero");
  output=om_n64_fopen(path,"ab+");require(output&&om_n64_fseek(output,0,SEEK_SET)==0&&om_n64_fputs("!",output)==0&&om_n64_fclose(output)==0,"append ignores repositioning for writes");
  require(value.bytes(path)==std::vector<std::uint8_t>({'a','b',0,0,0,'X','Y','!'}),"append preserves complete prior image");
  require(!om_n64_fopen("scratch/case/missing","r+"),"update cannot create a missing file");
  for(const char* invalid:{"save/case/a","scratch/../escape","scratch/case/CON","scratch/case/a:stream","C:/temp/x","scratch/case/a.","scratch//x"})require(!om_n64_fopen(invalid,"wb"),"closed scratch path admission");
  require(!om_n64_fopen(path,"rbx"),"unsupported mode rejected");
  auto bad=value.api;bad.version=2;require(!om_n64_files_install(&bad)&&om_n64_files_installed(),"bad table preserves installed context");
  require(om_n64_files_install(nullptr)==1&&!om_n64_fopen(path,"rb"),"uninstalled bridge has no OS fallback");
}

static void reader_snapshot_and_publication_faults(){
  Case value;const char* path="scratch/case/cache";value.raw(path,std::string("old complete cache"));
  auto* old=om_n64_fopen(path,"rb");require(old,"hold old reader");
  auto* output=om_n64_fopen(path,"wb");require(output&&om_n64_fputs("new",output)==0&&om_n64_fclose(output)==0,"replace while old reader remains open");
  std::array<char,32> bytes{};require(om_n64_fread(bytes.data(),1,bytes.size(),old)==18&&std::string(bytes.data(),18)=="old complete cache","old reader retains its original node");
  require(om_n64_fclose(old)==0&&value.text(path)=="new","new opens see committed replacement");
  old=om_n64_fopen(path,"rb");require(old,"reader length fixture");value.raw("scratch/case/replacement",std::string("longer replacement"));
  require(value.api.rename(value.api.context,"scratch/case/replacement",path,1)==0,"external namespace replacement fixture");
  require(om_n64_fseek(old,0,SEEK_END)==0&&om_n64_ftell(old)==3,"open-reader end belongs to its own snapshot");
  require(om_n64_fclose(old)==0,"snapshot length fixture close");
  underlying=value.api;
  for(unsigned fault=0;fault<3;++fault){
    value.raw(path,std::string("retained"));const auto before=value.usage();auto api=value.api;
    if(fault==0)api.write=short_write;else if(fault==1)api.flush=fail_flush;else api.rename=fail_rename;
    require(om_n64_files_install(&api)==1,"install publication fault");
    output=om_n64_fopen(path,"wb");require(output,"open under publication fault");
    const auto count=om_n64_fwrite("replacement",1,11,output);
    if(fault==0){require(count!=11&&om_n64_ferror(output),"short callback write is sticky");om_n64_rewind(output);require(om_n64_fwrite("retry",1,5,output)==0,"rewind cannot authorize a failed writer");}
    else require(count==11,"stage complete image before later fault");
    require(om_n64_fclose(output)!=0,"failed publication reported");
    require(om_n64_files_install(&value.api)==1&&value.text(path)=="retained","old image survives each publication fault");
    const auto after=value.usage();require(after.files==before.files&&after.open_handles==before.open_handles&&after.scratch_allocated_bytes==before.scratch_allocated_bytes,"owned failed temporary and handles retired");
  }
  auto fault=value.api;fault.read=fail_read;require(om_n64_files_install(&fault)==1,"copy-read fault");
  require(!om_n64_fopen(path,"r+b"),"failed copy cannot open an update stream");
  require(om_n64_files_install(&value.api)==1&&value.text(path)=="retained","failed copy preserves destination");
  fault=value.api;fault.open=collide;require(om_n64_files_install(&fault)==1,"exclusive collision fault");
  require(!om_n64_fopen(path,"wb"),"exclusive collision refuses adoption");
  require(om_n64_files_install(&value.api)==1&&value.text(collision_path.c_str())=="another owner","collision cleanup cannot delete another owner's node");
  require(value.text(path)=="retained","collision preserves target");
}

static void cpp_stream_formatting_binary_lookahead_and_abort(){
  Case value;const char* path="scratch/case/shader";value.raw(path,std::string("previous"));
  {
    OmniN64Output output(path,OmniN64Output::binary|OmniN64Output::trunc);
    require(output.is_open(),"C++ output open");output<<std::hex<<std::setfill('0')<<std::setw(8)<<0x13<<'\n';
    const char binary[]={'a',0,'b'};output.write(binary,3);output.flush();
    require(output.good()&&value.text(path)=="previous","C++ flush retains old image");output.close();require(!output.fail(),"C++ complete publication");
  }
  {
    OmniN64Input input(path,OmniN64Input::binary);require(input.peek()=='0'&&input.tellg()==0,"tellg compensates lookahead");
    require(input.get()=='0'&&input.tellg()==1,"lookahead consumed exactly once");input.seekg(-1,OmniN64Input::cur);
    unsigned number=0;input>>std::hex>>number;require(input.good()&&number==0x13,"formatted hex input");
    require(input.get()=='\n',"formatted delimiter");std::array<char,3> binary{};input.read(binary.data(),3);
    require(input.good()&&binary==std::array<char,3>{'a',0,'b'},"binary payload with NUL");input.close();
  }
  const auto retained=value.bytes(path);
  {OmniN64Output output(path);output<<"incomplete";output.setstate(std::ios_base::badbit);output.close();require(output.fail(),"caller stream error reported");}
  require(value.bytes(path)==retained,"C++ failed stream cannot replace retained image");
  {OmniN64Output output(path);output<<"destructor must abort";output.setstate(std::ios_base::failbit);}
  require(value.bytes(path)==retained,"failed output destructor aborts");
  {OmniN64Output output(path);output<<"healthy destructor";}
  require(value.text(path)=="healthy destructor","healthy output destructor publishes");
  try{OmniN64Output output(path);output<<"partial during exception";throw std::runtime_error("unwind fixture");}catch(const std::runtime_error&){}
  require(value.text(path)=="healthy destructor","exception unwind aborts even without an ostream failure bit");
}

static void staged_optional_text(){
  Case value;const char* path="scratch/case/custom.ini";value.raw(path,std::string("[ROM]\nvideo=1\n"));
  std::string text="retained";require(om_n64_text::read(path,4096,text)&&text=="[ROM]\nvideo=1\n","complete checked optional text");
  text="retained";require(!om_n64_text::read(path,3,text)&&text=="retained","text bound preserves caller state");
  underlying=value.api;auto fault=value.api;fault.read=fail_read;require(om_n64_files_install(&fault)==1,"text read fault");
  require(!om_n64_text::read(path,4096,text)&&text=="retained","partial text never reaches settings parser");
  require(om_n64_files_install(&value.api)==1,"restore text table");
  value.raw(path,std::vector<std::uint8_t>({'a',0,'b'}));
  require(!om_n64_text::read(path,4096,text)&&text=="retained"&&value.usage().open_handles==0,"NUL text rejected and handles closed");
}

static void u32le(std::vector<std::uint8_t>& bytes,std::uint32_t value){for(unsigned n=0;n<4;++n)bytes.push_back(static_cast<std::uint8_t>(value>>(8*n)));}
static void u64le(std::vector<std::uint8_t>& bytes,std::uint64_t value){for(unsigned n=0;n<8;++n)bytes.push_back(static_cast<std::uint8_t>(value>>(8*n)));}
static std::vector<std::uint8_t> shader_header(std::uint32_t count){
  std::vector<std::uint8_t> bytes;u32le(bytes,0x3b);u32le(bytes,7);u32le(bytes,8);
  const std::string renderer="renderer",version="GL4";bytes.insert(bytes.end(),renderer.begin(),renderer.end());
  u32le(bytes,3);bytes.insert(bytes.end(),version.begin(),version.end());u32le(bytes,count);return bytes;
}
static void shader_record(std::vector<std::uint8_t>& bytes,std::uint64_t mux,std::uint32_t length){
  u64le(bytes,mux);u32le(bytes,0x13);u32le(bytes,0x8e21);u32le(bytes,length);
}
static void shader_cache_full_admission_before_gl(){
  Case value;const char* path="scratch/case/shader-cache";
  auto valid=shader_header(1);shader_record(valid,0x1234,3);valid.insert(valid.end(),{0x31,0,0x32});
  value.raw(path,valid);OmniN64Input input(path,OmniN64Input::binary);om_n64_shader::Image image;
  require(om_n64_shader::read_image(input,0x3b,7,"renderer","GL4",image),"independent binary shader fixture");input.close();
  require(!input.fail()&&image.records.size()==1&&image.records[0].mux==0x1234&&image.records[0].binary==std::vector<char>({'1',0,'2'}),"complete shader bytes retained before GL");
  const auto reject=[&](const std::vector<std::uint8_t>& bytes){
    value.raw(path,bytes);OmniN64Input stream(path,OmniN64Input::binary);
    require(!om_n64_shader::read_image(stream,0x3b,7,"renderer","GL4",image),"malformed shader image rejected");
    require(image.records.size()==1&&image.records[0].mux==0x1234&&image.records[0].binary==std::vector<char>({'1',0,'2'}),"failed admission keeps caller's previous complete image");stream.close();
  };
  for(std::size_t n=0;n<valid.size();++n)reject(std::vector<std::uint8_t>(valid.begin(),valid.begin()+n));
  auto broken=valid;broken.push_back(0);reject(broken);
  broken=valid;broken[0]=0x3c;reject(broken);broken=valid;broken[4]=8;reject(broken);
  broken=shader_header(om_n64_shader::maximum_records+1);reject(broken);
  broken=shader_header(1);shader_record(broken,1,0xffffffff);reject(broken);
  broken=shader_header(1);shader_record(broken,1,om_n64_shader::maximum_binary+1);reject(broken);
  broken=shader_header(2);for(unsigned n=0;n<2;++n){shader_record(broken,1,1);broken.push_back(0x31);}reject(broken);
  broken=valid;broken[8]=4;reject(broken); // A renderer prefix must not match.
  // Exercise aggregate parser budget independently of the store's own 64-MiB
  // page limit, without granting any OS path to the production parser.
  auto aggregate=shader_header(4);
  for(unsigned n=0;n<4;++n){shader_record(aggregate,n,om_n64_shader::maximum_binary);aggregate.resize(aggregate.size()+om_n64_shader::maximum_binary,0x31);}
  const std::string all(reinterpret_cast<const char*>(aggregate.data()),aggregate.size());std::istringstream source(all,std::ios_base::binary);
  require(!om_n64_shader::read_image(source,0x3b,7,"renderer","GL4",image)&&image.records[0].mux==0x1234,"aggregate image bound precedes another binary allocation");
  require(value.usage().open_handles==0,"no shader input handle leak");
}

static void shader_key_legacy_and_current_formats(){
  om_n64_shader::Keys keys;std::istringstream current("0x5\n0x2\n0x13\n0x91\n");
  require(om_n64_shader::read_keys(current,keys)&&!keys.legacy_hwl&&keys.values==std::vector<std::uint64_t>({0x13,0x91}),"current key format");
  std::istringstream legacy("0x4\n0x1\n0x1\n0x13\n");
  require(om_n64_shader::read_keys(legacy,keys)&&keys.legacy_hwl&&keys.hwl&&keys.values==std::vector<std::uint64_t>({0x13}),"legacy HWL key format");
  for(const char* malformed:{"0x3\n0\n","0x6\n0\n","0x4\n0x2\n0\n","0x5\n0x10001\n","0x5\n1\n","0x5\n2\n1\n1\n","0x5\n1\n1\ntrailing"}){
    std::istringstream input(malformed);require(!om_n64_shader::read_keys(input,keys)&&keys.legacy_hwl&&keys.hwl&&keys.values==std::vector<std::uint64_t>({0x13}),"bad keys preserve previous image");
  }
  std::istringstream empty("0x5\n0\n");require(om_n64_shader::read_keys(empty,keys)&&keys.values.empty(),"empty valid keys avoid progress division");
}

static std::uint64_t texture_shape_bytes(const om_n64_texture::Shape& shape){
  if((shape.format&~om_n64_texture::compressed)!=0x8058||shape.texture_format!=0x1908||shape.pixel_type!=0x1401)return 0;
  return std::uint64_t(shape.width)*shape.height*4;
}
static void texture_metadata(std::vector<std::uint8_t>& bytes,bool legacy,std::uint32_t length=16){
  u32le(bytes,2);u32le(bytes,2);u32le(bytes,0x8058);bytes.insert(bytes.end(),{8,0x19,1,0x14,1});
  if(!legacy)bytes.insert(bytes.end(),{1,2});u32le(bytes,length);
}
static std::vector<std::uint8_t> texture_file_fixture(bool legacy){
  std::vector<std::uint8_t> bytes;const std::uint64_t begin=legacy?12:16,position=begin+(legacy?21:23)+16;
  if(!legacy)u32le(bytes,om_n64_texture::version);u32le(bytes,7);u64le(bytes,position);texture_metadata(bytes,legacy);
  bytes.insert(bytes.end(),16,0x51);u32le(bytes,1);u64le(bytes,0x1234);u64le(bytes,begin|(std::uint64_t(legacy?0:0x0201)<<48));return bytes;
}
static std::vector<std::uint8_t> independent_gzip(const std::vector<std::uint8_t>& bytes){
  z_stream stream{};require(deflateInit2(&stream,1,Z_DEFLATED,MAX_WBITS+16,8,Z_DEFAULT_STRATEGY)==Z_OK,"independent texture fixture compressor");
  std::vector<std::uint8_t> output(compressBound(static_cast<uLong>(bytes.size()))+64);
  stream.next_in=const_cast<Bytef*>(bytes.data());stream.avail_in=static_cast<uInt>(bytes.size());stream.next_out=output.data();stream.avail_out=static_cast<uInt>(output.size());
  require(deflate(&stream,Z_FINISH)==Z_STREAM_END,"independent texture fixture gzip");output.resize(stream.total_out);require(deflateEnd(&stream)==Z_OK,"fixture gzip end");return output;
}
static void texture_independent_formats_and_corruption(){
  Case value;const char* path="scratch/case/texture";
  for(bool legacy:{false,true}){
    const auto fixture=texture_file_fixture(legacy);value.raw(path,fixture);om_n64_texture::Index index;
    {om_n64_texture::File file(path,false);require(file&&om_n64_texture::read_index(file,7,false,texture_shape_bytes,index)&&file.close(),"independent modern/legacy texture index");}
    require(index.legacy==legacy&&index.records.size()==1&&index.stored==16,"admitted exact file index");
    const auto rejected=[&](const std::vector<std::uint8_t>& bytes){value.raw(path,bytes);om_n64_texture::File file(path,false);
      require(file&&!om_n64_texture::read_index(file,7,false,texture_shape_bytes,index),"bad texture file rejected");
      require(index.legacy==legacy&&index.records.size()==1&&index.stored==16,"failed file parser retains previous accepted index");
    };
    for(std::size_t length=0;length<fixture.size();++length)rejected(std::vector<std::uint8_t>(fixture.begin(),fixture.begin()+length));
    auto bad=fixture;bad.push_back(0);rejected(bad);bad=fixture;bad[legacy?4:8]=0xff;rejected(bad);
    bad=fixture;bad[legacy?12:16]=0xff;rejected(bad);bad=fixture;bad[fixture.size()-8]=0;rejected(bad);
    bad=fixture;bad[fixture.size()-1]^=1;rejected(bad);
    const auto initial=legacy?12u:16u,metadata=legacy?21u:23u;
    std::vector<std::uint8_t> stream;if(!legacy)u32le(stream,om_n64_texture::version);u32le(stream,7);u64le(stream,0x1234);
    stream.insert(stream.end(),fixture.begin()+initial,fixture.begin()+initial+metadata+16);
    const auto gzip=independent_gzip(stream);value.raw(path,gzip);om_n64_texture::Image image;
    require(om_n64_texture::read_gzip(path,7,false,texture_shape_bytes,image)&&image.legacy==legacy&&image.records.size()==1&&image.records[0].data==std::vector<unsigned char>(16,0x51),"independent modern/legacy gzip texture records");
    for(std::size_t length=0;length<gzip.size();++length){value.raw(path,std::vector<std::uint8_t>(gzip.begin(),gzip.begin()+length));
      require(!om_n64_texture::read_gzip(path,7,false,texture_shape_bytes,image)&&image.records.size()==1,"gzip truncation leaves accepted records intact");}
    value.raw(path,gzip);require(!om_n64_texture::read_gzip(path,8,false,texture_shape_bytes,image)&&om_n64_texture::read_gzip(path,8,true,texture_shape_bytes,image),"configuration match and explicit force semantics");
  }
  std::string path_bytes="retained";require(om_n64_utf8(L"scratch/cache/game.htc",path_bytes)&&path_bytes=="scratch/cache/game.htc","locale-free cache path");
  require(om_n64_cache_path(L"scratch/cache",L"../outside").empty()&&om_n64_cache_path(L"scratch/cache",L"a:stream").empty(),"cache leaf cannot escape path or create a stream");
  require(value.usage().open_handles==0,"texture parser temporary readers retired");
}

static void quotas_and_independent_writers(){
  Case value;const char* path="scratch/case/retained";value.raw(path,std::string("old"));
  std::vector<OmniN64File*> streams;
  for(unsigned n=0;n<64;++n){auto* input=om_n64_fopen(path,"rb");require(input,"admit bounded stream slot");streams.push_back(input);}
  require(!om_n64_fopen(path,"rb")&&errno==EMFILE,"stream cap enforced");
  require(!om_n64_files_install(nullptr)&&!om_n64_files_install(&value.api),"live users prevent callback replacement");
  for(auto* input:streams)require(om_n64_fclose(input)==0,"release stream slots");
  std::uint64_t filler=0;require(value.api.open(value.api.context,"scratch/case/filler",OMNI_FILES_WRITE|OMNI_FILES_CREATE,&filler)==0,"actual scratch quota filler");
  require(value.api.resize(value.api.context,filler,64ull*1024*1024-4096)==0,"fill all remaining scratch pages");
  auto* output=om_n64_fopen(path,"wb");require(output&&om_n64_fputs("new",output)!=0&&om_n64_fclose(output)!=0,"quota denies replacement without deleting old data");
  require(!om_n64_fopen(path,"r+b")&&value.text(path)=="old","private copy also pays actual quota");
  require(value.api.close(value.api.context,filler)==0&&value.api.remove(value.api.context,"scratch/case/filler")==0,"release filler debt");
  std::atomic<bool> okay{true};std::vector<std::jthread> writers;
  for(unsigned slot=0;slot<4;++slot)writers.emplace_back([&,slot]{const auto name="scratch/case/thread-"+std::to_string(slot);for(unsigned n=0;n<8;++n){OmniN64Output file(name);file<<slot<<":"<<n;file.close();if(file.fail())okay=false;}});
  for(auto& writer:writers)writer.join();require(okay,"independent concurrent publishers");
  for(unsigned slot=0;slot<4;++slot){const auto name="scratch/case/thread-"+std::to_string(slot);require(value.text(name.c_str())==std::to_string(slot)+":7","separate destination bytes");}
  require(value.usage().open_handles==0,"no live handles after concurrent publication");
}

static const std::vector<std::uint8_t> hello_gzip={0x1f,0x8b,0x08,0,0,0,0,0,0,3,0xcb,0x48,0xcd,0xc9,0xc9,0x07,0,0x86,0xa6,0x10,0x36,5,0,0,0};
static std::vector<std::uint8_t> independent_zero_gzip(std::uint64_t bytes){
  z_stream stream{};require(deflateInit2(&stream,1,Z_DEFLATED,MAX_WBITS+16,8,Z_DEFAULT_STRATEGY)==Z_OK,"independent fixture compressor");
  std::array<std::uint8_t,65536> zeros{},buffer{};std::vector<std::uint8_t> image;
  while(bytes){const auto count=static_cast<uInt>(std::min<std::uint64_t>(bytes,zeros.size()));stream.next_in=zeros.data();stream.avail_in=count;
    while(stream.avail_in){stream.next_out=buffer.data();stream.avail_out=static_cast<uInt>(buffer.size());require(deflate(&stream,Z_NO_FLUSH)==Z_OK,"fixture deflate");image.insert(image.end(),buffer.begin(),buffer.begin()+(buffer.size()-stream.avail_out));}
    bytes-=count;
  }
  int status=Z_OK;while(status!=Z_STREAM_END){stream.next_out=buffer.data();stream.avail_out=static_cast<uInt>(buffer.size());status=deflate(&stream,Z_FINISH);require(status==Z_OK||status==Z_STREAM_END,"fixture footer");image.insert(image.end(),buffer.begin(),buffer.begin()+(buffer.size()-stream.avail_out));}
  require(deflateEnd(&stream)==Z_OK,"fixture compressor end");return image;
}

static void gzip_interoperability_framing_failures_and_publication(){
  Case value;const char* path="scratch/case/texture.gz";value.raw(path,hello_gzip);
  auto file=om_n64_gzopen(path,"rb");std::array<char,32> decoded{};
  require(file&&om_n64_gzread(file,decoded.data(),static_cast<unsigned>(decoded.size()))==5&&std::string(decoded.data(),5)=="hello","independent canonical gzip fixture");
  require(om_n64_gzeof(file)&&om_n64_gzread(file,decoded.data(),1)==0&&om_n64_gzclose(file)==Z_OK,"checked footer and EOF");
  for(unsigned fault=0;fault<4;++fault){auto broken=hello_gzip;
    if(fault==0)broken[17]^=1;else if(fault==1)broken.pop_back();else if(fault==2)broken.push_back(0);else broken.insert(broken.end(),hello_gzip.begin(),hello_gzip.end());
    value.raw(path,broken);file=om_n64_gzopen(path,"rb");require(file&&om_n64_gzread(file,decoded.data(),static_cast<unsigned>(decoded.size()))<0,"CRC/truncation/trailing/member corruption rejected");
    require(!om_n64_gzeof(file)&&om_n64_gzclose(file)!=Z_OK&&value.bytes(path)==broken,"failed decode is not EOF and cannot overwrite source");
  }
  value.raw(path,hello_gzip);file=om_n64_gzopen(path,"wb1");require(file,"gzip output");
  const std::string next="texture cache with full gzip framing";
  require(om_n64_gzwrite(file,next.data(),static_cast<unsigned>(next.size()))==static_cast<int>(next.size())&&value.bytes(path)==hello_gzip,"gzip stages until close");
  require(om_n64_gzclose(file)==Z_OK,"gzip writer complete");const auto encoded=value.bytes(path);
  z_stream stream{};require(inflateInit2(&stream,MAX_WBITS+16)==Z_OK,"independent decoder");
  std::array<unsigned char,128> out{};stream.next_in=const_cast<Bytef*>(encoded.data());stream.avail_in=static_cast<uInt>(encoded.size());stream.next_out=out.data();stream.avail_out=static_cast<uInt>(out.size());
  require(inflate(&stream,Z_FINISH)==Z_STREAM_END&&stream.avail_in==0&&std::string(reinterpret_cast<char*>(out.data()),stream.total_out)==next,"external inflate interoperability");
  require(inflateEnd(&stream)==Z_OK,"independent decoder end");
  file=om_n64_gzopen(path,"rb");require(file&&om_n64_gzseek(file,8,SEEK_SET)==8&&om_n64_gzseek(file,3,SEEK_CUR)==11,"bounded forward decompression seek");
  require(om_n64_gzread(file,decoded.data(),4)==4&&std::string(decoded.data(),4)==next.substr(11,4)&&om_n64_gzclose(file)==Z_OK,"seek bytes and unread footer validation");
  underlying=value.api;auto fault=value.api;fault.rename=fail_rename;require(om_n64_files_install(&fault)==1,"gzip publication fault");
  file=om_n64_gzopen(path,"wb9");require(file&&om_n64_gzwrite(file,"failed",6)==6&&om_n64_gzclose(file)!=Z_OK,"gzip close propagates failed commit");
  require(om_n64_files_install(&value.api)==1&&value.bytes(path)==encoded,"gzip failed commit retains old container");
}

static void gzip_decoded_budget(){
  Case value;const char* path="scratch/case/bomb.gz";
  value.raw(path,independent_zero_gzip(256ull*1024*1024+1));
  auto file=om_n64_gzopen(path,"rb");require(file,"over-budget independent fixture opens compressed bytes");
  std::array<unsigned char,65536> decoded{};std::uint64_t observed=0;int count=0;
  while((count=om_n64_gzread(file,decoded.data(),static_cast<unsigned>(decoded.size())))>0)observed+=static_cast<unsigned>(count);
  require(count<0&&observed<=256ull*1024*1024&&om_n64_gzclose(file)!=Z_OK,"decoded-byte budget rejects compression bomb");
  const auto exact=independent_zero_gzip(256ull*1024*1024);value.raw(path,exact);
  file=om_n64_gzopen(path,"rb");require(file&&om_n64_gzclose(file)==Z_OK,"exact decoded limit may finish its footer");
}

static void uncertain_close_is_terminal_for_table_lifetime(){
  // Keep this last. The production policy intentionally has no in-process reset
  // for an unconfirmed close; only actor retirement may end that lifetime.
  Case value;const char* path="scratch/case/uncertain";value.raw(path,std::string("retained"));
  auto api=value.api;api.close=uncertain_close;require(om_n64_files_install(&api)==1,"unconfirmed close table");
  auto* file=om_n64_fopen(path,"wb");require(file&&om_n64_fputs("new",file)==0&&om_n64_fclose(file)!=0,"unconfirmed close aborts publication");
  require(!om_n64_files_install(&value.api)&&!om_n64_files_install(nullptr),"uncertain handle cannot license table replacement");
  require(!om_n64_fopen(path,"rb")&&om_n64_mkdir("scratch/case/no-reuse")!=0,"poisoned actor admits no new work");
  require(value.text(path)=="retained"&&value.usage().open_handles==1,"old target and unresolved handle debt remain");
}

int main(){try{
  stdio_modes_formatting_seek_and_atomic_visibility();reader_snapshot_and_publication_faults();
  cpp_stream_formatting_binary_lookahead_and_abort();quotas_and_independent_writers();
  staged_optional_text();shader_cache_full_admission_before_gl();shader_key_legacy_and_current_formats();
  texture_independent_formats_and_corruption();
  gzip_interoperability_framing_failures_and_publication();gzip_decoded_budget();
  uncertain_close_is_terminal_for_table_lifetime();std::cout<<"N64 memory I/O cases passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
