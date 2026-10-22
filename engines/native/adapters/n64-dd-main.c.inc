static void load_dd_rom(uint8_t* rom,size_t* rom_size,uint8_t* disk_region)
{
    struct file_storage image={0};char* filename=NULL;uint32_t magic;int result;
    *rom_size=0;
    if(!rom || !disk_region)return;
    if(g_media_loader.set_dd_rom_region)g_media_loader.set_dd_rom_region(g_media_loader.cb_data,*disk_region);
    if(g_media_loader.get_dd_rom){
        filename=g_media_loader.get_dd_rom(g_media_loader.cb_data);
        if(!filename || !filename[0]){free(filename);return;}
    }else if(retro_dd_path_rom){
        filename=strdup(retro_dd_path_rom);if(!filename || !filename[0]){free(filename);return;}
    }else{
        const char* directory=NULL;size_t bytes=0;const char* slash;
        if(!environ_cb || !environ_cb(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY,&directory) || !directory)return;
        while(bytes<=1024 && directory[bytes])++bytes;
        if(!bytes || bytes>1024-32)return;
        slash=directory[bytes-1]=='/' || directory[bytes-1]=='\\'?"":"/";
        filename=formatstr("%s%sMupen64plus/IPL.n64",directory,slash);if(!filename)return;
    }
    result=open_rom_file_storage(&image,filename);
    if(result!=file_ok){free(filename);return;}
    if(image.size<4 || image.size>DD_ROM_MAX_SIZE || (image.size&3)){close_file_storage(&image);return;}
    magic=((uint32_t)image.data[0]<<24)|((uint32_t)image.data[1]<<16)|((uint32_t)image.data[2]<<8)|image.data[3];
    switch(magic){
    case 0x80270740u:to_big_endian_buffer(image.data,4,image.size/4);break;
    case 0x40072780u:to_little_endian_buffer(image.data,4,image.size/4);break;
    case 0x27804007u:swap_buffer(image.data,2,image.size/2);break;
    default:close_file_storage(&image);return;
    }
    memcpy(rom,image.data,image.size);*rom_size=image.size;close_file_storage(&image);
}

extern char* retro_dd_path_img;
extern char* retro_dd_path_rom;
static int load_dd_disk(struct dd_disk* disk,const struct storage_backend_interface** interface)
{
    char* filename;
    int complete;
    if(g_media_loader.get_dd_disk)filename=g_media_loader.get_dd_disk(g_media_loader.cb_data);
    else{
        if(!retro_dd_path_img)return 0;
        filename=strdup(retro_dd_path_img);
        if(!filename)return -1;
    }
    if(!filename || !filename[0]){free(filename);return 0;}
    /* Preserve the pinned actual full-image default. The owned loader also
     * implements checked RAM-only/readonly modes for explicit integrations. */
    complete=om_n64_dd_open(disk,interface,filename,0);
    free(filename);return complete?1:-1;
}
static void close_dd_disk(struct dd_disk* disk)
{
    if(!om_n64_dd_close(disk)){
        if(log_cb)log_cb(RETRO_LOG_ERROR,"64DD final memory publication failed\n");
        if(environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);
    }
}

