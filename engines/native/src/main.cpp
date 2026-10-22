#include "engine.hpp"
#include <SDL3/SDL_main.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <thread>
#include <charconv>
#include <optional>
#ifdef __APPLE__
#include <sys/resource.h>
#endif
#ifdef _WIN32
#include <windows.h>
#include <fcntl.h>
#include <io.h>
#include <vector>

static bool restore_private_temporary_directory(){
  std::vector<wchar_t> path(32768);
  const auto length=GetEnvironmentVariableW(L"OMNIPLAY_PRIVATE_SCRATCH",path.data(),static_cast<DWORD>(path.size()));
  if(!length)return GetLastError()==ERROR_ENVVAR_NOT_FOUND; // Standalone qualification has no host scratch contract.
  if(length>=path.size()||!std::filesystem::path(path.data()).is_absolute())return false;
  // AppContainer construction can rewrite TEMP/TMP. Restore only the explicit
  // host-selected directory; its existing ACL/label controls write authority.
  // Keep the CRT environment coherent for cores using getenv/_wgetenv too.
  // This happens before our worker/core threads start.
  return _wputenv_s(L"TEMP",path.data())==0&&_wputenv_s(L"TMP",path.data())==0&&
    SetEnvironmentVariableW(L"TEMP",path.data())&&SetEnvironmentVariableW(L"TMP",path.data());
}
#endif

static std::uintptr_t endpoint(const char* text){if(!text||!*text||std::strlen(text)>20||(*text=='0'&&text[1]))throw omni::Failure("invalid_request");std::uintptr_t result=0;const auto end=text+std::strlen(text);const auto value=std::from_chars(text,end,result);if(value.ec!=std::errc{}||value.ptr!=end||!result)throw omni::Failure("invalid_request");return result;}
#ifdef __APPLE__
static bool apply_macos_limits(){const rlimit core{0,0},files{64,64},memory{10ULL*1024*1024*1024,10ULL*1024*1024*1024};return setrlimit(RLIMIT_CORE,&core)==0&&setrlimit(RLIMIT_NOFILE,&files)==0&&setrlimit(RLIMIT_AS,&memory)==0;}
#endif
int main(int argc,char** argv){
#ifdef __APPLE__
  if(!apply_macos_limits())return 2;
#endif
  std::optional<omni::presentation::Admission> admission;
  try{if(argc<2||std::strcmp(argv[1],"--stdio-runtime-v1")!=0)return 2;if(argc==6&&std::strcmp(argv[2],"--presentation-client-v2")==0)admission=omni::presentation::Admission{omni::presentation::session_from_text(std::span<const char>(argv[3],std::strlen(argv[3]))),endpoint(argv[4]),endpoint(argv[5])};else if(argc!=2)return 2;}catch(...){return 2;}
#ifdef _WIN32
  if(!restore_private_temporary_directory())return 2;
  if(_setmode(_fileno(stdin),_O_BINARY)==-1||_setmode(_fileno(stdout),_O_BINARY)==-1)return 2;
#endif
  if(!omni::initialize_transport())return 2;
  // Private duplicated handles carry framed IPC; ordinary core/SDL output is discarded.
  SDL_SetLogOutputFunction([](void*,int,SDL_LogPriority,const char*){},nullptr);
  SDL_SetMainReady();
  try{
    std::unique_ptr<omni::presentation::Client> presentation;if(admission){presentation=std::make_unique<omni::presentation::Client>(*admission);if(!presentation->wait_ready(std::chrono::seconds(5)))throw omni::Failure("runtime_unavailable");}
    omni::Engine engine(std::move(presentation));omni::Inbox inbox;
    // Keep process-lifetime inbox/core state alive beneath a blocked detached
    // reader on failure as well as normal exit. Actual process ownership belongs
    // to the broker; requesting channel stop alone is not process termination.
    try{
      std::thread([&inbox]{inbox.read_stdin();}).detach();
      int exit_code=0;
      while(!engine.quitting()&&!inbox.closed()){
        engine.poll();omni::Packet packet;
        if(inbox.pop(packet)){
          try{engine.process(packet);}
          catch(const omni::Failure& error){omni::reply_error(packet.header.at("id").get<std::uint64_t>(),error.code);if(std::strcmp(error.code,"runtime_crashed")==0){exit_code=1;break;}}
          catch(...){omni::reply_error(packet.header.at("id").get<std::uint64_t>(),"runtime_crashed");exit_code=1;break;}
          packet=omni::Packet{}; // Release the old load buffer before admitting another.
          if(!engine.quitting())inbox.complete();
        }
        if(engine.quitting())break;engine.tick();SDL_Delay(1);
      }
      if(inbox.failed())exit_code=1;
      engine.shutdown();std::fflush(stdout);omni_owned_actor_exit(exit_code);
    }catch(...){try{engine.shutdown();}catch(...){}omni_owned_actor_exit(1);}
  }catch(...){omni_owned_actor_exit(1);}
}
