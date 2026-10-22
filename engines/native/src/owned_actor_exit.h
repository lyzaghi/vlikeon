#ifndef OMNIPLAY_OWNED_ACTOR_EXIT_H
#define OMNIPLAY_OWNED_ACTOR_EXIT_H
/* Only for the disposable, broker-owned native actor. No save/stop success is
 * implied. Windows DLL detach must not run over unknown live core ownership. */
#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
__declspec(noreturn) static inline void omni_owned_actor_exit(unsigned code)
{
    TerminateProcess(GetCurrentProcess(),code);
    /* A successful self-termination does not return. If it is refused, retain
     * ownership until the broker's job/process termination; never unload or
     * fall back to ExitProcess/CRT termination with DLL detach callbacks. */
    for(;;)Sleep(INFINITE);
}
#else
#include <stdlib.h>
#if defined(__cplusplus)
[[noreturn]]
#elif defined(__GNUC__)
__attribute__((noreturn))
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Noreturn
#endif
static inline void omni_owned_actor_exit(unsigned code){_Exit((int)code);}
#endif
#endif
