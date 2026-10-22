#include "engine.hpp"
#include "saved_files.hpp"
#include "memory_files.hpp"
#include <algorithm>
#include <cctype>
#include <cstring>
#include <fstream>
#include <map>
#include <set>
#include <random>
#ifdef _WIN32
#include <windows.h>
#endif

namespace omni {
namespace {
constexpr std::uint64_t max_total=256u*1024u*1024u;
constexpr std::uint32_t max_files=4096;
constexpr std::uint32_t max_entries=8192;
std::uint32_t number(std::span<const std::uint8_t> data,std::size_t offset){
  if(offset>data.size()||data.size()-offset<4)throw Failure("incompatible_save");
  return std::uint32_t(data[offset])|(std::uint32_t(data[offset+1])<<8)|(std::uint32_t(data[offset+2])<<16)|(std::uint32_t(data[offset+3])<<24);
}
void append(std::vector<std::uint8_t>& data,std::uint32_t value){for(unsigned n=0;n<4;++n)data.push_back(static_cast<std::uint8_t>(value>>(n*8)));}
std::string folded_name(const std::string& value){
#ifdef _WIN32
  const auto count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,value.data(),static_cast<int>(value.size()),nullptr,0);
  if(count<=0)throw Failure("incompatible_save");std::wstring wide(count,L'\0');
  if(MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,value.data(),static_cast<int>(value.size()),wide.data(),count)!=count)throw Failure("incompatible_save");
  for(const auto c:wide)if(c<32||(c>=127&&c<=159))throw Failure("incompatible_save");
  const auto mapped_count=LCMapStringEx(LOCALE_NAME_INVARIANT,LCMAP_UPPERCASE,wide.data(),count,nullptr,0,nullptr,nullptr,0);
  if(mapped_count<=0)throw Failure("incompatible_save");std::wstring mapped(mapped_count,L'\0');
  if(LCMapStringEx(LOCALE_NAME_INVARIANT,LCMAP_UPPERCASE,wide.data(),count,mapped.data(),mapped_count,nullptr,nullptr,0)!=mapped_count)throw Failure("incompatible_save");
  const auto bytes=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,mapped.data(),mapped_count,nullptr,0,nullptr,nullptr);
  if(bytes<=0)throw Failure("incompatible_save");std::string result(bytes,'\0');
  if(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,mapped.data(),mapped_count,result.data(),bytes,nullptr,nullptr)!=bytes)throw Failure("incompatible_save");return result;
#else
  // This runtime package is Windows-only. Another platform's filesystem needs
  // its own authoritative filename equivalence policy before non-ASCII support.
  std::string result=value;for(unsigned char c:value)if(c>126)throw Failure("unsupported_capability");
  std::transform(result.begin(),result.end(),result.begin(),[](unsigned char c){return static_cast<char>(std::toupper(c));});return result;
#endif
}
std::string portable_name(const std::filesystem::path& path){const auto text=path.generic_u8string();return std::string(reinterpret_cast<const char*>(text.data()),text.size());}
std::string safe_name(const std::string& value){
  // PSP savedata uses portable names. Reject ambiguous host spellings, traversal,
  // device names, stream syntax and case aliases before materializing any byte.
  if(value.empty()||value.size()>512||value.front()=='/'||value.back()=='/')throw Failure("incompatible_save");
  unsigned depth=0;std::size_t at=0;
  for(unsigned char c:value)if(c<32||c==127||std::strchr("<>:\\|?*\"",c))throw Failure("incompatible_save");
  const auto folded=folded_name(value);
  while(at<value.size()){
    const auto end=value.find('/',at);const auto part=value.substr(at,end==std::string::npos?std::string::npos:end-at);
    if(part.empty()||part=="."||part==".."||part.back()=='.'||part.back()==' '||part.size()>255||++depth>8)throw Failure("incompatible_save");
    const auto upper=folded_name(part);
    const auto stem=upper.substr(0,upper.find('.'));
    const auto tail=stem.size()>3?stem.substr(3):std::string{};
    const bool device_number=(tail.size()==1&&tail[0]>='0'&&tail[0]<='9')||tail=="\xc2\xb9"||tail=="\xc2\xb2"||tail=="\xc2\xb3";
    if(stem=="CON"||stem=="PRN"||stem=="AUX"||stem=="NUL"||stem=="CONIN$"||stem=="CONOUT$"||stem=="CLOCK$"||((stem.starts_with("COM")||stem.starts_with("LPT"))&&device_number))throw Failure("incompatible_save");
    if(end==std::string::npos)break;at=end+1;
  }
  return folded;
}
bool linked(const std::filesystem::path& path){
#ifdef _WIN32
  const auto attributes=GetFileAttributesW(path.c_str());
  if(attributes==INVALID_FILE_ATTRIBUTES)throw Failure("runtime_crashed");
  return (attributes&FILE_ATTRIBUTE_REPARSE_POINT)!=0;
#else
  return std::filesystem::is_symlink(std::filesystem::symlink_status(path));
#endif
}
void directory(const std::filesystem::path& path){
  if(!std::filesystem::exists(path))std::filesystem::create_directory(path);
  if(linked(path)||!std::filesystem::is_directory(path))throw Failure("runtime_crashed");
}
void write_new(const std::filesystem::path& path,std::span<const std::uint8_t> payload){
#ifdef _WIN32
  const auto file=CreateFileW(path.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
  if(file==INVALID_HANDLE_VALUE){if(GetLastError()==ERROR_FILE_EXISTS||GetLastError()==ERROR_ALREADY_EXISTS)throw Failure("incompatible_save");throw Failure("storage_failure");}
  bool valid=true;std::size_t at=0;
  while(at<payload.size()){
    const auto amount=static_cast<DWORD>(std::min<std::size_t>(payload.size()-at,1024*1024));DWORD written=0;
    if(!WriteFile(file,payload.data()+at,amount,&written,nullptr)||written==0){valid=false;break;}at+=written;
  }
  if(valid&&!FlushFileBuffers(file))valid=false;if(!CloseHandle(file))valid=false;
  if(!valid)throw Failure("storage_failure");
#else
  if(std::filesystem::exists(path))throw Failure("incompatible_save");
  std::ofstream output(path,std::ios::binary|std::ios::out);
  if(!output||!output.write(reinterpret_cast<const char*>(payload.data()),static_cast<std::streamsize>(payload.size()))||!output.flush())throw Failure("storage_failure");output.close();if(!output)throw Failure("storage_failure");
#endif
}
struct Entry{std::string name;std::span<const std::uint8_t> payload;bool directory;};
std::vector<Entry> parse(std::span<const std::uint8_t> archive){
  if(archive.size()<12||archive.size()>max_total)throw Failure("incompatible_save");
  const bool legacy=std::memcmp(archive.data(),"OPSF0001",8)==0;
  if(!legacy&&std::memcmp(archive.data(),"OPSF0002",8)!=0)throw Failure("incompatible_save");
  const auto count=number(archive,8);if(count>(legacy?max_files:max_entries))throw Failure("incompatible_save");
  std::size_t at=12;std::uint64_t materialized=12;std::vector<Entry> entries;entries.reserve(count);std::map<std::string,bool> aliases;std::map<std::string,std::string> spellings;std::set<std::string> directories;unsigned files=0;
  for(std::uint32_t n=0;n<count;++n){
    const auto kind=legacy?0u:number(archive,at);const auto name_size=number(archive,at+(legacy?0:4)),bytes=number(archive,at+(legacy?4:8));at+=legacy?8:12;
    if(kind>1||(kind==1&&bytes!=0))throw Failure("incompatible_save");
    if(!name_size||name_size>512||at>archive.size()||name_size>archive.size()-at)throw Failure("incompatible_save");
    std::string name(reinterpret_cast<const char*>(archive.data()+at),name_size);at+=name_size;
    const auto alias=safe_name(name);if(!aliases.emplace(alias,kind==1).second||bytes>archive.size()-at)throw Failure("incompatible_save");
    for(std::size_t pos=0;;){const auto end=name.find('/',pos);const auto spelling=name.substr(0,end);const auto folded=safe_name(spelling);const auto [where,inserted]=spellings.emplace(folded,spelling);if(!inserted&&where->second!=spelling)throw Failure("incompatible_save");if(end==std::string::npos)break;pos=end+1;}
    materialized+=12+std::uint64_t(name_size)+bytes;if(materialized>max_total)throw Failure("incompatible_save");
    if(kind==1)directories.insert(alias);else if(++files>max_files)throw Failure("incompatible_save");
    for(auto pos=alias.find('/');pos!=std::string::npos;pos=alias.find('/',pos+1))directories.insert(alias.substr(0,pos));
    if(directories.size()+files>max_entries)throw Failure("incompatible_save");
    entries.push_back({std::move(name),archive.subspan(at,bytes),kind==1});at+=bytes;
  }
  if(at!=archive.size())throw Failure("incompatible_save");
  // A file may not shadow a parent directory, in either case spelling.
  for(const auto& [alias,is_directory]:aliases)for(auto pos=alias.find('/');pos!=std::string::npos;pos=alias.find('/',pos+1)){const auto parent=aliases.find(alias.substr(0,pos));if(parent!=aliases.end()&&!parent->second)throw Failure("incompatible_save");}
  for(const auto& path:directories)if(!aliases.contains(path)){materialized+=12+spellings.at(path).size();if(materialized>max_total)throw Failure("incompatible_save");}
  return entries;
}
std::filesystem::path fresh_directory(const std::filesystem::path& parent){
  std::random_device random;
  for(unsigned attempt=0;attempt<16;++attempt){const auto path=parent/(".omniplay-save-"+std::to_string(random())+"-"+std::to_string(random()));std::error_code error;if(std::filesystem::create_directory(path,error))return path;if(error&&error!=std::errc::file_exists)throw Failure("runtime_crashed");}
  throw Failure("runtime_crashed");
}
void remove_owned(const std::filesystem::path& parent,const std::filesystem::path& target){
  if(target.parent_path()!=parent||!target.filename().string().starts_with(".omniplay-save-")||linked(target))throw Failure("runtime_crashed");
  // Refuse a nested reparse point even on error cleanup. No recursive delete may
  // follow a game-created path outside this owned private staging directory.
  for(const auto& item:std::filesystem::recursive_directory_iterator(target))if(linked(item.path()))throw Failure("runtime_crashed");
  std::filesystem::remove_all(target);
}
}
namespace {
void memory_result(int result){
  if(result==OMNI_FILES_OK)return;
  if(result==OMNI_FILES_NO_SPACE||result==OMNI_FILES_NO_MEMORY)throw Failure("resource_limit");
  if(result==OMNI_FILES_INVALID||result==OMNI_FILES_EXISTS||result==OMNI_FILES_NOT_FOUND||result==OMNI_FILES_IS_DIRECTORY)throw Failure("incompatible_save");
  if(result==OMNI_FILES_BUSY||result==OMNI_FILES_STALE)throw Failure("invalid_state");
  throw Failure("storage_failure");
}
}
void SavedFiles::prepare(const std::string& relative,MemoryFiles* memory){
  if(relative.empty())return;safe_name(relative);
  if(!relative.starts_with("save/"))throw Failure("runtime_crashed");
  if(memory){auto api=memory->interface();memory_result(api.mkdir(api.context,relative.c_str(),1));root_=std::filesystem::u8path(relative);memory_=memory;return;}
  auto path=std::filesystem::current_path();
  for(const auto& component:std::filesystem::path(relative)){path/=component;directory(path);}
  root_=path;
}
std::vector<std::uint8_t> SavedFiles::read(const std::vector<SaveFileShape>* expected)const{
  if(!enabled())throw Failure("unsupported_capability");
  if(memory_){
    std::vector<std::uint8_t> archive;try{archive=memory_->capture(portable_name(root_));}catch(const std::length_error&){throw Failure("resource_limit");}
    const auto entries=parse(archive);
    if(expected){if(entries.size()!=expected->size())throw Failure("incompatible_save");for(const auto& entry:entries){const auto found=std::find_if(expected->begin(),expected->end(),[&](const SaveFileShape& value){return value.name==entry.name;});if(entry.directory||found==expected->end()||found->bytes!=entry.payload.size())throw Failure("incompatible_save");}}
    return archive;
  }
  if(linked(root_))throw Failure("runtime_crashed");
  struct Node{std::string name;std::filesystem::path path;bool directory;std::uint64_t bytes;};std::vector<Node> nodes;std::set<std::string> aliases;std::size_t visited=0;unsigned files=0,dirs=0;
  for(auto it=std::filesystem::recursive_directory_iterator(root_);it!=std::filesystem::recursive_directory_iterator();++it){
    if(++visited>max_entries)throw Failure("resource_limit");if(linked(it->path()))throw Failure("runtime_crashed");
    const auto relative=portable_name(it->path().lexically_relative(root_));const auto alias=safe_name(relative);
    if(!aliases.insert(alias).second)throw Failure("runtime_crashed");
    const bool is_directory=it->is_directory();
    if(it->is_regular_file()){if(++files>max_files)throw Failure("resource_limit");}
    else if(!is_directory)throw Failure("runtime_crashed");
    else if(++dirs>max_entries)throw Failure("resource_limit");
    const auto bytes=is_directory?0:std::filesystem::file_size(it->path());
    if(expected){const auto found=std::find_if(expected->begin(),expected->end(),[&](const SaveFileShape& shape){return shape.name==relative;});if(is_directory||nodes.size()>=expected->size()||found==expected->end()||found->bytes!=bytes)throw Failure("incompatible_save");}
    nodes.push_back({relative,it->path(),is_directory,bytes});
  }
  if(expected&&nodes.size()!=expected->size())throw Failure("incompatible_save");
  std::sort(nodes.begin(),nodes.end(),[](const Node& a,const Node& b){return a.name<b.name;});std::vector<std::uint8_t> out{'O','P','S','F','0','0','0','2'};append(out,static_cast<std::uint32_t>(nodes.size()));
  for(const auto& node:nodes){
    const auto size=node.bytes;
    if(size>max_total||out.size()+12+node.name.size()+size>max_total)throw Failure("resource_limit");
    append(out,node.directory?1:0);append(out,static_cast<std::uint32_t>(node.name.size()));append(out,static_cast<std::uint32_t>(size));out.insert(out.end(),node.name.begin(),node.name.end());const auto at=out.size();out.resize(at+size);
    if(!node.directory){std::ifstream input(node.path,std::ios::binary);if(!input||!input.read(reinterpret_cast<char*>(out.data()+at),static_cast<std::streamsize>(size))||input.peek()!=std::char_traits<char>::eof())throw Failure("storage_failure");}
  }
  return out;
}
void SavedFiles::validate(std::span<const std::uint8_t> archive){(void)parse(archive);}
void SavedFiles::replace(std::span<const std::uint8_t> archive)const{
  if(!enabled())throw Failure("unsupported_capability");const auto entries=parse(archive);
  if(memory_){std::vector<MemoryFileEntry> memory;memory.reserve(entries.size());for(const auto& entry:entries)memory.push_back({entry.name,entry.payload,entry.directory});memory_result(memory_->replace(portable_name(root_),memory));return;}
  if(linked(root_))throw Failure("runtime_crashed");
  const auto parent=root_.parent_path();const auto staging=fresh_directory(parent);std::filesystem::path backup;
  try{
    for(const auto& entry:entries){const auto relative=std::filesystem::u8path(entry.name);const auto path=staging/relative;auto cursor=staging;for(const auto& part:relative.parent_path()){cursor/=part;directory(cursor);}if(entry.directory)directory(path);else write_new(path,entry.payload);}
    backup=fresh_directory(parent);std::filesystem::remove(backup);std::filesystem::rename(root_,backup);
    try{std::filesystem::rename(staging,root_);}catch(...){std::filesystem::rename(backup,root_);throw;}
    remove_owned(parent,backup);
  }catch(...){if(std::filesystem::exists(staging)){try{remove_owned(parent,staging);}catch(...){}}throw;}
}
}
