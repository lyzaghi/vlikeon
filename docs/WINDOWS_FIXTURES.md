# OmniPlay Windows Sandbox Fixtures & Ignored Cases (Item 105)

This document provides the recipe, environment variable mapping, and test execution procedures for the 4 Windows-specific sandbox tests that require dedicated native test probes.

---

## 1. Ignored Windows Cases

Located in `platform/sandbox/src/windows/`:
1. `actual_device_free_actor_exchanges_media_under_win32k_lockdown`: Validates that a contained actor under Win32k lockdown cannot call forbidden user32 APIs, but can successfully exchange media with the presentation host via anonymous RGBA/PCM/input pipes. Requires `OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE`, `OMNIPLAY_WINDOWS_DEVICE_FREE_WORKER`, and `OMNIPLAY_WINDOWS_DEVICE_FREE_CORE`.
2. `actual_actor_cannot_write_its_default_storage_but_can_use_private_scratch`: Validates that AppContainer denies write access to default application directories, package roots, and private AppContainer registry while allowing writes only to the designated private scratch directory. Requires `OMNIPLAY_WINDOWS_STORAGE_PROBE`.
3. `actual_actor_without_scratch_grant_cannot_write_private_scratch`: Validates that when scratch capability is withheld, the actor receives access denied when attempting to create files in scratch. Requires `OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE`.
4. `actual_actor_reports_staged_system_dll_initialization`: Batch 020/022 historical diagnostic probe that stages `advapi32`, `combase`, `userenv`, `user32`, and `ole32` inside the headless AppContainer with `JOB_OBJECT_UILIMIT_DESKTOP`. Demonstrates why `user32.dll` (`1114` / `ERROR_DLL_INIT_FAILED`) cannot be loaded by a headless actor and why `storage-probe.cpp` links `windowsapp.lib` instead of `ole32.lib` (see `qa/windows/STORAGE_PROBE.md`).

---

## 2. Compilation and Test Recipe

```powershell
# 1. Compile all four probe executables inside an MSVC x64 developer prompt
py tools/run_windows_fixtures.py --build

# 2. Set the required absolute environment variables
$env:OMNIPLAY_WINDOWS_STORAGE_PROBE = (Resolve-Path "out/qualification/windows-storage/storage-probe.exe").Path
$env:OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE = (Resolve-Path "out/qualification/windows-storage/storage-probe-readonly.exe").Path
$env:OMNIPLAY_WINDOWS_LOADER_PROBE = (Resolve-Path "out/qualification/windows-storage/loader-probe.exe").Path
$env:OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE = (Resolve-Path "out/qualification/windows-presentation/omniplay-presentation-fixture.exe").Path
$env:OMNIPLAY_WINDOWS_DEVICE_FREE_WORKER = (Resolve-Path "out/qualification/worker-device-free/omniplay-native-worker.exe").Path
$env:OMNIPLAY_WINDOWS_DEVICE_FREE_CORE = (Resolve-Path "out/qualification/lifecycle-cores/omniplay-lifecycle-fixture.dll").Path

# 3. Execute the 3 acceptance Windows OS AppContainer/Win32k tests
cargo test --locked --offline -j 2 -p omniplay-sandbox -- --ignored --skip actual_actor_reports_staged_system_dll_initialization --nocapture
```
