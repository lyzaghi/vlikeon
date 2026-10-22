#include "owned_actor_exit.h"
static int om_n64_snapshot_mutex_ready;
int savestates_init(void)
{
    if(om_n64_snapshot_mutex_ready)return 0;
#ifdef USE_SDL
    savestates_lock=SDL_CreateMutex();if(!savestates_lock)return 0;
#else
    if(pthread_mutex_init(&savestates_lock,NULL)!=0)return 0;
#endif
    om_n64_snapshot_mutex_ready=1;return 1;
}
void savestates_deinit(void)
{
    if(!om_n64_snapshot_mutex_ready)return;
    savestates_clear_job();
#ifdef USE_SDL
    SDL_DestroyMutex(savestates_lock);savestates_lock=NULL;
#else
    /* This native profile has no asynchronous snapshot worker. A refused
     * retirement is not permission to unload an unknown live mutex owner. */
    if(pthread_mutex_destroy(&savestates_lock)!=0)omni_owned_actor_exit(1);
#endif
    om_n64_snapshot_mutex_ready=0;
}
