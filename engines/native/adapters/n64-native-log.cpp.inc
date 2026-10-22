// Pinned native GLideN64 diagnostics through the frontend log callback only.
#include <cstdarg>
#include <cstdio>
#include <cstdint>
#include <mupen64plus-next_common.h>
#include "Log.h"

extern retro_log_printf_t log_cb;
void LogDebug(const char* file,int line,u16 type,const char* format,...)
{
    if(!log_cb||!format||type>5)return;
    char message[4096],framed[4608];va_list args;va_start(args,format);
    const auto count=std::vsnprintf(message,sizeof(message),format,args);va_end(args);
    if(count<0)return;message[sizeof(message)-1]=0;
    std::snprintf(framed,sizeof(framed),"%.384s:%d, \"%s\"\n",file?file:"",line,message);
    static const enum retro_log_level levels[]={RETRO_LOG_INFO,RETRO_LOG_ERROR,RETRO_LOG_INFO,RETRO_LOG_WARN,RETRO_LOG_DEBUG,RETRO_LOG_DEBUG};
    // Content containing percent sequences is always data for this callback.
    log_cb(levels[type],"%s",framed);
}
#if defined(OS_WINDOWS) && !defined(MINGW)
void debugPrint(const char* format,...)
{
    if(!log_cb||!format)return;char message[4096];va_list args;va_start(args,format);
    const auto count=std::vsnprintf(message,sizeof(message),format,args);va_end(args);
    if(count>=0){message[sizeof(message)-1]=0;log_cb(RETRO_LOG_DEBUG,"%s",message);}
}
#endif
