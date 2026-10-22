# Windows worker build

Codex inspected the actual Antigravity batch 012 result and raw compiler logs.
All four configure/build steps exited zero, with the v47 source inventory unchanged.
Result records the exact worker and both fixture DLL hashes. Source base is
3b2c2137cf64cb779c084ef8b941498f2bf7e0ee; selected input hashes in the retained QA
inventory are authoritative for materialized dependencies.

Published result omits the duplicated full before/after inventory; its original
is retained in the local batch directory. No downloaded toolchain or binary is
published. MSVC warnings remain; compilation is not runtime qualification.
