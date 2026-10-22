#ifndef OMNIPLAY_N64_FRAMEBUFFER_CACHE_HPP
#define OMNIPLAY_N64_FRAMEBUFFER_CACHE_HPP
#include <cstddef>
#include <utility>
/* A full observation cache must not suppress the underlying copy/write. */
template<class Array,class Pointer>
static std::pair<bool,unsigned> om_n64_find_framebuffer(const Array& buffers,Pointer buffer)
{
    for(std::size_t i=0;i<buffers.size();++i){
        if(buffers[i]==buffer)return {true,static_cast<unsigned>(i)};
        if(buffers[i]==nullptr)return {false,static_cast<unsigned>(i)};
    }
    return {false,static_cast<unsigned>(buffers.size())};
}
#endif
