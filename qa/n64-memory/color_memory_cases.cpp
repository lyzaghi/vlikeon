// Source-only CPU fixtures: no renderer, driver, core or GL context is started.
#include "n64-color-memory.hpp"
#include <algorithm>
#include <array>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <vector>
using u16=std::uint16_t;using u32=std::uint32_t;
#include "n64-color-converters.cpp.inc"
static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
static void exact_coverage_and_partial_consumption(){
    static_assert(sizeof(OmniN64DirtyBytes)==1048576,"fixed one-MiB exact bitmap");
    std::unique_ptr<OmniN64DirtyBytes> dirty(new OmniN64DirtyBytes);
    require(!dirty->any(0,0x800000),"cold bitmap empty");
    for(unsigned i=0;i<200000;++i)require(dirty->mark(0x1000,4),"duplicate byte coverage admitted");
    require(dirty->mark(0x7fffff,1)&&dirty->any(0x1000,4)&&dirty->any(0x7fffff,1),"first and second framebuffer tracked independently");
    dirty->clear(0x1000,4);require(!dirty->any(0,0x700000)&&dirty->any(0x7fffff,1),"first frame consume preserves other pending frame");
    require(!dirty->mark(0x7fffff,2)&&!dirty->mark(UINT32_MAX,2)&&dirty->any(0x7fffff,1),"invalid complete mark never changes bitmap");
    dirty->clear();require(dirty->mark(0,0x800000)&&dirty->any(0,1)&&dirty->any(0x7fffff,1),"maximum RAM coverage is finite and exact");
    dirty->clear(1,0x7ffffe);require(dirty->any(0,1)&&dirty->any(0x7fffff,1)&&!dirty->any(1,0x7ffffe),"range masks preserve edge bits across all words");
    dirty->clear();std::array<bool,257> oracle{};
    for(unsigned at=0;at<257;++at){auto count=std::min<unsigned>(257-at,(at*37)%73);dirty->mark(at,count);std::fill(oracle.begin()+at,oracle.begin()+at+count,true);
        auto erase=std::min<unsigned>(257-at,(at*11)%39);dirty->clear(at,erase);std::fill(oracle.begin()+at,oracle.begin()+at+erase,false);
        for(unsigned i=0;i<257;++i)require(dirty->any(i,1)==oracle[i],"word-boundary range operations match independent byte oracle");}
}
static void actual_converters_and_pixel_bounds(){
    std::unique_ptr<OmniN64DirtyBytes> dirty(new OmniN64DirtyBytes);
    std::array<u16,8> source16{{0xf801,0x07c1,0x003f,0xffff,0,0x0001,0xf800,0x07c0}};
    std::array<u32,10> output;output.fill(0xa5a5a5a5);bool nonzero=false;
    require(om_n64_color_copy<u16>(reinterpret_cast<const std::uint8_t*>(source16.data()),sizeof(source16),0,4,2,nullptr,output.data()+1,8,RGBA16ToABGR32,1,false,nonzero),"full 16-bit image admitted");
    require(output.front()==0xa5a5a5a5&&output.back()==0xa5a5a5a5&&output[1]==0xff00f800&&output[2]==0xff0000f8&&output[3]==0xfff8f8f8&&output[4]==0xfff80000&&nonzero,"pinned halfword order and exact known color values with output guards");
    dirty->mark(0,4);output.fill(0xa5a5a5a5);
    require(om_n64_color_copy<u16>(reinterpret_cast<const std::uint8_t*>(source16.data()),sizeof(source16),0,4,2,dirty.get(),output.data()+1,8,RGBA16ToABGR32,1,false,nonzero),"four-byte write updates both halfword pixels");
    require(output[1]==0xff00f800&&output[2]==0xff0000f8&&std::all_of(output.begin()+3,output.end()-1,[](auto v){return v==0;})&&dirty->any(0,4),"partial output is transparent elsewhere and conversion does not consume retry coverage");
    std::array<u32,4> source32{{0x11223344,0xaabbccdd,UINT32_MAX,1}};dirty->clear();dirty->mark(5,1);
    require(om_n64_color_copy<u32>(reinterpret_cast<const std::uint8_t*>(source32.data()),sizeof(source32),0,2,2,dirty.get(),output.data()+1,4,RGBA32ToABGR32,0,false,nonzero),"single-byte write selects its complete 32-bit pixel");
    require(output[1]==0&&output[2]==0xddccbbaa&&output[3]==0&&output[4]==0,"32-bit channel conversion is independent of marker byte within pixel");
    require(om_n64_color_copy<u32>(reinterpret_cast<const std::uint8_t*>(source32.data()),sizeof(source32),8,2,1,nullptr,output.data()+1,2,RGBA32ToABGR32,0,false,nonzero)&&nonzero,"nonzero decision cannot wrap to zero from summed pixels");
    output.fill(0xa5a5a5a5);const auto before=output;
    require(!om_n64_color_copy<u16>(reinterpret_cast<const std::uint8_t*>(source16.data()),sizeof(source16),14,1,1,nullptr,output.data()+1,1,RGBA16ToABGR32,1,false,nonzero)&&output==before,"last unpaired full halfword rejected before output mutation");
    require(!om_n64_color_copy<u32>(reinterpret_cast<const std::uint8_t*>(source32.data()),sizeof(source32),0,2,2,nullptr,output.data()+1,3,RGBA32ToABGR32,0,false,nonzero)&&output==before,"complete output capacity required before clearing output");
    dirty->clear();dirty->mark(4,2);require(!om_n64_color_copy<u16>(reinterpret_cast<const std::uint8_t*>(source16.data()),sizeof(source16),0,3,1,dirty.get(),output.data()+1,3,RGBA16ToABGR32,1,false,nonzero)&&dirty->any(4,2),"unmatched partial halfword cannot overflow destination or lose retry coverage");
}
static void geometry_and_black_pixels(){
    std::size_t pixels=123,bytes=456;
    require(om_n64_color_extent(0x800000,0x7ffff8,2,1,4,pixels,bytes)&&pixels==2&&bytes==8,"exact 8-MiB end");
    pixels=123;bytes=456;require(!om_n64_color_extent(0x800000,0,UINT32_MAX,UINT32_MAX,4,pixels,bytes)&&pixels==123&&bytes==456,"overflow products rejected without output commit");
    require(!om_n64_color_extent(0x400000,0x400000,1,1,2,pixels,bytes)&&!om_n64_color_extent(0x800004,0,1,1,4,pixels,bytes)&&!om_n64_color_extent(16,0,0,1,4,pixels,bytes)&&!om_n64_color_extent(16,2,1,1,4,pixels,bytes),"actual capacity, nonzero width and aligned pixel base");
    std::array<u32,4> zero{};std::array<u32,4> output{};bool nonzero=true;
    require(om_n64_color_copy<u32>(reinterpret_cast<const std::uint8_t*>(zero.data()),sizeof(zero),0,2,2,nullptr,output.data(),4,RGBA32ToABGR32,0,true,nonzero)&&!nonzero&&std::all_of(output.begin(),output.end(),[](auto v){return v==0xff000000;}),"full-alpha black is valid opaque black, independently of nonzero flag");
}
int main(){try{exact_coverage_and_partial_consumption();actual_converters_and_pixel_bounds();geometry_and_black_pixels();std::cout<<"Color memory source cases passed\n";return 0;}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
