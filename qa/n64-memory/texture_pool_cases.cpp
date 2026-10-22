// Authored only; compile actual C++11 pool/TxMemBuf, execute in final item 16.
#include "n64-texture-pool.hpp"
#include "TxUtil.h"
#include <atomic>
#include <iostream>
#include <limits>
#include <memory>
#include <stdexcept>
#include <thread>
#include <vector>

static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
struct AllocationState {unsigned calls=0,fail=0;std::vector<std::size_t> requests;};
struct FaultAllocator {
  AllocationState* state;
  std::unique_ptr<std::uint32_t[]> operator()(std::size_t count){
    ++state->calls;state->requests.push_back(count);if(state->calls==state->fail)throw std::bad_alloc();
    return std::unique_ptr<std::uint32_t[]>(new std::uint32_t[count]());
  }
};
using TestPool=om_n64_texture::BasicTexturePool<FaultAllocator>;

static void init_and_growth_allocation_failures(){
  AllocationState state;TestPool pool(FaultAllocator{&state});
  require(pool.retired()&&!pool.init(0,1)&&!pool.init(-1,1)&&!pool.init(4097,1)&&!pool.init(std::numeric_limits<int>::max(),std::numeric_limits<int>::max()),"invalid dimensions rejected before allocation");
  require(state.calls==0,"overflow inputs never allocate");
  for(unsigned at=1;at<=2;++at){state.fail=state.calls+at;require(!pool.init(32,32)&&pool.retired()&&pool.stats().live_bytes==0,"partial init keeps empty owner");}
  state.fail=0;require(pool.init(32,32)&&pool.size(0)==4096&&pool.size(1)==4096,"complete pair init");
  auto* root=pool.shared(0);root[0]=0x41;
  require(!pool.init(16,16)&&pool.shared(0)==root&&root[0]==0x41,"reinit cannot invalidate published main buffers");
  std::uint32_t* previous=nullptr;
  {
    TestPool::Use use(pool);require(bool(use),"owned processing scope");TestPool::Work work(pool);require(bool(work),"owned workspace scope");
    previous=pool.workspace(0,0,64);auto* paired=pool.workspace(0,1,64);
    require(previous&&paired&&previous!=paired,"complete independent workspace pair");previous[0]=0x1234;paired[0]=0x5678;
    require(!pool.workspace(0,1,128)&&previous[0]==0x1234,"larger borrow cannot invalidate a pointer in the same workspace scope");
    require(!pool.workspace(1,0,64)&&!pool.workspace(0,2,64)&&!pool.workspace(0,0,1025),"lane/slot/count bounds");
    require(!pool.shutdown()&&!pool.retired()&&root[0]==0x41,"active scope prevents release");
  }
  for(unsigned at=1;at<=2;++at){
    TestPool::Use use(pool);TestPool::Work work(pool);state.fail=state.calls+at;
    require(!pool.workspace(0,0,128)&&pool.stats().workspace_pixels==64,"failed pair growth preserves committed workspace size");
    require(pool.workspace(0,0,64)==previous&&previous[0]==0x1234&&pool.workspace(0,1,64)[0]==0x5678,"failed growth retains both old pointer images");
  }
  state.fail=0;
  {TestPool::Use use(pool);TestPool::Work work(pool);require(pool.workspace(0,0,128)&&pool.workspace(0,1,128)&&pool.stats().workspace_pixels==128,"later independent scope can grow successfully");}
  require(pool.stats().live_bytes==2*4096+2*128*4&&pool.shutdown()&&pool.retired(),"exact live-byte accounting and confirmed release");
}

static void thread_ownership_retirement_and_nested_bounds(){
  AllocationState state;TestPool pool(FaultAllocator{&state});require(pool.init(16,16),"ownership fixture init");
  {
    TestPool::Use use(pool);require(bool(use),"primary owner");TestPool::Work work(pool);require(bool(work),"primary workspace");
    auto* data=pool.workspace(0,0,32);require(data,"owned workspace");data[0]=0x91;
    std::atomic<bool> foreign_rejected{false};
    std::jthread other([&]{TestPool::Use denied(pool);pool.end();pool.end_work();foreign_rejected=!denied&&!pool.workspace(0,1,32)&&!pool.shutdown();});other.join();
    require(foreign_rejected&&data[0]==0x91&&pool.stats().users==1&&pool.stats().workspace_live,"foreign operations cannot release another processing scope");
    require(!pool.request_retirement()&&pool.stats().retiring&&data[0]==0x91,"retirement closes future root admission and retains active bytes");
  }
  {TestPool::Use denied(pool);require(!denied,"new work cannot begin after retirement requested");}
  require(pool.request_retirement()&&pool.shutdown()&&pool.retired(),"actual end permits retirement completion");
  require(pool.init(16,16),"next fully retired lifecycle can initialize");
  std::vector<std::unique_ptr<TestPool::Use>> scopes;
  for(unsigned n=0;n<TestPool::maximum_depth;++n){std::unique_ptr<TestPool::Use> scope(new TestPool::Use(pool));require(bool(*scope),"bounded nested owner scope");scopes.push_back(std::move(scope));}
  {TestPool::Use denied(pool);require(!denied&&pool.stats().users==TestPool::maximum_depth,"nesting bound cannot wrap users");}
  scopes.clear();require(pool.shutdown()&&pool.retired(),"all nested guards retire");
}

static void actual_adapter_and_full_buffer_ceiling(){
  auto* buffers=TxMemBuf::getInstance();require(om_n64_texture_pool_retired()==1,"real pool initially retired");
  require(!buffers->init(0,4096)&&!buffers->get(0)&&!buffers->get(2)&&buffers->size_of(2)==0,"real TxMemBuf rejects absent/out-of-range buffers");
  require(buffers->init(4096,4096)&&buffers->size_of(0)==64u*1024u*1024u&&buffers->size_of(1)==64u*1024u*1024u,"pinned maximum main buffers use checked products");
  require(!buffers->getThreadBuf(0,0,1),"workspace cannot be borrowed outside a processing scope");
  {
    om_n64_texture::TexturePool::Use use(om_n64_texture::pool());om_n64_texture::TexturePool::Work work(om_n64_texture::pool());
    auto* first=buffers->getThreadBuf(0,0,4096u*4096u);auto* second=buffers->getThreadBuf(0,1,4096u*4096u);
    require(use&&work&&first&&second&&om_n64_texture::pool().stats().live_bytes==om_n64_texture::TexturePool::maximum_live_bytes,"shared and workspace allocation obey the full 256-MiB live ceiling");
    first[0]=0x13;second[4096u*4096u-1]=0x91;
    buffers->shutdown();require(om_n64_texture_pool_retired()==0&&first[0]==0x13&&second[4096u*4096u-1]==0x91,"void adapter cannot pretend a busy pool was released");
  }
  buffers->shutdown();require(om_n64_texture_pool_retired()==1&&!buffers->get(0),"real adapter confirms final retirement");
}

int main(){try{init_and_growth_allocation_failures();thread_ownership_retirement_and_nested_bounds();actual_adapter_and_full_buffer_ceiling();
  std::cout<<"N64 bounded texture pool cases passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
