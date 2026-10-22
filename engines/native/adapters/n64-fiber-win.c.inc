/*
  Based on libco.win (2008-01-28), Nach and byuu, public domain.
  OmniPlay: own one root/guest pair, check conversion and retire the root.
*/
#define LIBCO_C
#include <libco.h>
#ifndef WINVER
#define WINVER 0x0A00
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#if defined(_MSC_VER)
#define OM_N64_FIBER_TLS __declspec(thread)
#elif defined(__GNUC__)
#define OM_N64_FIBER_TLS __thread
#else
#error Select an explicit Windows compiler TLS implementation
#endif

#ifdef __cplusplus
extern "C" {
#endif
/* The pinned libco.h may define thread_local as empty without LIBCO_MP. */
static OM_N64_FIBER_TLS cothread_t co_active_=0,co_root_=0,co_guest_=0;

static void __stdcall co_thunk(void* entry)
{
  ((void(*)(void))entry)();
}

cothread_t co_active(void)
{
  if(!co_active_){
    /* Never adopt another owner's fiber or infer conversion success from
     * GetCurrentFiber after a failed ConvertThreadToFiberEx. */
    if(IsThreadAFiber())return 0;
    co_root_=ConvertThreadToFiberEx(0,FIBER_FLAG_FLOAT_SWITCH);
    if(!co_root_)return 0;
    co_active_=co_root_;
  }
  return co_active_;
}

cothread_t co_create(unsigned int size,void(*entry)(void))
{
  if(!entry||co_guest_||!co_active()||co_active_!=co_root_)return 0;
  co_guest_=(cothread_t)CreateFiberEx(size,size,FIBER_FLAG_FLOAT_SWITCH,co_thunk,(void*)entry);
  return co_guest_;
}

void co_delete(cothread_t fiber)
{
  if(!fiber||fiber!=co_guest_||co_active_!=co_root_)return;
  DeleteFiber(fiber);co_guest_=0;
}

void co_switch(cothread_t fiber)
{
  co_active_=fiber;SwitchToFiber(fiber);
}

int om_n64_co_retire_thread(void)
{
  if(!co_active_&&!co_root_&&!co_guest_)return 1;
  if(co_guest_||!co_root_||co_active_!=co_root_||!IsThreadAFiber()||GetCurrentFiber()!=co_root_)return 0;
  if(!ConvertFiberToThread())return 0;
  co_active_=0;co_root_=0;return 1;
}
#ifdef __cplusplus
}
#endif
