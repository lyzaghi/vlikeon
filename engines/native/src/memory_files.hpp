#pragma once
#include "writable_files_v1.h"
#include <array>
#include <cstdint>
#include <map>
#include <memory>
#include <mutex>
#include <span>
#include <stdexcept>
#include <string>
#include <vector>

namespace omni {
struct MemoryFileEntry{std::string name;std::span<const std::uint8_t> payload;bool directory;};
class MemoryFiles {
  static constexpr std::size_t page_bytes=4096;
  // Keep the existing 256-MiB save-envelope admission. Page rounding can add at
  // most one page per admitted file, so save allocation includes 16 MiB slack.
  static constexpr std::uint64_t save_limit=272ull*1024*1024,scratch_limit=64ull*1024*1024,file_limit=256ull*1024*1024;
  // Preserve 8192 archive descendants plus private root/ancestor metadata.
  static constexpr std::uint32_t maximum_files=4096,maximum_nodes=8224,maximum_handles=256;
  using Page=std::unique_ptr<std::array<std::uint8_t,page_bytes>>;
  struct Budget{std::uint64_t allocated=0,limit=0;std::uint32_t files=0,directories=0;};
  struct Node{
    Budget* budget;std::string path;bool directory;std::uint64_t bytes=0;std::vector<Page> pages;
    Node(Budget*,std::string,bool);~Node();
  };
  struct Path{std::string text,key;unsigned root;};
  struct Handle{std::shared_ptr<Node> node;std::uint32_t mode;};
  std::mutex mutex_;
  std::array<Budget,2> budgets_;
  std::map<std::string,std::shared_ptr<Node>> nodes_;
  std::map<std::uint64_t,Handle> handles_;
  std::uint64_t next_handle_=1,revision_=1;
  static Path path(const char*);
  static std::string folded(const std::string&);
  static std::string parent_key(const std::string&);
  static std::string leaf(const std::string&);
  bool room_node(bool directory)const;
  bool mutable_revision()const;
  bool children(const std::string&)const;
  int grow(Node&,std::uint64_t);
  static void shrink(Node&,std::uint64_t);
  static void zero(Node&,std::uint64_t,std::uint64_t);
  static void describe(const Node&,OmniFilesStatV1&);
  int stat(const char*,OmniFilesStatV1*);
  int mkdir(const char*,std::uint32_t);
  int open(const char*,std::uint32_t,std::uint64_t*);
  int close(std::uint64_t);
  int read(std::uint64_t,std::uint64_t,void*,std::uint64_t,std::uint64_t*);
  int write(std::uint64_t,std::uint64_t,const void*,std::uint64_t,std::uint64_t*);
  int resize(std::uint64_t,std::uint64_t);
  int flush(std::uint64_t);
  int remove(const char*);
  int rename(const char*,const char*,std::uint32_t);
  int list(const char*,std::uint64_t,std::uint32_t,OmniFilesEntryV1*,std::uint32_t,std::uint32_t*,std::uint64_t*);
  int usage(OmniFilesUsageV1*);
  template<auto Method,class... Args>static int invoke(void* context,Args... args)noexcept{
    if(!context)return OMNI_FILES_INVALID;
    try{return (static_cast<MemoryFiles*>(context)->*Method)(args...);}
    catch(const std::bad_alloc&){return OMNI_FILES_NO_MEMORY;}
    catch(const std::invalid_argument&){return OMNI_FILES_INVALID;}
    catch(...){return OMNI_FILES_INTERNAL;}
  }
public:
  MemoryFiles();~MemoryFiles()=default;
  MemoryFiles(const MemoryFiles&)=delete;MemoryFiles& operator=(const MemoryFiles&)=delete;
  MemoryFiles(MemoryFiles&&)=delete;MemoryFiles& operator=(MemoryFiles&&)=delete;
  OmniWritableFilesV1 interface()noexcept;
  std::vector<std::uint8_t> capture(const std::string& root);
  int replace(const std::string& root,std::span<const MemoryFileEntry> entries);
};
}
