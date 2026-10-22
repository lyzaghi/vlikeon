#pragma once
#include <array>
#include <cstddef>
#include <cstdint>
#include <memory>
#include <mutex>
#include <thread>
#include <utility>

namespace om_n64_texture {
struct PoolAllocator {
  std::unique_ptr<std::uint32_t[]> operator()(std::size_t count)const{
    return std::unique_ptr<std::uint32_t[]>(new std::uint32_t[count]());
  }
};

// Preserve the pinned filter's 4096 limit and one actual processing lane.
// Main buffers never move while initialized. Workspace growth stages a whole
// pair, with the old pair retained until both allocations succeed. No OS file.
template<class Allocator=PoolAllocator>class BasicTexturePool {
  struct Buffer {std::unique_ptr<std::uint32_t[]> data;std::uint32_t count=0;};
  Allocator allocator_;mutable std::mutex mutex_;
  std::array<Buffer,2> shared_,work_;
  std::uint32_t width_=0,height_=0,pixels_=0,depth_=0,work_request_=0;
  std::thread::id owner_;
  bool active_=false,work_live_=false,retiring_=false;
  std::uint64_t live_bytes_=0;
  bool ours()const{return depth_&&owner_==std::this_thread::get_id();}
public:
  static constexpr std::uint32_t maximum_dimension=4096,maximum_depth=16;
  static constexpr std::uint64_t maximum_live_bytes=256ull*1024*1024;
  static constexpr std::uint64_t maximum_transient_bytes=384ull*1024*1024;
  struct Stats {std::uint64_t live_bytes=0;std::uint32_t width=0,height=0,users=0,workspace_pixels=0;bool active=false,workspace_live=false,retiring=false;};
  explicit BasicTexturePool(Allocator allocator=Allocator()):allocator_(std::move(allocator)){}
  BasicTexturePool(const BasicTexturePool&)=delete;BasicTexturePool& operator=(const BasicTexturePool&)=delete;

  bool init(int width,int height)noexcept{
    try{
      if(width<=0||height<=0||width>int(maximum_dimension)||height>int(maximum_dimension))return false;
      const auto count=std::uint64_t(width)*static_cast<unsigned>(height);
      if(count*8>maximum_live_bytes)return false;
      std::lock_guard<std::mutex> lock(mutex_);
      if(active_||depth_||work_live_)return false;
      auto first=allocator_(static_cast<std::size_t>(count));if(!first)return false;
      auto second=allocator_(static_cast<std::size_t>(count));if(!second)return false;
      shared_[0].data=std::move(first);shared_[1].data=std::move(second);
      shared_[0].count=shared_[1].count=static_cast<std::uint32_t>(count);
      width_=static_cast<unsigned>(width);height_=static_cast<unsigned>(height);pixels_=static_cast<std::uint32_t>(count);
      live_bytes_=count*8;active_=true;retiring_=false;return true;
    }catch(...){return false;}
  }
  bool shutdown()noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);if(depth_||work_live_)return false;
      for(auto& buffer:work_){buffer.data.reset();buffer.count=0;}
      for(auto& buffer:shared_){buffer.data.reset();buffer.count=0;}
      width_=height_=pixels_=work_request_=0;owner_=std::thread::id();active_=false;retiring_=false;live_bytes_=0;return true;
    }catch(...){return false;}
  }
  bool retired()const noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);return !active_&&!depth_&&!work_live_&&!live_bytes_;}catch(...){return false;}
  }
  bool request_retirement()noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);retiring_=true;return !depth_&&!work_live_;}catch(...){return false;}
  }
  bool begin()noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);if(!active_||(!depth_&&retiring_)||depth_>=maximum_depth||(depth_&&!ours()))return false;
      if(!depth_)owner_=std::this_thread::get_id();++depth_;return true;
    }catch(...){return false;}
  }
  void end()noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);if(!ours()||(!depth_)||(depth_==1&&work_live_))return;
      if(!--depth_)owner_=std::thread::id();
    }catch(...){}
  }
  bool begin_work()noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);if(!ours()||work_live_)return false;work_live_=true;work_request_=0;return true;}catch(...){return false;}
  }
  void end_work()noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);if(ours()&&work_live_){work_live_=false;work_request_=0;}}catch(...){}
  }
  std::uint8_t* shared(unsigned slot)noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);return active_&&slot<2?reinterpret_cast<std::uint8_t*>(shared_[slot].data.get()):nullptr;}catch(...){return nullptr;}
  }
  std::uint32_t size(unsigned slot)const noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);return active_&&slot<2?shared_[slot].count*4:0;}catch(...){return 0;}
  }
  std::uint32_t* workspace(unsigned lane,unsigned slot,std::uint32_t count)noexcept{
    try{
      std::lock_guard<std::mutex> lock(mutex_);
      if(!active_||!ours()||!work_live_||lane!=0||slot>=2||!count||count>pixels_||(work_request_&&count>work_request_))return nullptr;
      if(!work_request_){
        if(count>work_[0].count){
          const auto charge=std::uint64_t(count)*8;
          if(live_bytes_>maximum_transient_bytes||charge>maximum_transient_bytes-live_bytes_)return nullptr;
          auto first=allocator_(count);if(!first)return nullptr;auto second=allocator_(count);if(!second)return nullptr;
          const auto prior=std::uint64_t(work_[0].count+work_[1].count)*4;
          const auto final=live_bytes_-prior+charge;if(final>maximum_live_bytes)return nullptr;
          work_[0].data.swap(first);work_[1].data.swap(second);work_[0].count=work_[1].count=count;live_bytes_=final;
        }
        work_request_=count;
      }
      return work_[slot].data.get();
    }catch(...){return nullptr;}
  }
  Stats stats()const noexcept{
    try{std::lock_guard<std::mutex> lock(mutex_);Stats value;value.live_bytes=live_bytes_;value.width=width_;value.height=height_;value.users=depth_;
      value.workspace_pixels=work_[0].count;value.active=active_;value.workspace_live=work_live_;value.retiring=retiring_;return value;
    }catch(...){return Stats();}
  }
  class Use {
    BasicTexturePool* pool_=nullptr;
  public:
    explicit Use(BasicTexturePool& pool){if(pool.begin())pool_=&pool;}
    ~Use(){if(pool_)pool_->end();}
    Use(const Use&)=delete;Use& operator=(const Use&)=delete;
    explicit operator bool()const{return pool_!=nullptr;}
  };
  class Work {
    BasicTexturePool* pool_=nullptr;
  public:
    explicit Work(BasicTexturePool& pool){if(pool.begin_work())pool_=&pool;}
    ~Work(){if(pool_)pool_->end_work();}
    Work(const Work&)=delete;Work& operator=(const Work&)=delete;
    explicit operator bool()const{return pool_!=nullptr;}
  };
};
using TexturePool=BasicTexturePool<>;
TexturePool& pool();
}
extern "C" int om_n64_texture_pool_retired(void);
