/* Actual selected GB storage helpers. Complete-range access; a failed read is
 * deterministic open-bus data. No 16-bit truncation of banked RAM offsets. */
#include "n64-rtc-storage.h"
typedef char om_n64_gb_rtc_device_contract[(GED_RTC==OM_N64_GB_RTC_DEVICE)?1:-1];
static const uint8_t* om_n64_gb_read_range(const void* storage,const struct storage_backend_interface* api,uint32_t address,size_t bytes)
{
    size_t length;const uint8_t* data;
    if(!storage || !api || !api->size || !api->data)return NULL;
    length=api->size(storage);
    if(address>length || bytes>length-address)return NULL;
    data=api->data(storage);return data?data+address:NULL;
}
static void read_rom(const void* storage,const struct storage_backend_interface* api,uint32_t address,uint8_t* data,size_t bytes)
{
    const uint8_t* source=om_n64_gb_read_range(storage,api,address,bytes);
    if(source)memcpy(data,source,bytes);else memset(data,0xff,bytes);
}
static void read_ram(const void* storage,const struct storage_backend_interface* api,unsigned int enabled,uint32_t address,uint8_t* data,size_t bytes,uint8_t mask)
{
    size_t i;const uint8_t* source=enabled?om_n64_gb_read_range(storage,api,address,bytes):NULL;
    if(!source){memset(data,0xff,bytes);return;}
    for(i=0;i<bytes;++i)data[i]=source[i]&mask;
}
static void write_ram(void* storage,const struct storage_backend_interface* api,unsigned int enabled,uint32_t address,const uint8_t* data,size_t bytes,uint8_t mask)
{
    size_t length,i;uint8_t* destination;
    if(!enabled || !storage || !api || !api->size || !api->data || !api->save)return;
    length=api->size(storage);
    if(address>length || bytes>length-address)return;
    destination=api->data(storage);if(!destination)return;
    for(i=0;i<bytes;++i)destination[address+i]=data[i]&mask;
    api->save(storage,address,bytes);
}
int read_gb_cart(struct gb_cart* cart,uint16_t address,uint8_t* data,size_t bytes)
{
    if(!om_n64_owner_access() || !data || !bytes || bytes>32)return -1;
    memset(data,0xff,bytes);
    if(!cart || !cart->read_gb_cart || bytes>0x2000u-(address&0x1fffu)
       || cart->rom_bank>0x1ff || cart->ram_bank>0xff || cart->ram_enable>1 || cart->mbc1_mode>1)return -1;
    return cart->read_gb_cart(cart,address,data,bytes);
}
int write_gb_cart(struct gb_cart* cart,uint16_t address,const uint8_t* data,size_t bytes)
{
    if(!om_n64_owner_access() || !cart || !data || !bytes || bytes>32 || !cart->write_gb_cart
       || bytes>0x2000u-(address&0x1fffu) || cart->rom_bank>0x1ff || cart->ram_bank>0xff
       || cart->ram_enable>1 || cart->mbc1_mode>1)return -1;
    return cart->write_gb_cart(cart,address,data,bytes);
}
