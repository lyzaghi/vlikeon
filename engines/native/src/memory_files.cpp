#include "memory_files.hpp"
#include <algorithm>
#include <cstring>
#include <limits>
#include <stdexcept>
#include <utility>
#ifdef _WIN32
#include <windows.h>
#endif

namespace omni {
MemoryFiles::Node::Node(Budget* value,std::string name,bool dir):budget(value),path(std::move(name)),directory(dir){if(dir)++budget->directories;else ++budget->files;}
MemoryFiles::Node::~Node(){budget->allocated-=std::uint64_t(pages.size())*page_bytes;if(directory)--budget->directories;else --budget->files;}
MemoryFiles::MemoryFiles(){
  budgets_[0].limit=save_limit;budgets_[1].limit=scratch_limit;
  nodes_.emplace("SAVE",std::make_shared<Node>(&budgets_[0],"save",true));
  nodes_.emplace("SCRATCH",std::make_shared<Node>(&budgets_[1],"scratch",true));
}
std::string MemoryFiles::folded(const std::string& value){
#ifdef _WIN32
  const auto count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,value.data(),static_cast<int>(value.size()),nullptr,0);
  if(count<=0)throw std::invalid_argument("path");std::wstring wide(count,L'\0');
  if(MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,value.data(),static_cast<int>(value.size()),wide.data(),count)!=count)throw std::invalid_argument("path");
  for(auto unit:wide)if(unit<32||(unit>=127&&unit<=159))throw std::invalid_argument("path");
  const auto n=LCMapStringEx(LOCALE_NAME_INVARIANT,LCMAP_UPPERCASE,wide.data(),count,nullptr,0,nullptr,nullptr,0);
  if(n<=0)throw std::invalid_argument("path");std::wstring upper(n,L'\0');
  if(LCMapStringEx(LOCALE_NAME_INVARIANT,LCMAP_UPPERCASE,wide.data(),count,upper.data(),n,nullptr,nullptr,0)!=n)throw std::invalid_argument("path");
  const auto bytes=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,upper.data(),n,nullptr,0,nullptr,nullptr);
  if(bytes<=0)throw std::invalid_argument("path");std::string result(bytes,'\0');
  if(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,upper.data(),n,result.data(),bytes,nullptr,nullptr)!=bytes)throw std::invalid_argument("path");return result;
#else
  // The initial native file-equivalence policy is Windows-only. Other targets
  // retain the existing ASCII-only admission until their own policy is reviewed.
  std::string result=value;for(auto& c:result){const auto u=static_cast<unsigned char>(c);if(u>126)throw std::invalid_argument("path");if(c>='a'&&c<='z')c=static_cast<char>(c-'a'+'A');}return result;
#endif
}
MemoryFiles::Path MemoryFiles::path(const char* raw){
  if(!raw)throw std::invalid_argument("path");std::size_t size=0;while(size<=1024&&raw[size])++size;
  if(!size||size>1024)throw std::invalid_argument("path");std::string text(raw,size);
  if(text.front()=='/'||text.back()=='/')throw std::invalid_argument("path");
  for(unsigned char c:text)if(c<32||c==127||std::strchr("<>:\\|?*\"",c))throw std::invalid_argument("path");
  const auto key=folded(text);unsigned depth=0;std::size_t at=0;
  while(at<text.size()){
    const auto end=text.find('/',at);const auto part=text.substr(at,end==std::string::npos?std::string::npos:end-at);
    if(part.empty()||part=="."||part==".."||part.back()=='.'||part.back()==' '||part.size()>255||++depth>16)throw std::invalid_argument("path");
    const auto upper=folded(part),stem=upper.substr(0,upper.find('.'));const auto tail=stem.size()>3?stem.substr(3):std::string{};
    const bool numbered=(tail.size()==1&&tail[0]>='0'&&tail[0]<='9')||tail=="\xc2\xb9"||tail=="\xc2\xb2"||tail=="\xc2\xb3";
    if(stem=="CON"||stem=="PRN"||stem=="AUX"||stem=="NUL"||stem=="CONIN$"||stem=="CONOUT$"||stem=="CLOCK$"||((stem.starts_with("COM")||stem.starts_with("LPT"))&&numbered))throw std::invalid_argument("path");
    if(end==std::string::npos)break;at=end+1;
  }
  const auto top=key.substr(0,key.find('/'));if(top!="SAVE"&&top!="SCRATCH")throw std::invalid_argument("root");
  return {std::move(text),key,top=="SAVE"?0u:1u};
}
std::string MemoryFiles::parent_key(const std::string& key){const auto at=key.rfind('/');return at==std::string::npos?std::string{}:key.substr(0,at);}
std::string MemoryFiles::leaf(const std::string& text){const auto at=text.rfind('/');return at==std::string::npos?text:text.substr(at+1);}
bool MemoryFiles::room_node(bool directory)const{const auto files=budgets_[0].files+budgets_[1].files,dirs=budgets_[0].directories+budgets_[1].directories;return files+dirs<maximum_nodes&&(directory||files<maximum_files);}
bool MemoryFiles::mutable_revision()const{return revision_!=std::numeric_limits<std::uint64_t>::max();}
bool MemoryFiles::children(const std::string& key)const{const auto prefix=key+'/';const auto at=nodes_.lower_bound(prefix);return at!=nodes_.end()&&at->first.starts_with(prefix);}
int MemoryFiles::grow(Node& node,std::uint64_t bytes){
  if(bytes>file_limit)return OMNI_FILES_NO_SPACE;const auto required=static_cast<std::size_t>((bytes+page_bytes-1)/page_bytes);
  if(required<=node.pages.size())return OMNI_FILES_OK;const auto added=required-node.pages.size();const auto charge=std::uint64_t(added)*page_bytes;
  if(charge>node.budget->limit-node.budget->allocated)return OMNI_FILES_NO_SPACE;
  // Allocate only new pages and stage all of them before mutating old data.
  // A failed allocation/reserve leaves bytes, handles and counters unchanged.
  std::vector<Page> staged;staged.reserve(added);
  for(std::size_t n=0;n<added;++n)staged.push_back(std::make_unique<std::array<std::uint8_t,page_bytes>>());
  node.pages.reserve(required);for(auto& page:staged)node.pages.push_back(std::move(page));node.budget->allocated+=charge;return OMNI_FILES_OK;
}
void MemoryFiles::zero(Node& node,std::uint64_t from,std::uint64_t to){while(from<to){const auto page=static_cast<std::size_t>(from/page_bytes),within=static_cast<std::size_t>(from%page_bytes);const auto n=static_cast<std::size_t>(std::min<std::uint64_t>(to-from,page_bytes-within));std::memset(node.pages[page]->data()+within,0,n);from+=n;}}
void MemoryFiles::shrink(Node& node,std::uint64_t bytes){
  const auto required=static_cast<std::size_t>((bytes+page_bytes-1)/page_bytes);
  if(required&&bytes%page_bytes)zero(node,bytes,std::uint64_t(required)*page_bytes);
  const auto removed=node.pages.size()-required;node.pages.resize(required);node.budget->allocated-=std::uint64_t(removed)*page_bytes;node.bytes=bytes;
}
void MemoryFiles::describe(const Node& node,OmniFilesStatV1& out){out={static_cast<std::uint32_t>(node.directory?OMNI_FILES_DIRECTORY:OMNI_FILES_REGULAR),0,node.bytes,std::uint64_t(node.pages.size())*page_bytes};}
int MemoryFiles::stat(const char* raw,OmniFilesStatV1* out){if(!out)return OMNI_FILES_INVALID;const auto name=path(raw);std::lock_guard lock(mutex_);const auto found=nodes_.find(name.key);if(found==nodes_.end())return OMNI_FILES_NOT_FOUND;describe(*found->second,*out);return OMNI_FILES_OK;}
int MemoryFiles::mkdir(const char* raw,std::uint32_t recursive){
  if(recursive>1)return OMNI_FILES_INVALID;const auto name=path(raw);std::lock_guard lock(mutex_);
  const auto found=nodes_.find(name.key);if(found!=nodes_.end())return found->second->directory?OMNI_FILES_OK:OMNI_FILES_EXISTS;
  if(!mutable_revision())return OMNI_FILES_BUSY;auto candidate=nodes_;std::size_t at=name.text.find('/');
  while(at!=std::string::npos){const auto end=name.text.find('/',at+1);const auto text=name.text.substr(0,end),key=folded(text);
    const auto existing=candidate.find(key);
    if(existing!=candidate.end()){if(!existing->second->directory)return OMNI_FILES_IS_DIRECTORY;}
    else{
      if(!recursive&&end!=std::string::npos)return OMNI_FILES_NOT_FOUND;
      const auto parent=candidate.find(parent_key(key));if(parent==candidate.end()||!parent->second->directory)return OMNI_FILES_NOT_FOUND;
      if(!room_node(true))return OMNI_FILES_NO_SPACE;
      candidate.emplace(key,std::make_shared<Node>(&budgets_[name.root],parent->second->path+'/'+leaf(text),true));
    }
    at=end;
  }
  nodes_.swap(candidate);++revision_;return OMNI_FILES_OK;
}
int MemoryFiles::open(const char* raw,std::uint32_t mode,std::uint64_t* output){
  if(!output)return OMNI_FILES_INVALID;*output=0;
  if((mode&~31u)||!(mode&(OMNI_FILES_READ|OMNI_FILES_WRITE))||((mode&(OMNI_FILES_CREATE|OMNI_FILES_TRUNCATE))&&!(mode&OMNI_FILES_WRITE))||((mode&OMNI_FILES_EXCLUSIVE)&&!(mode&OMNI_FILES_CREATE)))return OMNI_FILES_INVALID;
  const auto name=path(raw);std::lock_guard lock(mutex_);
  if(handles_.size()>=maximum_handles||next_handle_==std::numeric_limits<std::uint64_t>::max())return OMNI_FILES_NO_SPACE;
  const auto existing=nodes_.find(name.key);std::shared_ptr<Node> node;bool created=false;
  if(existing!=nodes_.end()){
    node=existing->second;if(node->directory)return OMNI_FILES_IS_DIRECTORY;if((mode&OMNI_FILES_CREATE)&&(mode&OMNI_FILES_EXCLUSIVE))return OMNI_FILES_EXISTS;
  }else{
    if(!(mode&OMNI_FILES_CREATE))return OMNI_FILES_NOT_FOUND;if(!mutable_revision())return OMNI_FILES_BUSY;
    const auto parent=nodes_.find(parent_key(name.key));if(parent==nodes_.end()||!parent->second->directory)return OMNI_FILES_NOT_FOUND;
    if(!room_node(false))return OMNI_FILES_NO_SPACE;node=std::make_shared<Node>(&budgets_[name.root],parent->second->path+'/'+leaf(name.text),false);created=true;
  }
  if((mode&OMNI_FILES_TRUNCATE)&&!mutable_revision())return OMNI_FILES_BUSY;
  const auto id=next_handle_;handles_.emplace(id,Handle{node,mode});
  try{if(created)nodes_.emplace(name.key,node);}catch(...){handles_.erase(id);throw;}
  if(mode&OMNI_FILES_TRUNCATE)shrink(*node,0);
  if(created||(mode&OMNI_FILES_TRUNCATE))++revision_;
  ++next_handle_;*output=id;return OMNI_FILES_OK;
}
int MemoryFiles::close(std::uint64_t handle){std::lock_guard lock(mutex_);return handles_.erase(handle)?OMNI_FILES_OK:OMNI_FILES_INVALID;}
int MemoryFiles::read(std::uint64_t handle,std::uint64_t offset,void* data,std::uint64_t bytes,std::uint64_t* actual){
  if(!actual||bytes>OMNI_FILES_MAX_TRANSFER||(!data&&bytes))return OMNI_FILES_INVALID;*actual=0;std::lock_guard lock(mutex_);const auto found=handles_.find(handle);
  if(found==handles_.end())return OMNI_FILES_INVALID;if(!(found->second.mode&OMNI_FILES_READ))return OMNI_FILES_DENIED;
  const auto& node=*found->second.node;const auto count=offset>=node.bytes?0:std::min(bytes,node.bytes-offset);auto* out=static_cast<std::uint8_t*>(data);std::uint64_t done=0;
  while(done<count){const auto at=offset+done,page=static_cast<std::size_t>(at/page_bytes),within=static_cast<std::size_t>(at%page_bytes),n=static_cast<std::size_t>(std::min<std::uint64_t>(count-done,page_bytes-within));std::memcpy(out+done,node.pages[page]->data()+within,n);done+=n;}
  *actual=count;return OMNI_FILES_OK;
}
int MemoryFiles::write(std::uint64_t handle,std::uint64_t offset,const void* data,std::uint64_t bytes,std::uint64_t* actual){
  if(!actual||bytes>OMNI_FILES_MAX_TRANSFER||(!data&&bytes))return OMNI_FILES_INVALID;*actual=0;std::lock_guard lock(mutex_);const auto found=handles_.find(handle);
  if(found==handles_.end())return OMNI_FILES_INVALID;if(!(found->second.mode&OMNI_FILES_WRITE))return OMNI_FILES_DENIED;if(!bytes)return OMNI_FILES_OK;
  if(offset>file_limit||bytes>file_limit-offset)return OMNI_FILES_NO_SPACE;if(!mutable_revision())return OMNI_FILES_BUSY;
  auto& node=*found->second.node;const auto end=offset+bytes;const auto result=grow(node,end);if(result!=OMNI_FILES_OK)return result;
  if(offset>node.bytes)zero(node,node.bytes,offset);const auto* input=static_cast<const std::uint8_t*>(data);std::uint64_t done=0;
  while(done<bytes){const auto at=offset+done,page=static_cast<std::size_t>(at/page_bytes),within=static_cast<std::size_t>(at%page_bytes),n=static_cast<std::size_t>(std::min<std::uint64_t>(bytes-done,page_bytes-within));std::memcpy(node.pages[page]->data()+within,input+done,n);done+=n;}
  node.bytes=std::max(node.bytes,end);++revision_;*actual=bytes;return OMNI_FILES_OK;
}
int MemoryFiles::resize(std::uint64_t handle,std::uint64_t bytes){
  if(bytes>file_limit)return OMNI_FILES_NO_SPACE;std::lock_guard lock(mutex_);const auto found=handles_.find(handle);if(found==handles_.end())return OMNI_FILES_INVALID;
  if(!(found->second.mode&OMNI_FILES_WRITE))return OMNI_FILES_DENIED;auto& node=*found->second.node;if(bytes==node.bytes)return OMNI_FILES_OK;if(!mutable_revision())return OMNI_FILES_BUSY;
  if(bytes<node.bytes)shrink(node,bytes);else{const auto result=grow(node,bytes);if(result!=OMNI_FILES_OK)return result;zero(node,node.bytes,bytes);node.bytes=bytes;}
  ++revision_;return OMNI_FILES_OK;
}
int MemoryFiles::flush(std::uint64_t handle){std::lock_guard lock(mutex_);return handles_.contains(handle)?OMNI_FILES_OK:OMNI_FILES_INVALID;}
int MemoryFiles::remove(const char* raw){const auto name=path(raw);std::lock_guard lock(mutex_);if(parent_key(name.key).empty())return OMNI_FILES_DENIED;const auto found=nodes_.find(name.key);if(found==nodes_.end())return OMNI_FILES_NOT_FOUND;if(children(name.key))return OMNI_FILES_NOT_EMPTY;if(!mutable_revision())return OMNI_FILES_BUSY;nodes_.erase(found);++revision_;return OMNI_FILES_OK;}
int MemoryFiles::rename(const char* raw_from,const char* raw_to,std::uint32_t replace){
  if(replace>1)return OMNI_FILES_INVALID;const auto from=path(raw_from),to=path(raw_to);std::lock_guard lock(mutex_);
  if(from.root!=to.root||parent_key(from.key).empty()||parent_key(to.key).empty())return OMNI_FILES_DENIED;
  const auto source=nodes_.find(from.key);if(source==nodes_.end())return OMNI_FILES_NOT_FOUND;if(to.key.starts_with(from.key+'/'))return OMNI_FILES_INVALID;
  const auto parent=nodes_.find(parent_key(to.key));if(parent==nodes_.end()||!parent->second->directory)return OMNI_FILES_NOT_FOUND;
  const auto target=nodes_.find(to.key);if(target!=nodes_.end()&&target!=source){if(!replace)return OMNI_FILES_EXISTS;if(source->second->directory!=target->second->directory)return OMNI_FILES_INVALID;if(children(to.key))return OMNI_FILES_NOT_EMPTY;}
  if(!mutable_revision())return OMNI_FILES_BUSY;const auto spelling=parent->second->path+'/'+leaf(to.text);auto candidate=nodes_;
  struct Move{std::string old_key,new_key,path;std::shared_ptr<Node> node;};std::vector<Move> moves;
  for(const auto& [key,node]:nodes_)if(key==from.key||key.starts_with(from.key+'/')){
    auto new_path=spelling+node->path.substr(source->second->path.size());const auto admitted=path(new_path.c_str());
    const auto new_key=to.key+key.substr(from.key.size());if(admitted.key!=new_key)return OMNI_FILES_INVALID;
    moves.push_back({key,new_key,std::move(new_path),node});
  }
  for(const auto& move:moves)candidate.erase(move.old_key);if(target!=nodes_.end()&&target!=source)candidate.erase(to.key);
  for(const auto& move:moves)if(!candidate.emplace(move.new_key,move.node).second)return OMNI_FILES_EXISTS;
  nodes_.swap(candidate);for(auto& move:moves)move.node->path.swap(move.path);++revision_;return OMNI_FILES_OK;
}
int MemoryFiles::list(const char* raw,std::uint64_t expected,std::uint32_t offset,OmniFilesEntryV1* out,std::uint32_t capacity,std::uint32_t* actual,std::uint64_t* revision){
  if(!actual||!revision||capacity>64||offset>maximum_nodes||(!out&&capacity))return OMNI_FILES_INVALID;*actual=0;const auto name=path(raw);std::lock_guard lock(mutex_);*revision=revision_;
  if(expected&&expected!=revision_)return OMNI_FILES_STALE;const auto parent=nodes_.find(name.key);if(parent==nodes_.end())return OMNI_FILES_NOT_FOUND;if(!parent->second->directory)return OMNI_FILES_IS_DIRECTORY;
  const auto prefix=name.key+'/';std::uint32_t skipped=0;
  for(auto it=nodes_.lower_bound(prefix);it!=nodes_.end()&&it->first.starts_with(prefix)&&*actual<capacity;++it){
    if(it->first.find('/',prefix.size())!=std::string::npos)continue;if(skipped++<offset)continue;
    auto& entry=out[*actual];entry={};describe(*it->second,entry.stat);const auto name=leaf(it->second->path);entry.name_bytes=static_cast<std::uint32_t>(name.size());std::memcpy(entry.name,name.data(),name.size());entry.name[name.size()]=0;++*actual;
  }return OMNI_FILES_OK;
}
int MemoryFiles::usage(OmniFilesUsageV1* out){if(!out)return OMNI_FILES_INVALID;std::lock_guard lock(mutex_);*out={budgets_[0].allocated,budgets_[1].allocated,revision_,budgets_[0].files+budgets_[1].files,budgets_[0].directories+budgets_[1].directories,static_cast<std::uint32_t>(handles_.size()),0};return OMNI_FILES_OK;}
OmniWritableFilesV1 MemoryFiles::interface()noexcept{
  return {1,static_cast<std::uint32_t>(sizeof(OmniWritableFilesV1)),this,
    &invoke<&MemoryFiles::stat,const char*,OmniFilesStatV1*>,&invoke<&MemoryFiles::mkdir,const char*,std::uint32_t>,
    &invoke<&MemoryFiles::open,const char*,std::uint32_t,std::uint64_t*>,&invoke<&MemoryFiles::close,std::uint64_t>,
    &invoke<&MemoryFiles::read,std::uint64_t,std::uint64_t,void*,std::uint64_t,std::uint64_t*>,
    &invoke<&MemoryFiles::write,std::uint64_t,std::uint64_t,const void*,std::uint64_t,std::uint64_t*>,
    &invoke<&MemoryFiles::resize,std::uint64_t,std::uint64_t>,&invoke<&MemoryFiles::flush,std::uint64_t>,
    &invoke<&MemoryFiles::remove,const char*>,&invoke<&MemoryFiles::rename,const char*,const char*,std::uint32_t>,
    &invoke<&MemoryFiles::list,const char*,std::uint64_t,std::uint32_t,OmniFilesEntryV1*,std::uint32_t,std::uint32_t*,std::uint64_t*>,
    &invoke<&MemoryFiles::usage,OmniFilesUsageV1*>};
}
std::vector<std::uint8_t> MemoryFiles::capture(const std::string& raw){
  const auto root=path(raw.c_str());if(root.root!=0||parent_key(root.key).empty())throw std::invalid_argument("save root");
  std::lock_guard lock(mutex_);const auto parent=nodes_.find(root.key);if(parent==nodes_.end()||!parent->second->directory)throw std::invalid_argument("save root");
  struct Item{std::string name;std::shared_ptr<Node> node;};std::vector<Item> items;std::uint64_t total=12;
  const auto prefix=root.key+'/';
  for(auto at=nodes_.lower_bound(prefix);at!=nodes_.end()&&at->first.starts_with(prefix);++at){
    const auto name=at->second->path.substr(parent->second->path.size()+1);total+=12+name.size()+at->second->bytes;
    if(items.size()>=8192||total>256ull*1024*1024)throw std::length_error("save archive");
    items.push_back({name,at->second});
  }
  std::sort(items.begin(),items.end(),[](const Item& a,const Item& b){return a.name<b.name;});
  std::vector<std::uint8_t> result;result.reserve(static_cast<std::size_t>(total));result.insert(result.end(),{'O','P','S','F','0','0','0','2'});
  auto number=[&](std::uint32_t value){for(unsigned n=0;n<4;++n)result.push_back(static_cast<std::uint8_t>(value>>(n*8)));};number(static_cast<std::uint32_t>(items.size()));
  for(const auto& item:items){const auto& node=*item.node;number(node.directory?1:0);number(static_cast<std::uint32_t>(item.name.size()));number(static_cast<std::uint32_t>(node.bytes));result.insert(result.end(),item.name.begin(),item.name.end());
    for(std::uint64_t offset=0;offset<node.bytes;offset+=page_bytes){const auto count=static_cast<std::size_t>(std::min<std::uint64_t>(node.bytes-offset,page_bytes));const auto& page=*node.pages[static_cast<std::size_t>(offset/page_bytes)];result.insert(result.end(),page.begin(),page.begin()+count);}
  }return result;
}
int MemoryFiles::replace(const std::string& raw,std::span<const MemoryFileEntry> entries){
  const auto root=path(raw.c_str());if(root.root!=0||parent_key(root.key).empty()||entries.size()>8192)return OMNI_FILES_INVALID;
  std::string canonical_root;std::uint64_t expected_revision=0;
  {std::lock_guard lock(mutex_);const auto found=nodes_.find(root.key);if(found==nodes_.end()||!found->second->directory)return OMNI_FILES_NOT_FOUND;if(!handles_.empty())return OMNI_FILES_BUSY;canonical_root=found->second->path;expected_revision=revision_;}
  // Full staging has its own finite save budget. The old store remains
  // unchanged until all content, metadata and namespace operations succeed.
  MemoryFiles stage;auto api=stage.interface();int result=api.mkdir(api.context,canonical_root.c_str(),1);if(result)return result;
  for(const auto& entry:entries){
    const auto target=canonical_root+'/'+entry.name;const auto checked=path(target.c_str());
    if(!checked.key.starts_with(root.key+'/')||entry.name.empty()||(entry.directory&&!entry.payload.empty()))return OMNI_FILES_INVALID;
    const auto split=target.rfind('/');result=api.mkdir(api.context,target.substr(0,split).c_str(),1);if(result)return result;
    if(entry.directory){result=api.mkdir(api.context,target.c_str(),1);if(result)return result;continue;}
    std::uint64_t handle=0;result=api.open(api.context,target.c_str(),OMNI_FILES_WRITE|OMNI_FILES_CREATE|OMNI_FILES_EXCLUSIVE,&handle);if(result)return result;
    for(std::size_t offset=0;offset<entry.payload.size();){const auto bytes=std::min<std::size_t>(OMNI_FILES_MAX_TRANSFER,entry.payload.size()-offset);std::uint64_t actual=0;
      result=api.write(api.context,handle,offset,entry.payload.data()+offset,bytes,&actual);if(result||actual!=bytes)return result?result:OMNI_FILES_INTERNAL;offset+=bytes;
    }
    result=api.close(api.context,handle);if(result)return result;
  }
  std::lock_guard lock(mutex_);if(!handles_.empty()||!mutable_revision()||revision_!=expected_revision)return OMNI_FILES_BUSY;
  const auto old_root=nodes_.find(root.key);if(old_root==nodes_.end()||!old_root->second->directory)return OMNI_FILES_NOT_FOUND;
  const auto prefix=root.key+'/';auto candidate=nodes_;std::uint64_t retired_bytes=0;std::uint32_t retired_files=0,retired_dirs=0;
  for(auto at=candidate.lower_bound(prefix);at!=candidate.end()&&at->first.starts_with(prefix);){const auto& node=*at->second;retired_bytes+=std::uint64_t(node.pages.size())*page_bytes;if(node.directory)++retired_dirs;else ++retired_files;at=candidate.erase(at);}
  std::uint64_t incoming_bytes=0;std::uint32_t incoming_files=0,incoming_dirs=0;
  for(auto at=stage.nodes_.lower_bound(prefix);at!=stage.nodes_.end()&&at->first.starts_with(prefix);++at){const auto& node=*at->second;incoming_bytes+=std::uint64_t(node.pages.size())*page_bytes;if(node.directory)++incoming_dirs;else ++incoming_files;candidate.emplace(at->first,at->second);}
  const auto files=budgets_[0].files+budgets_[1].files-retired_files+incoming_files;
  const auto dirs=budgets_[0].directories+budgets_[1].directories-retired_dirs+incoming_dirs;
  if(incoming_bytes>save_limit-(budgets_[0].allocated-retired_bytes)||files>maximum_files||files+dirs>maximum_nodes)return OMNI_FILES_NO_SPACE;
  // All allocations and rejection points precede ownership transfer. Move the
  // staged counters with each node so neither old/open/unlinked debt is lost.
  for(auto at=stage.nodes_.lower_bound(prefix);at!=stage.nodes_.end()&&at->first.starts_with(prefix);++at){auto& node=*at->second;
    const auto charge=std::uint64_t(node.pages.size())*page_bytes;node.budget->allocated-=charge;if(node.directory)--node.budget->directories;else --node.budget->files;
    node.budget=&budgets_[0];node.budget->allocated+=charge;if(node.directory)++node.budget->directories;else ++node.budget->files;
  }
  nodes_.swap(candidate);++revision_;return OMNI_FILES_OK;
}
}
