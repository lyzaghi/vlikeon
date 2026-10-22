cmake_minimum_required(VERSION 3.25)
project(OmniPlayMacOwner LANGUAGES OBJCXX)
if(NOT CMAKE_SYSTEM_NAME STREQUAL "Darwin")
  message(FATAL_ERROR "The OmniPlay XPC owner is a macOS-only target")
endif()
set(OMNIPLAY_BROKER_REQUIREMENT "" CACHE STRING "Exact designated requirement for the signed OmniPlay broker")
set(OMNIPLAY_OWNER_BUNDLE_ID "" CACHE STRING "Approved XPC owner bundle identifier")
set(OMNIPLAY_RUNTIME_CATALOG "" CACHE FILEPATH "Closed signed-bundle runtime catalog")
if(OMNIPLAY_BROKER_REQUIREMENT STREQUAL "" OR OMNIPLAY_BROKER_REQUIREMENT MATCHES "[<>&]" OR NOT OMNIPLAY_OWNER_BUNDLE_ID MATCHES "^[A-Za-z0-9-]+(\\.[A-Za-z0-9-]+){2,}$" OR NOT EXISTS "${OMNIPLAY_RUNTIME_CATALOG}")
  message(FATAL_ERROR "Explicit broker code requirement and owner bundle identity are required")
endif()
configure_file(Info.plist.in ${CMAKE_CURRENT_BINARY_DIR}/Info.plist @ONLY)
configure_file("${OMNIPLAY_RUNTIME_CATALOG}" "${CMAKE_CURRENT_BINARY_DIR}/runtime-catalog.json" COPYONLY)
set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/runtime-catalog.json" PROPERTIES MACOSX_PACKAGE_LOCATION Resources)
add_executable(omniplay-macos-owner MACOSX_BUNDLE src/main.mm src/session.mm src/catalog.mm src/process.mm "${CMAKE_CURRENT_BINARY_DIR}/runtime-catalog.json")
set_target_properties(omniplay-macos-owner PROPERTIES
  BUNDLE_EXTENSION xpc MACOSX_BUNDLE_INFO_PLIST ${CMAKE_CURRENT_BINARY_DIR}/Info.plist
  BUILD_RPATH "" INSTALL_RPATH "" XCODE_ATTRIBUTE_ENABLE_HARDENED_RUNTIME YES
  XCODE_ATTRIBUTE_CODE_SIGN_INJECT_BASE_ENTITLEMENTS NO
  XCODE_ATTRIBUTE_CODE_SIGN_ENTITLEMENTS "${CMAKE_CURRENT_SOURCE_DIR}/macos-owner.entitlements")
target_compile_features(omniplay-macos-owner PRIVATE cxx_std_20)
target_compile_options(omniplay-macos-owner PRIVATE -fobjc-arc -fblocks -Wall -Wextra -Wpedantic -fstack-protector-strong)
target_link_options(omniplay-macos-owner PRIVATE -Wl,-pie -Wl,-dead_strip -Wl,-fatal_warnings)
target_link_libraries(omniplay-macos-owner PRIVATE "-framework Foundation" "-framework Security")
