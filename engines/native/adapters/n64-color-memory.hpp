#ifndef OMNIPLAY_N64_COLOR_MEMORY_HPP
#define OMNIPLAY_N64_COLOR_MEMORY_HPP
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
/* Exact byte coverage for the whole maximum RDRAM. Duplicate writes consume
 * no extra storage; writes for other framebuffers survive a partial consume. */
class OmniN64DirtyBytes {
    std::array<std::uint64_t,0x800000/64> bits_{};
    static std::uint64_t mask(unsigned int low,unsigned int high)
    {return (~UINT64_C(0)<<low)&(~UINT64_C(0)>>(64-high));}
public:
    static constexpr std::size_t capacity=0x800000;
    bool range(std::size_t at,std::size_t count)const{return at<=capacity && count<=capacity-at;}
    bool mark(std::size_t at,std::size_t count){if(!range(at,count))return false;change(at,count,true);return true;}
    void clear(std::size_t at,std::size_t count){if(range(at,count))change(at,count,false);}
    void clear(){bits_.fill(0);}
    bool any(std::size_t at,std::size_t count)const{
        if(!range(at,count) || !count)return false;
        const std::size_t end=at+count;
        while(at<end){auto word=at/64;unsigned int low=static_cast<unsigned int>(at%64);
            auto next=(word+1)*64;auto stop=end<next?end:next;
            if(bits_[word]&mask(low,static_cast<unsigned int>(stop-word*64)))return true;
            at=stop;
        }return false;
    }
private:
    void change(std::size_t at,std::size_t count,bool set){const std::size_t end=at+count;
        while(at<end){auto word=at/64;unsigned int low=static_cast<unsigned int>(at%64);
            auto next=(word+1)*64;auto stop=end<next?end:next;
            auto value=mask(low,static_cast<unsigned int>(stop-word*64));
            if(set)bits_[word]|=value;else bits_[word]&=~value;at=stop;
        }
    }
};
static inline bool om_n64_color_extent(std::size_t ram,std::uint32_t start,std::uint32_t width,
    std::uint32_t height,std::uint32_t pixel_bytes,std::size_t& pixels,std::size_t& source_bytes)
{
    if(!ram || ram>0x800000 || (ram&3) || (pixel_bytes!=2 && pixel_bytes!=4)
        || start>=ram || start%pixel_bytes || !width || !height || width>(ram-start)/pixel_bytes)return false;
    std::size_t row=static_cast<std::size_t>(width)*pixel_bytes;
    if(height>(ram-start)/row)return false;
    pixels=static_cast<std::size_t>(width)*height;source_bytes=pixels*pixel_bytes;return true;
}
/* This is the actual selected CPU pixel conversion, independent of a GL
 * context. Keep the pinned full/partial halfword XOR mapping. */
template<class Source>
static bool om_n64_color_copy(const std::uint8_t* ram,std::size_t ram_bytes,std::uint32_t start,
    std::uint32_t width,std::uint32_t height,const OmniN64DirtyBytes* dirty,
    std::uint32_t* output,std::size_t output_pixels,std::uint32_t(*convert)(Source,bool),
    unsigned int pixel_xor,bool full_alpha,bool& nonzero)
{
    std::size_t pixels=0,source_bytes=0;
    if(!ram || !output || !convert || pixel_xor!=(sizeof(Source)==2?1u:0u)
        || !om_n64_color_extent(ram_bytes,start,width,height,sizeof(Source),pixels,source_bytes)
        || pixels>output_pixels)return false;
    if(!dirty){
        auto last=(pixels-1)|pixel_xor;
        if(last>=(ram_bytes-start)/sizeof(Source))return false;
    }
    std::memset(output,0,pixels*sizeof(*output));nonzero=false;
    for(std::size_t i=0;i<pixels;++i){
        if(dirty && !dirty->any(start+i*sizeof(Source),sizeof(Source)))continue;
        std::size_t from=dirty?i:(i^pixel_xor),to=dirty?(i^pixel_xor):i;
        if(to>=pixels)return false; // unmatched final halfword: retain pending writes
        Source value;std::memcpy(&value,ram+start+from*sizeof(Source),sizeof(value));
        output[to]=convert(value,full_alpha);nonzero=nonzero || value!=0;
    }
    return true;
}
#endif
