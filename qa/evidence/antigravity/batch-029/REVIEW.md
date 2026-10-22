# Worker and fixture CRT scratch build

Codex inspected four successful CMake/MSVC steps on frozen v50/base3823f29.
All exited zero; full source inventories remained unchanged. Worker and both
fixture DLL identities are in result.json. Raw build warnings remain intact;
fixture warnings about deliberately throwing across C ABI exercise the existing
lifetime suite and are not removed to obtain a pass.

This build includes Win32/CRT private scratch restoration and the actual loaded
core's environment observations. Runtime evidence is separately batch030.
Duplicated full source inventories remain local; no toolchain/binary is published.
