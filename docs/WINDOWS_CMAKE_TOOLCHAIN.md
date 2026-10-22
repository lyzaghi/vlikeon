# Portable Windows CMake prerequisite

`tools/prepare_windows_cmake.py` prepares official CMake 3.31.6 x64 bin/share
files under a fresh project-local directory. The archive is checked against
its entry in the official published SHA-256 list. No installer, registry edit,
persistent PATH change or executable invocation occurs during preparation.
These checks establish distribution/download integrity, not independent signing.

```powershell
python tools/prepare_windows_cmake.py --project out/toolchains/cmake-3.31.6-windows-v1
```

The script rejects linked ancestry, unsafe/reserved paths, nonordinary selected
entries, case aliases, oversized downloads and expansion. Original archives
retain notices. Partial output is preserved without a success receipt and must
not be used or overwritten. The first completed local preparation contains
3,777 files / 75,419,105 bytes with individual hashes and the archive hash in
its ignored receipt. No compiler/runtime behavior is inferred from extraction.

Antigravity batch 011 first executed this CMake with existing VS2022/Windows
SDK and the frozen worker/SDL/JSON/libretro sources. Compiler detection and
configuration ran, but generation rejected a missing nlohmann_json.natvis.
Codex corrected the source materializer to include that exact tracked file at
the pinned JSON revision, rather than changing the upstream build requirements.
No worker/fixture binary or runtime pass resulted from the failed batch.

Selected subsequent batches may configure/build the explicit qualification
worker and fixture DLLs in isolated output directories. Execution of those
artifacts remains a separate exact assignment; fixture IDs never enter product
catalogs. Original core, helper, OS, dependency-license and device requirements
remain open.

Distribution and checksum list:

- https://cmake.org/files/v3.31/cmake-3.31.6-windows-x86_64.zip
- https://cmake.org/files/v3.31/cmake-3.31.6-SHA-256.txt
