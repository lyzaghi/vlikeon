# PS1 memory-card final qualification

Authored cases only. Run after all source authoring, within item 16. Record exact
source, toolchain/SDK, header, core/worker and package hashes for every case.

1. Build this C/C++20 project with OMNIPLAY_PS1_SOURCE pointing to the exact
   prepared native3 PS1 source, and OMNIPLAY_NATIVE_SOURCE to the current
   engines/native directory. For a frozen source selection, explicitly select
   its inventoried directories. Execute ps1-memory-bridge-contract. It compiles
   the real C bridge/store and pinned atomic headers; unexecuted cases do not
   pass, and missing dependencies must fail rather than silently skip.
2. Observe full 128-KiB bytes, separator normalization, no OS fallback, absent
   cards and path/table/size rejection. Inject read/allocation/close faults:
   guest NV stays unchanged, and cleanup cannot publish until a new quiescent
   load clears the error. Malformed/read-error cards cannot become blank cards.
3. Inject partial write, flush/close failure and failed rename. Retain the old
   complete card and retire only its private temporary. Exhaust actual store
   handle/file/page budgets, restore capacity and retry. Observe independent
   concurrent paths and quiescent install/uninstall callback/handle lifetime.
4. Build/admit exact PS1 core/worker/broker artifacts. Reject missing exports,
   stale native2 receipts, changed ABI/header/FrontIO/async/barrier hashes,
   unsupported identities/backend/platform and false prepare replies. Require
   actual Windows token/job/default-storage isolation and denied OS work
   save/scratch writes alongside successful memory publication.
5. Exercise default SRAM, file-managed first/second/multitap cards, every admitted
   index/port and unused cards. Compare independent fixtures/checksums. Failed
   initial load/unload must retain original or malformed memory-tree bytes and
   all previously committed history; no silent formatting or overwrite.
6. Interleave async snapshots, pause/join/lazy restart and index swaps. Drain old
   jobs before inline publication. Failed swap must stop selection/format.
   Exercise writer allocation/thread/queue-slot fallback, real stalled I/O and
   process termination, repeat lifecycles and final sanitizer configurations.
7. Cover OPNM0001/OPSF0002 progress and disc/tray OPNS0002 snapshots, rejected
   state and successful/failed old-state recovery, read-error capture refusal,
   save/export/final-save overlap and broker/core crash. No new automatic
   native2/native3/Web metadata exception exists; test original cross-version
   requirements independently.
8. Verify approved firmware, regions, multidisc/media, input/audio/video, actual
   memory/pagefile/frametime/device/OS behavior and compiled licensing/source/
   artifact content. Bridge cases do not prove complete containment, gameplay
   compatibility, physical accounting or release readiness.

Other cores, helper/driver caches, legacy recovery and all original product
requirements remain in the same final consolidated verification.
