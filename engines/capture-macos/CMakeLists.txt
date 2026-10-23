cmake_minimum_required(VERSION 3.25)
project(VlikeonMacCapture LANGUAGES CXX)
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Darwin")
  message(FATAL_ERROR "The ScreenCaptureKit adapter requires a real macOS SDK")
endif()
enable_language(OBJCXX)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_OBJCXX_STANDARD 20)
set(CMAKE_OBJCXX_STANDARD_REQUIRED ON)
set(CMAKE_OBJCXX_EXTENSIONS OFF)
# This is a library for a trusted, signed host. It is not an executable,
# admission rule, renderer API or permission grant.
add_library(vlikeon-macos-capture STATIC src/capture.mm src/encoder.mm)
target_include_directories(vlikeon-macos-capture PUBLIC include)
target_compile_options(vlikeon-macos-capture PRIVATE
  -fobjc-arc -fblocks -Wall -Wextra -Wpedantic -fstack-protector-strong)
target_link_libraries(vlikeon-macos-capture PUBLIC
  "-framework AppKit" "-framework ScreenCaptureKit" "-framework CoreMedia"
  "-framework CoreVideo" "-framework CoreGraphics" "-framework Foundation"
  "-framework VideoToolbox")
set_target_properties(vlikeon-macos-capture PROPERTIES
  OSX_DEPLOYMENT_TARGET 13.0 POSITION_INDEPENDENT_CODE ON)
