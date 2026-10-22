// Actual prepared native diagnostics plus real memory-file bridge/store.
#include "memory_files.hpp"
#include "n64-memory-io.h"
#include "TxDbg.h"
#include "Log.h"
#include <libretro.h>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

static void require(bool good,const char* why){if(!good)throw std::runtime_error(why);}
void LogDebug(const char*,int,u16,const char*,...);
static unsigned calls=0;static bool literal=true;static enum retro_log_level severity=RETRO_LOG_INFO;static std::string message;
static void sink(enum retro_log_level level,const char* format,...){
  ++calls;severity=level;
  if(!format||std::strcmp(format,"%s")!=0){literal=false;message=format?format:"";return;}
  char text[8192];va_list args;va_start(args,format);std::vsnprintf(text,sizeof(text),format,args);va_end(args);message=text;
}
retro_log_printf_t log_cb=sink;

static std::string read(const OmniWritableFilesV1& api,const char* path){
  OmniFilesStatV1 shape{};require(api.stat(api.context,path,&shape)==0&&shape.bytes<=1024*1024,"bounded diagnostic log");
  std::string value(static_cast<std::size_t>(shape.bytes),'\0');std::uint64_t handle=0,actual=0;
  require(api.open(api.context,path,OMNI_FILES_READ,&handle)==0&&api.read(api.context,handle,0,value.empty()?nullptr:&value[0],value.size(),&actual)==0&&actual==value.size()&&api.close(api.context,handle)==0,"complete diagnostic log bytes");return value;
}
int main(){try{
  LogDebug("file%p",4,LOG_WARNING,"literal %%s %%n %d",7);
  require(calls==1&&literal&&severity==RETRO_LOG_WARN&&message.find("literal %s %n 7")!=std::string::npos&&message.find("file%p:4")!=std::string::npos,"format sequences remain diagnostic data");
  const std::string oversized(10000,'x');LogDebug("fixture",5,LOG_ERROR,"%s",oversized.c_str());
  require(calls==2&&literal&&severity==RETRO_LOG_ERROR&&message.size()<=4607,"frontend diagnostics have a finite format buffer");
  LogDebug("fixture",0,65535,"bad severity");LogDebug("fixture",0,0,nullptr);require(calls==2,"invalid diagnostic inputs refused");
  log_cb=nullptr;LogDebug("fixture",0,0,"no callback");require(calls==2,"absent callback is safe");log_cb=sink;
  omni::MemoryFiles store;const auto api=store.interface();require(om_n64_files_install(&api)==1,"real scratch log table");
  const char* path="scratch/Mupen64plus/debug/glidenhq.dbg";auto* logger=TxDbg::getInstance();
  logger->output(1,L"message %ls %d",L"\u00e9",7);
  require(read(api,path)=="1:\tmessage \xc3\xa9 7\n","wide texture diagnostics convert to UTF-8 payload");
  logger->output(81,L"filtered out");require(read(api,path)=="1:\tmessage \xc3\xa9 7\n","level admission preserves prior log");
  std::uint64_t handle=0;require(api.open(api.context,path,OMNI_FILES_WRITE,&handle)==0&&api.resize(api.context,handle,1024*1024)==0&&api.close(api.context,handle)==0,"full log fixture");
  const auto retained=read(api,path);logger->output(1,L"beyond log limit");require(read(api,path)==retained,"full scratch log cannot silently truncate old diagnostics");
  OmniFilesUsageV1 usage{};require(api.usage(api.context,&usage)==0&&usage.open_handles==0,"diagnostic calls retain no streams");
  require(om_n64_files_install(nullptr)==1,"retire log callback context");logger->output(1,L"uninstalled");
  std::cout<<"N64 bounded diagnostic cases passed\n";return 0;
}catch(const std::exception& error){std::cerr<<error.what()<<'\n';return 1;}}
