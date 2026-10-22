/* Called after actual GB construction and before power-on/guest exposure.
 * The ROM-content/physical-port name also works for RTC-only cartridges. */
static int om_n64_bind_gb_clock(struct gb_cart_data* data,struct gb_cart* cart)
{
    char* path;size_t length;
    if(!cart->read_gb_cart || !(cart->extra_devices&OM_N64_GB_RTC_DEVICE))return 1;
    if(!cart->rtc.iclock)goto rejected;
    path=get_gb_ram_path(data->rom_fstorage.data,data->rom_fstorage.size,(unsigned int)data->control_id+1);
    if(!path)goto rejected;
    length=strlen(path);
    if(length<4){free(path);goto rejected;}
    memcpy(path+length-4,".rtc",5);
    if(!om_n64_rtc_storage_bind(&cart->rtc,path)){free(path);goto rejected;}
    free(path);data->rtc=&cart->rtc;return 1;
rejected:
    om_n64_owner_admission_failed();
    /* No guest used these buffers. Bad RTC data cannot publish default RAM. */
    om_n64_file_storage_discard_unstarted(&data->ram_fstorage);
    close_file_storage(&data->rom_fstorage);
    memset(cart,0,sizeof(*cart));return 0;
}
