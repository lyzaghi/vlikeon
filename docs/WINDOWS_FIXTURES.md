# OmniPlay Windows Sandbox Fixtures & Ignored Cases (Item 105)

This document provides the recipe, environment variable mapping, and test execution procedures for the 4 Windows-specific sandbox tests that require dedicated native test probes.

---

## 1. Ignored Windows Cases

Located in `platform/sandbox/src/windows/`:
1. `actual_device_free_actor_exchanges_media_under_win32k_lockdown`: Validates that a contained actor under Win32k lockdown cannot call forbidden user32 APIs, but can successfully exchange media with the presentation host via shared memory and anonymous pipes. Requires `OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE`.
2. `actual_actor_cannot_write_its_default_storage_but_can_use_private_scratch`: Validates that AppContainer denies write access to default application directories, package roots, and system registry, while allowing writes only to designated private scratch directory. Requires `OMNIPLAY_WINDOWS_STORAGE_PROBE`.
3. `actual_actor_without_scratch_grant_cannot_write_private_scratch`: Validates that when scratch capability is withheld, the actor receives access denied when attempting to create files in scratch. Requires `OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE`.
4. `actual_actor_reports_staged_system_dll_initialization`: Validates staged loader diagnostics and DLL initialization reporting without OLE32 crash 1114. Requires `OMNIPLAY_WINDOWS_LOADER_DIAGNOSTIC_PROBE`.

---

## 2. Compilation and Test Recipe

```powershell
# 1. Generate fixtures and commands
py tools/run_windows_fixtures.py

# 2. Compile probe executables in an MSVC x64 developer prompt
cl /nologo /std:c++20 /EHsc /W4 /WX /MT /O2 /guard:cf /DUNICODE /D_UNICODE qa/windows/storage-probe.cpp /Fe:out/qualification/windows-storage/storage-probe.exe /link /DYNAMICBASE /HIGHENTROPYVA /NXCOMPAT /GUARD:CF advapi32.lib userenv.lib windowsapp.lib

cl /nologo /std:c++20 /EHsc /W4 /WX /MT /O2 /guard:cf /DUNICODE /D_UNICODE /DOMNIPLAY_EXPECT_SCRATCH_DENIED qa/windows/storage-probe.cpp /Fe:out/qualification/windows-storage/storage-probe-readonly.exe /link /DYNAMICBASE /HIGHENTROPYVA /NXCOMPAT /GUARD:CF advapi32.lib userenv.lib windowsapp.lib

# 3. Set environment variables
$env:OMNIPLAY_WINDOWS_STORAGE_PROBE = (Resolve-Path "out/qualification/windows-storage/storage-probe.exe").Path
$env:OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE = (Resolve-Path "out/qualification/windows-storage/storage-probe-readonly.exe").Path

# 4. Execute tests
cargo test --frozen -p omniplay-sandbox --target x86_64-pc-windows-msvc -- --ignored --nocapture
```
