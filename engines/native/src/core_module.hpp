#pragma once
#include <cstdint>
#include <string>
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(_WIN32)
#include <windows.h>
#elif !defined(OMNIPLAY_DEVICE_FREE_ACTOR)
#include <SDL3/SDL.h>
#endif

namespace omni {
#if defined(OMNIPLAY_DEVICE_FREE_ACTOR) && defined(_WIN32)
using CoreModule=HMODULE;
#elif defined(OMNIPLAY_DEVICE_FREE_ACTOR)
using CoreModule=void*;
#else
using CoreModule=SDL_SharedObject*;
#endif
using CoreSymbol=void(*)();
std::string executable_base_directory(); // UTF-8, trailing separator.
CoreModule load_core_module(const std::string& absolute_utf8_path);
CoreSymbol core_module_symbol(CoreModule,const char* name);
void unload_core_module(CoreModule);
std::uint64_t runtime_ticks();
}
