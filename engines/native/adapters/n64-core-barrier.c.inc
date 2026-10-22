/* Actual single-thread GL candidate exports. No fake pause or queued snapshot
 * success: device/CPU initialization and each snapshot execute on the owned
 * guest fiber, parked outside the interpreter's next instruction. */
#ifdef M64P_PARALLEL
#error The OmniPlay single-owner N64 snapshot profile needs synchronous workqueue callbacks
#endif
int om_n64_native_peripherals_quiet(void);
int om_n64_native_flush_peripherals(void);
int om_n64_native_retire_peripherals(void);

static int om_n64_at_root(void)
{return om_n64_owner_thread() && retro_thread && co_active()==retro_thread;}
static void om_n64_core_failure(void)
{
    if(!om_n64_owner_thread())return;
    om_n64_owner_admission_failed();
    if(environ_cb)environ_cb(RETRO_ENVIRONMENT_SHUTDOWN,NULL);
}
static int om_n64_guest_gate(int initial)
{
    if(!om_n64_owner_thread() || co_active()!=game_thread || !om_n64_owner_boundary(game_thread,initial)){om_n64_core_failure();return 0;}
    for(;;){
        void* buffer=NULL;size_t bytes=0;int action;
        co_switch(retro_thread);
        action=om_n64_owner_action(game_thread,&buffer,&bytes);
        if(action==OM_N64_RUN)return 1;
        if(action==OM_N64_STOP)return 0;
        if((action==OM_N64_SAVE || action==OM_N64_LOAD) && buffer && bytes==retro_serialize_size()){
            int result;
            retro_savestate_complete=false;retro_savestate_result=0;
            savestates_set_job(action==OM_N64_SAVE?savestates_job_save:savestates_job_load,savestates_type_m64p,(const char*)buffer);
            result=action==OM_N64_SAVE?savestates_save():savestates_load();
            savestates_set_job(savestates_job_nothing,savestates_type_unknown,NULL);om_n64_frame_pending=0;
            if(!om_n64_owner_snapshot_done(game_thread,result==1 && retro_savestate_complete && retro_savestate_result==1)){om_n64_core_failure();return 0;}
        }else{om_n64_core_failure();return 0;}
    }
}
int om_n64_guest_initial_checkpoint(void){return om_n64_guest_gate(1);}
void om_n64_guest_interrupt_checkpoint(void)
{
    if(!om_n64_owner_admitted()){*r4300_stop(&g_dev.r4300)=1;return;}
    if(!om_n64_frame_pending)return;
    om_n64_frame_pending=0;
    if(!om_n64_guest_gate(0))*r4300_stop(&g_dev.r4300)=1;
}
static int om_n64_drive(int action,void* buffer,size_t bytes)
{
    int result;
    if(!om_n64_at_root() || !game_thread || current_rdp_type!=RDP_PLUGIN_GLIDEN64 || EnableThreadedRenderer)return 0;
    if(!glsm_ctl(GLSM_CTL_STATE_BIND,NULL))return 0;
    if(!om_n64_owner_enter(retro_thread,action,buffer,bytes)){glsm_ctl(GLSM_CTL_STATE_UNBIND,NULL);return 0;}
    co_switch(game_thread);
    /* A malformed yield cannot return while a borrowed snapshot remains in
     * flight. This profile runs only in the owned actor; exit before caller
     * buffers, DLL statics or the callback context can be destroyed. */
    if(!om_n64_owner_at_boundary(retro_thread))_Exit(1);
    glsm_ctl(GLSM_CTL_STATE_UNBIND,NULL);
    result=om_n64_owner_finish(retro_thread);return result;
}
RETRO_API bool omniplay_finish_prepare(void)
{
    if(!om_n64_at_root() || !omniplay_memory_startup_ready || !load_game_successful)return false;
    if(om_n64_owner_ready(retro_thread))return true;
    return om_n64_drive(OM_N64_PREPARE,NULL,0)==1 && om_n64_owner_ready(retro_thread);
}
RETRO_API void omniplay_pause_guest(void)
{
    if(!om_n64_at_root() || !om_n64_owner_ready(retro_thread))om_n64_core_failure();
}
RETRO_API int omniplay_flush_io_checked(void)
{
    int result;
    if(!om_n64_at_root() || !om_n64_owner_flush_begin(retro_thread))return -1;
    result=om_n64_files_writes_settled()?om_n64_native_flush_peripherals():-1;
    if(!om_n64_files_writes_settled())result=-1;
    return om_n64_owner_flush_end(retro_thread,result);
}
bool retro_serialize(void* buffer,size_t bytes)
{
    if(!om_n64_owner_thread() || !buffer || bytes!=retro_serialize_size() || !load_game_successful)return false;
    return om_n64_drive(OM_N64_SAVE,buffer,bytes)==1;
}
bool retro_unserialize(const void* buffer,size_t bytes)
{
    if(!om_n64_owner_thread() || !buffer || bytes!=retro_serialize_size() || !load_game_successful)return false;
    return om_n64_drive(OM_N64_LOAD,(void*)buffer,bytes)==1;
}
void retro_run(void)
{
    bool updated=false;
    if(!om_n64_at_root() || !om_n64_owner_ready(retro_thread) || !load_game_successful)return;
    libretro_swap_buffer=false;
    if(environ_cb && environ_cb(RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,&updated) && updated)update_variables(false);
    if(!om_n64_drive(OM_N64_RUN,NULL,0)){om_n64_core_failure();return;}
    if(!video_cb)return;
    if(libretro_swap_buffer)video_cb(RETRO_HW_FRAME_BUFFER_VALID,retro_screen_width,retro_screen_height,0);
    else if(EnableFrameDuping)video_cb(NULL,retro_screen_width,retro_screen_height,0);
}
void retro_return(void)
{
    if(!om_n64_owner_thread() || co_active()!=game_thread){om_n64_core_failure();return;}
    om_n64_frame_pending=1;
}
void retro_unload_game(void)
{
    if(!om_n64_at_root())return;
    if(!load_game_successful)return;
    if(!om_n64_owner_started()){
        if(!om_n64_owner_cancel_unstarted(retro_thread)){om_n64_core_failure();return;}
    }else if(!om_n64_owner_stopped(retro_thread)){
        if(CoreDoCommand(M64CMD_STOP,0,NULL)!=M64ERR_SUCCESS || !om_n64_drive(OM_N64_STOP,NULL,0)){om_n64_core_failure();return;}
    }
    if(!om_n64_native_retire_peripherals()){om_n64_core_failure();return;}
    if(CoreDoCommand(M64CMD_ROM_CLOSE,0,NULL)!=M64ERR_SUCCESS){om_n64_core_failure();return;}
    cleanup_global_paths();load_game_successful=false;emu_initialized=false;context_setup_first_init=false;retro_savestate_complete=false;om_n64_frame_pending=0;
}
void retro_deinit(void)
{
    if(!om_n64_owner_thread() || !omniplay_memory_core_live)return;
    if(retro_thread && co_active()!=retro_thread){om_n64_core_failure();return;}
    if(load_game_successful)retro_unload_game();
    if(load_game_successful || !om_n64_native_retire_peripherals() || !om_n64_native_peripherals_quiet())return;
    if(omniplay_memory_startup_ready && !om_n64_owner_started()){
        m64p_error closed=CoreDoCommand(M64CMD_ROM_CLOSE,0,NULL);
        if(closed!=M64ERR_SUCCESS && closed!=M64ERR_INVALID_STATE){om_n64_core_failure();return;}
    }
    if(omniplay_memory_startup_ready && !omniplay_upstream_deinit_finished){
        omniplay_upstream_deinit();omniplay_upstream_deinit_finished=1;
    }
    if(!om_n64_native_peripherals_quiet() || !om_n64_texture_pool_retired() || !om_n64_files_idle())return;
    if(game_thread){co_delete(game_thread);game_thread=NULL;}
#ifdef _WIN32
    if(!om_n64_co_retire_thread())return;
#endif
    if(!om_n64_owner_end(retro_thread) && !om_n64_owner_end(NULL))return;
    retro_thread=NULL;omniplay_memory_startup_ready=0;omniplay_memory_core_live=0;
}
RETRO_API int omniplay_install_writable_files_v1(const struct OmniWritableFilesV1* table)
{
    if(!om_n64_owner_quiescent() || omniplay_memory_core_live || !om_n64_texture_pool_retired() || !om_n64_native_peripherals_quiet())return 0;
    return om_n64_files_install(table);
}
