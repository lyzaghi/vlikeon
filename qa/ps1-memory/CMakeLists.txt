cmake_minimum_required(VERSION 3.25)
project(OmniPlayPs1MemoryBridgeCases LANGUAGES C CXX)
# Source only: configure/build/run exclusively in the final consolidated pass.
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(OMNIPLAY_PS1_SOURCE "" CACHE PATH "Exact prepared pinned native PS1 source")
set(OMNIPLAY_NATIVE_SOURCE "${CMAKE_CURRENT_SOURCE_DIR}/../../engines/native" CACHE PATH "Exact current native frontend source")
if(NOT EXISTS "${OMNIPLAY_PS1_SOURCE}/libretro-common/include/retro_atomic.h" OR NOT EXISTS "${OMNIPLAY_NATIVE_SOURCE}/adapters/beetle-memory-files.c")
  message(FATAL_ERROR "Select the exact prepared PS1 and current native source directories")
endif()
find_package(Threads REQUIRED)
add_executable(omniplay-ps1-memory-cases ps1_memory_cases.cpp
  "${OMNIPLAY_NATIVE_SOURCE}/adapters/beetle-memory-files.c"
  "${OMNIPLAY_NATIVE_SOURCE}/src/memory_files.cpp")
target_include_directories(omniplay-ps1-memory-cases PRIVATE
  "${OMNIPLAY_NATIVE_SOURCE}/src" "${OMNIPLAY_NATIVE_SOURCE}/adapters"
  "${OMNIPLAY_PS1_SOURCE}/libretro-common/include")
target_link_libraries(omniplay-ps1-memory-cases PRIVATE Threads::Threads)
if(MSVC)
  target_compile_definitions(omniplay-ps1-memory-cases PRIVATE NOMINMAX WIN32_LEAN_AND_MEAN)
  target_compile_options(omniplay-ps1-memory-cases PRIVATE /W4 /utf-8 /GS /guard:cf)
  target_link_options(omniplay-ps1-memory-cases PRIVATE /DYNAMICBASE /NXCOMPAT /HIGHENTROPYVA /guard:cf)
else()
  target_compile_options(omniplay-ps1-memory-cases PRIVATE -Wall -Wextra -Wpedantic -fstack-protector-strong)
endif()
enable_testing()
add_test(NAME ps1-memory-bridge-contract COMMAND omniplay-ps1-memory-cases)
