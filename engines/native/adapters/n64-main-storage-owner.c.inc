int om_n64_gb_retire_storage(struct gb_cart_data* data)
{
    if(!om_n64_owner_access() || !data)return 0;
    if(!om_n64_rtc_storage_prepare_retire(data->rtc))return 0;
    if(!close_file_storage(&data->ram_fstorage))return 0;
    if(!om_n64_rtc_storage_detach(data->rtc))return 0;
    data->rtc=NULL;return 1;
}
int om_n64_native_peripherals_quiet(void)
{
    unsigned int port;
    if(!om_n64_owner_access())return 0;
    for(port=0;port<GAME_CONTROLLERS_COUNT;++port)
        if(l_gb_carts_data[port].ram_fstorage.data || l_gb_carts_data[port].ram_fstorage.filename || l_gb_carts_data[port].rtc)return 0;
    return om_n64_dd_quiescent() && om_n64_rtc_storage_quiescent();
}
int om_n64_native_flush_peripherals(void)
{
    unsigned int port;int complete=1;
    if(!om_n64_owner_access() || !om_n64_owner_persistence_allowed())return -1;
    for(port=0;port<GAME_CONTROLLERS_COUNT;++port){
        struct file_storage* storage=&l_gb_carts_data[port].ram_fstorage;
        if(storage->data || storage->filename){int result=om_n64_file_storage_flush(storage);if(result<0)return -1;if(!result)complete=0;}
    }
    {
        int result=om_n64_dd_flush_all();if(result<0)return -1;if(!result)complete=0;
    }
    {int result=om_n64_rtc_storage_flush_all();if(result<0)return -1;if(!result)complete=0;}
    return complete;
}
int om_n64_native_retire_peripherals(void)
{
    unsigned int port;int complete=1;
    if(!om_n64_owner_access())return 0;
    for(port=0;port<GAME_CONTROLLERS_COUNT;++port){
        if(!om_n64_gb_retire_storage(&l_gb_carts_data[port]))complete=0;
    }
    if(!om_n64_dd_retire_all())complete=0;
    return complete && om_n64_native_peripherals_quiet();
}
