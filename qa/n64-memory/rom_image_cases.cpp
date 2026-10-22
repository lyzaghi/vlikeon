// Authored only; the production admission/conversion helper, no core execution.
#include "n64-rom-image.h"
#include <algorithm>
#include <array>
#include <iostream>
#include <stdexcept>
#include <vector>
static void require(bool ok,const char* why){if(!ok)throw std::runtime_error(why);}
int main(){try{
    require(om_n64_rom_image_format(nullptr,0)<0,"null input");
    std::array<unsigned char,4> signature{{0x80,0x37,0x12,0x40}};
    for(size_t bytes=0;bytes<4;++bytes)require(om_n64_rom_image_format(signature.data(),bytes)<0,"short signature never read");
    require(om_n64_rom_image_format(signature.data(),512u*1024u*1024u+4)<0,"oversized input rejected before signature access");
    std::vector<unsigned char> canonical(0x1004);for(size_t i=0;i<canonical.size();++i)canonical[i]=static_cast<unsigned char>(i*17+3);
    std::copy(signature.begin(),signature.end(),canonical.begin());
    for(unsigned kind=0;kind<3;++kind){
        std::vector<unsigned char> input(canonical.size()+2,0xa5),output(canonical.size()+2,0x69);
        for(size_t i=0;i<canonical.size();++i)input[1+i]=canonical[i^(kind==1?1:(kind==2?3:0))];
        unsigned char format=99;
        require(om_n64_rom_image_format(input.data()+1,canonical.size())==static_cast<int>(kind),"actual input order identified");
        require(om_n64_rom_image_copy(output.data()+1,canonical.size(),input.data()+1,canonical.size(),&format)&&format==kind,"unaligned byte conversion admitted");
        require(std::equal(canonical.begin(),canonical.end(),output.begin()+1)&&output.front()==0x69&&output.back()==0x69,"all converted bytes and guard bytes exact");
        auto retained=output;format=99;
        require(!om_n64_rom_image_copy(output.data()+1,canonical.size()-1,input.data()+1,canonical.size(),&format)&&output==retained&&format==99,"short output cannot partially write or commit format");
        require(om_n64_rom_image_copy(input.data()+1,canonical.size(),input.data()+1,canonical.size(),&format)&&std::equal(canonical.begin(),canonical.end(),input.begin()+1),"exact in-place conversion stages words");
    }
    for(size_t bytes: {size_t(4),size_t(0x40),size_t(0xffc),size_t(0x1001),size_t(0x1002),size_t(0x1003)})
        require(om_n64_rom_image_format(canonical.data(),bytes)<0,"truncated boot or incomplete native word rejected");
    require(om_n64_rom_image_format(canonical.data(),0x1000)==0,"complete minimum boot image admitted");
    canonical[0]=0;require(om_n64_rom_image_format(canonical.data(),canonical.size())<0,"unknown signature rejected");
    std::cout<<"ROM image source cases passed\n";return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}}
