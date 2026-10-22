#include "n64-core-owner.h"
#include <cstdint>
#include <mutex>
#include <thread>
namespace {
enum class Phase {Dormant,Initialized,Loaded,Guest,Paused,Stopped};
struct Owner {
    std::mutex mutex;std::thread::id thread;
    Phase phase=Phase::Dormant;void* root=nullptr;void* guest=nullptr;
    void* buffer=nullptr;size_t bytes=0;int operation=0;
    bool active=false,started=false,ready=false,ended=false,admission_failed=false,snapshot_ok=false,exit_ok=false;
    bool current()const{return active && thread==std::this_thread::get_id();}
    bool root_is(void* value)const{return current() && root && root==value;}
    bool guest_is(void* value)const{return current() && guest && guest==value;}
    bool paused(void* value)const{return root_is(value)&&phase==Phase::Paused&&ready&&!ended&&!operation&&!admission_failed;}
} state;
template<class F> int guarded(F function,int failure=0) noexcept {
    try{std::lock_guard<std::mutex> lock(state.mutex);return function();}catch(...){return failure;}
}
}
extern "C" int om_n64_owner_begin(void){return guarded([]{
    if(state.active)return 0;
    state.thread=std::this_thread::get_id();state.phase=Phase::Initialized;state.active=true;
    state.root=nullptr;state.guest=nullptr;state.buffer=nullptr;state.bytes=0;state.operation=0;
    state.started=false;state.ready=false;state.ended=false;state.admission_failed=false;state.snapshot_ok=false;state.exit_ok=false;return 1;
});}
extern "C" int om_n64_owner_thread(void){return guarded([]{return state.current()?1:0;});}
extern "C" int om_n64_owner_access(void){return guarded([]{return !state.active || state.current()?1:0;});}
extern "C" int om_n64_owner_bind(void* root,void* guest){return guarded([&]{
    if(!state.current()||state.phase!=Phase::Initialized||state.root||state.guest||!root||!guest||root==guest)return 0;
    state.root=root;state.guest=guest;return 1;
});}
extern "C" int om_n64_owner_loaded(void* root){return guarded([&]{
    if(!state.root_is(root)||state.phase!=Phase::Initialized||!state.guest||state.operation||state.admission_failed)return 0;
    state.phase=Phase::Loaded;return 1;
});}
extern "C" int om_n64_owner_enter(void* root,int action,void* buffer,size_t bytes){return guarded([&]{
    if(!state.root_is(root)||state.operation||!state.guest)return 0;
    if(action==OM_N64_PREPARE){if(state.phase!=Phase::Loaded||state.started||state.admission_failed)return 0;}
    else if(action==OM_N64_STOP){if(state.phase!=Phase::Paused||!state.started||state.ended)return 0;}
    else if(action==OM_N64_RUN||action==OM_N64_SAVE||action==OM_N64_LOAD){if(!state.paused(root))return 0;}
    else return 0;
    if(action==OM_N64_SAVE||action==OM_N64_LOAD){if(!buffer||!bytes||bytes>32u*1024u*1024u)return 0;}
    else if(buffer||bytes)return 0;
    state.phase=Phase::Guest;state.operation=action;state.buffer=buffer;state.bytes=bytes;state.snapshot_ok=false;state.started=true;return 1;
});}
extern "C" int om_n64_owner_action(void* guest,void** buffer,size_t* bytes){return guarded([&]{
    if(!buffer||!bytes||!state.guest_is(guest)||state.phase!=Phase::Guest||state.operation<OM_N64_PREPARE||state.operation>OM_N64_STOP)return 0;
    if(state.admission_failed&&state.operation!=OM_N64_STOP)return 0;
    *buffer=state.buffer;*bytes=state.bytes;return state.operation;
});}
extern "C" int om_n64_owner_boundary(void* guest,int initial){return guarded([&]{
    if(!state.guest_is(guest)||state.phase!=Phase::Guest||state.admission_failed)return 0;
    if(initial){if(state.operation!=OM_N64_PREPARE||state.ready)return 0;state.ready=true;}
    else if(state.operation!=OM_N64_RUN||!state.ready)return 0;
    state.phase=Phase::Paused;return 1;
});}
extern "C" int om_n64_owner_snapshot_done(void* guest,int success){return guarded([&]{
    if(!state.guest_is(guest)||state.phase!=Phase::Guest||!state.ready||(state.operation!=OM_N64_SAVE&&state.operation!=OM_N64_LOAD))return 0;
    state.snapshot_ok=success==1;state.buffer=nullptr;state.bytes=0;state.phase=Phase::Paused;return 1;
});}
extern "C" int om_n64_owner_ended(void* guest,int success){return guarded([&]{
    if(!state.guest_is(guest)||state.phase!=Phase::Guest||!state.started||state.ended)return 0;
    state.phase=Phase::Stopped;state.ended=true;state.ready=false;state.exit_ok=success==1;
    if(!state.exit_ok)state.admission_failed=true;
    state.buffer=nullptr;state.bytes=0;return 1;
});}
extern "C" int om_n64_owner_finish(void* root){return guarded([&]{
    if(!state.root_is(root)||!state.operation||state.operation==OM_N64_FLUSH||state.phase==Phase::Guest)return 0;
    int okay=0;
    if(state.operation==OM_N64_STOP)okay=state.phase==Phase::Stopped&&state.ended&&state.exit_ok;
    else if(state.phase==Phase::Paused&&state.ready&&!state.admission_failed)
        okay=(state.operation==OM_N64_SAVE||state.operation==OM_N64_LOAD)?state.snapshot_ok:1;
    state.operation=0;state.buffer=nullptr;state.bytes=0;return okay;
});}
extern "C" int om_n64_owner_ready(void* root){return guarded([&]{return state.paused(root)?1:0;});}
extern "C" int om_n64_owner_at_boundary(void* root){return guarded([&]{return state.root_is(root)&&state.operation&&(state.phase==Phase::Paused||state.phase==Phase::Stopped)?1:0;});}
extern "C" int om_n64_owner_started(void){return guarded([]{return state.current()&&state.started?1:0;});}
extern "C" int om_n64_owner_stopped(void* root){return guarded([&]{return state.root_is(root)&&state.phase==Phase::Stopped&&!state.operation?1:0;});}
extern "C" int om_n64_owner_cancel_unstarted(void* root){return guarded([&]{
    if(!state.root_is(root)||state.phase!=Phase::Loaded||state.started||state.operation)return 0;
    state.phase=Phase::Stopped;state.ended=true;state.exit_ok=true;return 1;
});}
extern "C" int om_n64_owner_flush_begin(void* root){return guarded([&]{
    const bool stopped=state.root_is(root)&&state.phase==Phase::Stopped&&state.ended&&!state.operation;
    if(!state.paused(root)&&!stopped)return 0;state.operation=OM_N64_FLUSH;return 1;
});}
extern "C" int om_n64_owner_flush_end(void* root,int result){return guarded([&]{
    if(!state.root_is(root)||(state.phase!=Phase::Paused&&state.phase!=Phase::Stopped)||state.operation!=OM_N64_FLUSH)return -1;
    state.operation=0;
    /* Post-stop persistence has its own receipt. It cannot make a failed CPU
     * session ready again, but must permit recovery of its last owned saves. */
    return (state.phase!=Phase::Stopped&&state.admission_failed)||result < -1||result>1?-1:result;
},-1);}
extern "C" void om_n64_owner_admission_failed(void){(void)guarded([]{if(state.current())state.admission_failed=true;return 1;});}
extern "C" int om_n64_owner_admitted(void){return guarded([]{return !state.active || (state.current()&&!state.admission_failed)?1:0;});}
extern "C" int om_n64_owner_persistence_allowed(void){return guarded([]{
    return !state.active || (state.current()&&(!state.admission_failed
        || (state.phase==Phase::Stopped&&state.ended&&state.operation==OM_N64_FLUSH)))?1:0;
});}
extern "C" int om_n64_owner_end(void* root){return guarded([&]{
    if(!state.current()||state.root!=root||state.operation||state.phase==Phase::Guest||(state.started&&!state.ended))return 0;
    state.active=false;state.phase=Phase::Dormant;state.thread=std::thread::id();state.root=nullptr;state.guest=nullptr;state.buffer=nullptr;state.bytes=0;return 1;
});}
extern "C" int om_n64_owner_quiescent(void){return guarded([]{return state.active?0:1;});}
