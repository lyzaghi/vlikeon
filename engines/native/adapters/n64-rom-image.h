#ifndef OMNIPLAY_N64_ROM_IMAGE_H
#define OMNIPLAY_N64_ROM_IMAGE_H
#include <stddef.h>
#include <stdint.h>
#include <string.h>
/* Selected native profile already caps content at 512 MiB. Boot HLE copies
 * [0x40,0x1000); main later swaps whole words and stores length in signed int. */
static inline int om_n64_rom_image_format(const void* image,size_t bytes)
{
    const uint8_t* data=(const uint8_t*)image;
    if(!data || bytes<0x1000 || bytes>512u*1024u*1024u || (bytes&3))return -1;
    if(data[0]==0x80 && data[1]==0x37 && data[2]==0x12 && data[3]==0x40)return 0;
    if(data[0]==0x37 && data[1]==0x80 && data[2]==0x40 && data[3]==0x12)return 1;
    if(data[0]==0x40 && data[1]==0x12 && data[2]==0x37 && data[3]==0x80)return 2;
    return -1;
}
/* Each four-byte word is staged before copying; no alignment assumptions,
 * typed aliasing or endian-dependent integer loads. Exact in-place works. */
static inline int om_n64_rom_image_copy(void* output,size_t capacity,const void* image,size_t bytes,unsigned char* format)
{
    int kind=om_n64_rom_image_format(image,bytes);size_t at;uint8_t* dst=(uint8_t*)output;
    const uint8_t* src=(const uint8_t*)image;
    if(kind<0 || !dst || !format || capacity<bytes)return 0;
    for(at=0;at<bytes;at+=4){uint8_t word[4];unsigned i;
        memcpy(word,src+at,4);
        for(i=0;i<4;++i)dst[at+i]=word[i^(kind==1?1:(kind==2?3:0))];
    }
    *format=(unsigned char)kind;return 1;
}
#endif
