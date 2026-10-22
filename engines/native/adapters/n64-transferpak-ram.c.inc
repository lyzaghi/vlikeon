static void init_gb_ram(void* opaque,size_t ram_size,void** storage,const struct storage_backend_interface** istorage)
{
    struct gb_cart_data* data=(struct gb_cart_data*)opaque;
    char* import_path=(g_media_loader.get_gb_cart_ram==NULL)
        ? (retro_transferpak_ram_path?strdup(retro_transferpak_ram_path):NULL)
        : g_media_loader.get_gb_cart_ram(g_media_loader.cb_data,data->control_id);
    char* filename;
    int result;
    *storage=NULL;*istorage=NULL;
    if(g_media_loader.get_gb_cart_ram==NULL && retro_transferpak_ram_path && !import_path){om_n64_owner_admission_failed();return;}
    filename=get_gb_ram_path(data->rom_fstorage.data,data->rom_fstorage.size,data->control_id+1);
    if(!filename){free(import_path);om_n64_owner_admission_failed();return;}
    result=open_file_storage(&data->ram_fstorage,ram_size,filename);
    if(result!=file_ok)goto rejected;
    /* An explicit/adjacent old RAM is a readonly import, never a destination.
     * A retained memory save wins. Errors other than literal absence cannot
     * turn the old import into zeroed guest RAM that later overwrites it. */
    if(!data->ram_fstorage.loaded_existing && import_path && import_path[0]){
        result=read_from_file(import_path,data->ram_fstorage.data,ram_size);
        if(result!=file_ok && !(result==file_open_error && errno==ENOENT))goto rejected;
        if(result==file_ok){
            g_ifile_storage.save(&data->ram_fstorage,0,ram_size);
            if(data->ram_fstorage.write_failed){result=file_write_error;goto rejected;}
        }
    }
    free(import_path);*storage=&data->ram_fstorage;*istorage=&g_ifile_storage;return;
rejected:
    om_n64_owner_admission_failed();
    if(log_cb)log_cb(RETRO_LOG_ERROR,"Transferpak RAM admission failed (%d)\n",result);
    free(import_path);om_n64_file_storage_discard_unstarted(&data->ram_fstorage);
}
