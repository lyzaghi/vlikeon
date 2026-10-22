# Native memory-files final qualification

Authored only. Configure/build/test only after the complete source-authoring
phase, inside item 16. Use the exact qualified C++20 Windows x64 toolchain,
record source/artifact/SDK identities and retain output for each failure case.

1. Build the standalone `qa/native-memory` CMake project in a fresh qualification
   output and explicitly run its `memory-files-contract` case. Exercise both
   direct executable and registered CTest selection under the final pass plan.
   These cases require actual store operations; an unexecuted source check does
   not pass them. They cover sparse zeroing, page charge, shrink/regrow,
   unlink-with-open-handle debt, aliases, directory moves, readonly rights,
   revision listing, non-reused handles, staged atomic replacement, empty
   directories, scratch preservation, bounds and file/handle caps.
   When using the frozen presentation source layout, explicitly select its
   `source/native/src` through `OMNIPLAY_NATIVE_MEMORY_SOURCE`; the default points
   to `engines/native/src` in the complete product workspace. Record that exact
   source selection with the case output.
2. Inject allocation failure at each new page, pointer array, candidate map,
   path/list/capture buffer and replacement stage allocation. Compare old bytes,
   reachable/unlinked nodes, counters, revisions and owned handles. No partially
   committed growth or namespace/ownership transfer may escape a rejection.
3. Exercise mixed concurrent reads/writes/listing/rename/unlink/resize and
   replacement across save and scratch. Cover cross-root rename rejection,
   path/Unicode/device aliases, deep/long subtree moves, metadata exhaustion,
   near-maximum save envelopes, page rounding and repeated replacement. Capture
   must exclude scratch, obey common archive admission and preserve file bytes.
4. Build the exact new MD core and worker through their recorded recipes. Admit
   the exact protocol/header/adapter/artifact receipts; reject missing or stale
   declarations. Exercise absent/rejected install export, incorrect table
   versions/sizes, missing callbacks, false prepare acknowledgement and caller
   attempts to enable the route for another core/backend/platform.
5. Observe actual Windows ACL/mandatory labels and token/job state. With protocol
   1, attempts to write the work save/scratch directories or default storage
   must fail while actual MD memory-file saves work. Neither worker nor core may
   retry by granting OS write access. Observe filesystem changes and source/
   artifact imports; a store unit case cannot establish this isolation.
6. Use approved independently checked MD fixtures: no nonvolatile memory, byte/
   word SRAM, EEPROM variants, user SVP firmware and three/six-button controls.
   Confirm canonical OPSF0002 progress and OPNS0002 guest state, native/Web
   compatibility, selection/history/CAS behavior and no extra conversion rule.
7. Cover load rejection before stage commit and after guest restart, core/state
   rejection, OOM under the 1536-MiB job, export/save/final-save failures, crash
   and broker close. Prior committed history must survive; live guest/file
   handles must retire before replace/uninstall. Observe actual peak memory,
   CPU/latency, pagefile effects, frametime and disk/resource behavior. No hard
   no-disk or single-budget peak claim follows from the memory store.

Final qualification still includes every other core, helper/driver cache,
physical-accounting/recovery and original product/release requirement.
