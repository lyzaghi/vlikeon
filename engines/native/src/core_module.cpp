#include "core_module.hpp"
#include <chrono>
#include <filesystem>
#include <stdexcept>
#include <vector>

namespace omni {
std::string executable_base_directory(){
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  std::vector<wchar_t> path(32768);
  const auto length=GetModuleFileNameW(nullptr,path.data(),static_cast<DWORD>(path.size()));
  if(!length||length>=path.size())throw std::runtime_error("runtime_unavailable");
  const auto directory=std::filesystem::path(std::wstring(path.data(),length)).parent_path();
  if(!directory.is_absolute())throw std::runtime_error("runtime_unavailable");
  const auto utf8=directory.generic_u8string();
  return std::string(reinterpret_cast<const char*>(utf8.data()),utf8.size())+"/";
#else
  const auto* path=SDL_GetBasePath();if(!path)throw std::runtime_error("runtime_unavailable");return path;
#endif
}
CoreModule load_core_module(const std::string& path){
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  const auto native=std::filesystem::u8path(path);
  if(!native.is_absolute())return nullptr;
  // Only this reviewed package directory and System32 resolve static imports.
  // No process-wide DLL search setting or current-directory fallback.
  return LoadLibraryExW(native.c_str(),nullptr,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR|LOAD_LIBRARY_SEARCH_SYSTEM32);
#else
  return SDL_LoadObject(path.c_str());
#endif
}
CoreSymbol core_module_symbol(CoreModule module,const char* name){
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  return reinterpret_cast<CoreSymbol>(GetProcAddress(module,name));
#else
  return SDL_LoadFunction(module,name);
#endif
}
void unload_core_module(CoreModule module){
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  if(!FreeLibrary(module))throw std::runtime_error("runtime_crashed");
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
