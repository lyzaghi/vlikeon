#include "core_module.hpp"
#include <chrono>
#include <filesystem>
#include <stdexcept>
#include <vector>
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(__APPLE__)
#include <dlfcn.h>
#include <mach-o/dyld.h>
#endif
#if defined(OMNIPLAY_GUEST_ACTOR) && defined(__linux__)
#include <dlfcn.h>
#include <unistd.h>
#endif

namespace omni {
std::string executable_base_directory(){
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(_WIN32)
  std::vector<wchar_t> path(32768);
  const auto length=GetModuleFileNameW(nullptr,path.data(),static_cast<DWORD>(path.size()));
  if(!length||length>=path.size())throw std::runtime_error("runtime_unavailable");
  const auto directory=std::filesystem::path(std::wstring(path.data(),length)).parent_path();
  if(!directory.is_absolute())throw std::runtime_error("runtime_unavailable");
  const auto utf8=directory.generic_u8string();
  return std::string(reinterpret_cast<const char*>(utf8.data()),utf8.size())+"/";
#elif defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(__APPLE__)
  std::uint32_t size=0;_NSGetExecutablePath(nullptr,&size);
  if(!size||size>32768)throw std::runtime_error("runtime_unavailable");
  std::vector<char> path(size);if(_NSGetExecutablePath(path.data(),&size)!=0)throw std::runtime_error("runtime_unavailable");
  const auto executable=std::filesystem::canonical(path.data());
  if(!executable.is_absolute())throw std::runtime_error("runtime_unavailable");
  return executable.parent_path().generic_string()+"/";
#elif defined(OMNIPLAY_GUEST_ACTOR) && defined(__linux__)
  std::vector<char> path(32768);
  const auto length=readlink("/proc/self/exe",path.data(),path.size());
  if(length<=0||static_cast<size_t>(length)>=path.size())throw std::runtime_error("runtime_unavailable");
  const auto executable=std::filesystem::path(std::string(path.data(),length));
  if(executable.parent_path()!=std::filesystem::path("/opt/vlike/runtime"))throw std::runtime_error("runtime_unavailable");
  return executable.parent_path().generic_string()+"/";
#else
  const auto* path=SDL_GetBasePath();if(!path)throw std::runtime_error("runtime_unavailable");return path;
#endif
}
CoreModule load_core_module(const std::string& path){
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(_WIN32)
  const auto native=std::filesystem::u8path(path);
  if(!native.is_absolute())return nullptr;
  // Only this reviewed package directory and System32 resolve static imports.
  // No process-wide DLL search setting or current-directory fallback.
  return LoadLibraryExW(native.c_str(),nullptr,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR|LOAD_LIBRARY_SEARCH_SYSTEM32);
#elif defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(__APPLE__)
  if(!std::filesystem::path(path).is_absolute())return nullptr;
  return dlopen(path.c_str(),RTLD_NOW|RTLD_LOCAL|RTLD_FIRST);
#elif defined(OMNIPLAY_GUEST_ACTOR) && defined(__linux__)
  if(!std::filesystem::path(path).is_absolute())return nullptr;
  return dlopen(path.c_str(),RTLD_NOW|RTLD_LOCAL);
#else
  return SDL_LoadObject(path.c_str());
#endif
}
CoreSymbol core_module_symbol(CoreModule module,const char* name){
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(_WIN32)
  return reinterpret_cast<CoreSymbol>(GetProcAddress(module,name));
#elif defined(OMNIPLAY_DEVICE_FREE_ACTOR) && (defined(__APPLE__) || defined(OMNIPLAY_GUEST_ACTOR))
  return reinterpret_cast<CoreSymbol>(dlsym(module,name));
#else
  return SDL_LoadFunction(module,name);
#endif
}
void unload_core_module(CoreModule module){
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(_WIN32)
  if(!FreeLibrary(module))throw std::runtime_error("runtime_crashed");
#elif defined(OMNIPLAY_DEVICE_FREE_ACTOR) && (defined(__APPLE__) || defined(OMNIPLAY_GUEST_ACTOR))
  if(dlclose(module)!=0)throw std::runtime_error("runtime_crashed");
#else
  SDL_UnloadObject(module);
#endif
}
std::uint64_t runtime_ticks(){
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  return static_cast<std::uint64_t>(std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now().time_since_epoch()).count());
#else
  return SDL_GetTicks();
#endif
}
}
