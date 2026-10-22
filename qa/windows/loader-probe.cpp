// Test-only staged loader observations under the unchanged production sandbox.
// Link static CRT and kernel32 only; all optional system DLLs load after entry.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <cstdio>

int wmain() {
  std::puts("storage-loader-start");std::fflush(stdout);
  const struct {const wchar_t* library;const char* observation;} steps[]={
    {L"advapi32.dll","storage-loader-advapi32"},
    {L"combase.dll","storage-loader-combase"},
    {L"userenv.dll","storage-loader-userenv"},
    {L"user32.dll","storage-loader-user32"},
    {L"ole32.dll","storage-loader-ole32"},
  };
  for(const auto& step:steps){
    if(!LoadLibraryExW(step.library,nullptr,LOAD_LIBRARY_SEARCH_SYSTEM32)){
      const auto error=GetLastError();
      std::printf("storage-loader-failed %s %lu\n",step.observation,error);
      std::fflush(stdout);return 5;
    }
    std::puts(step.observation);std::fflush(stdout);
  }
  std::puts("storage-loader-complete");std::fflush(stdout);return 0;
}
