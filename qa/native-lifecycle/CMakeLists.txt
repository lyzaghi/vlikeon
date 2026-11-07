cmake_minimum_required(VERSION 3.25)
project(OmniPlayLifecycleQualification LANGUAGES CXX)
# Authored only; configure/build/run in the final item-16 pass.
if(NOT WIN32)
  message(FATAL_ERROR "This owned-worker qualification fixture currently targets Windows")
endif()
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(OMNIPLAY_LIBRETRO_INCLUDE "" CACHE PATH "Exact qualified libretro interface directory")
if(NOT EXISTS "${OMNIPLAY_LIBRETRO_INCLUDE}/libretro.h")
  message(FATAL_ERROR "Supply the reviewed pinned libretro interface")
endif()
find_package(Threads REQUIRED)
foreach(target omniplay-lifecycle-fixture omniplay-lifecycle-void-fixture)
  add_library(${target} SHARED fixture_core.cpp)
  target_include_directories(${target} PRIVATE "${OMNIPLAY_LIBRETRO_INCLUDE}")
  target_link_libraries(${target} PRIVATE Threads::Threads)
  set_target_properties(${target} PROPERTIES PREFIX "")
  if(MSVC)
    target_compile_options(${target} PRIVATE /W4 /EHsc /permissive- /utf-8 /GS /guard:cf)
    target_link_options(${target} PRIVATE /DYNAMICBASE /NXCOMPAT /HIGHENTROPYVA /guard:cf)
  endif()
endforeach()
target_compile_definitions(omniplay-lifecycle-void-fixture PRIVATE OMNIPLAY_LIFECYCLE_VOID_ONLY=1)

set(OMNIPLAY_SDL_SOURCE "" CACHE PATH "Pinned SDL3 source directory")
set(OMNIPLAY_JSON_SOURCE "" CACHE PATH "Pinned nlohmann-json source directory")
set(OMNIPLAY_GENERATED_INCLUDE "" CACHE PATH "Generated runtime_identity.hpp directory")
set(OMNIPLAY_SDL_STATIC_LIB "" CACHE FILEPATH "Prebuilt SDL3-static library")
set(OMNIPLAY_PRESENTATION_CLIENT_LIB "" CACHE FILEPATH "Prebuilt omniplay-presentation-client library")
set(OMNIPLAY_PRESENTATION_WIRE_LIB "" CACHE FILEPATH "Prebuilt omniplay-presentation-wire library")

if(OMNIPLAY_SDL_STATIC_LIB AND EXISTS "${OMNIPLAY_SDL_STATIC_LIB}")
  add_executable(omniplay-media-input-tests
    media_input_tests.cpp
    "${CMAKE_CURRENT_SOURCE_DIR}/../../engines/native/src/protocol.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/../../engines/native/src/gl_video.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/../../engines/native/src/vk_video.cpp"
  )
  target_include_directories(omniplay-media-input-tests PRIVATE
    "${OMNIPLAY_LIBRETRO_INCLUDE}"
    "${OMNIPLAY_SDL_SOURCE}/include"
    "${OMNIPLAY_JSON_SOURCE}/single_include"
    "${OMNIPLAY_GENERATED_INCLUDE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/../../engines/native/src"
    "${CMAKE_CURRENT_SOURCE_DIR}/../../engines/presentation/src"
  )
  target_compile_definitions(omniplay-media-input-tests PRIVATE
    NOMINMAX
    _CRT_SECURE_NO_WARNINGS
    _SILENCE_CXX20_U8PATH_DEPRECATION_WARNING
  )
  target_link_libraries(omniplay-media-input-tests PRIVATE
    "${OMNIPLAY_PRESENTATION_CLIENT_LIB}"
    "${OMNIPLAY_SDL_STATIC_LIB}"
    "${OMNIPLAY_PRESENTATION_WIRE_LIB}"
    kernel32 user32 gdi32 winmm imm32 ole32 oleaut32 version uuid advapi32 setupapi shell32 dinput8
  )
  if(MSVC)
    target_compile_options(omniplay-media-input-tests PRIVATE /EHsc /permissive- /utf-8 /GS /guard:cf)
    target_link_options(omniplay-media-input-tests PRIVATE /DYNAMICBASE /NXCOMPAT /HIGHENTROPYVA /guard:cf)
  endif()
endif()
