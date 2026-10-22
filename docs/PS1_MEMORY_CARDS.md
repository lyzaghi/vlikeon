# Windows PS1 memory-card route

Source only. No compiler, core/worker, memory-store operation, Windows API,
application or test has run. No binary/catalog entry is built or approved.
The original item-2 and full 1–16 scope remain open.

## Exact new core and package

The new pinned Beetle PSX source retains upstream revision
`df2b53b306ba52d2116f8e8cc93b9ee33d239be7` and declares library version
`0.9.44.1+omniplay.native3`, product SemVer `0.9.44+1.omniplay.native3`.
It adds a C card bridge to OmniWritableFilesV1, matching ABI header,
FrontIO load/save adaptation and explicit install/uninstall exports. The
source receipt binds each adapter/header hash, protocol, source inventory and
eventual artifact. The packager requires these before emitting
`writable_files_protocol=1` with the checked combined-memory/file route.

Windows admission allows only the exact software PS1 identity/source/system/
version and `save/PSX` contract alongside the separately admitted MD route.
The common worker installs its table before retro_init and requires success;
the broker checks actual prepare acknowledgement and grants no actor OS writes
to work save/scratch. Default filesystem/registry and token/job admission still
precede resume. Missing installation refuses load without a disk-card fallback.

## Card producers and consumers

The adaptation targets pinned FrontIO file-managed-card reads/synchronous saves,
the asynchronous writer callback and checked all-card capture. They use only the
owned memory table. Readonly disc/BIOS/media input paths remain unchanged. This
is not interception of arbitrary host streams or a widened file capability.

The bridge admits exactly 128-KiB cards under `save/PSX/` with finite ASCII
basenames and `.mcr` suffix. Windows separators normalize to slash; nested,
traversal, alternate-stream or unrelated-root names fail. Complete temporary
memory write/flush/close and atomic replacement publish each card. Failure
retains the old complete card. Flush is memory visibility, not disk durability.

Reads distinguish absence from invalid/failed data. Complete checked bytes are
staged before changing guest NV. Only confirmed absence may use the existing
forced-selection new-card formatting. Wrong size, allocation/read/close failure
or invalid path latches an error. That error blocks subsequent cleanup writes,
so rejected load/unload cannot overwrite retained data with newly initialized
guest bytes. Initial load refuses admission; later read/selection failure
requests paused close and makes the checked barrier reject capture.

The bounded upstream asynchronous writer remains. Pause/capture/unload joins it
before replacement/uninstall; guest-frame execution restarts it lazily. Index
swaps additionally drain old jobs before an inline publication, preventing an
older queued image from replacing a newer image. Failed synchronous publication
stops the swap. The checked barrier still rewrites every allocated card because
periodic dirty counters alone do not prove publication. Unused cards remain
included. The default first card stays SRAM; files are captured alongside it.

## Formats and compatibility limits

OPNM0001 RAM/RTC plus OPSF0002 files and disc-aware OPNS0002 snapshots retain
their byte formats and 256-MiB envelope bound. Second-card selection, firmware/
disc policies and checked guest/file recovery remain. This change admits no
new native/Web conversion or automatic native2-to-native3 exception. Runtime
version/artifact/configuration checks still apply; the writable-file policy now
enters native configuration identity when declared. Cross-version migration
remains original product work requiring independent evidence.

Current selected source is
`out/native-sources/item2-ps1-memory-cards-source`; earlier native2 source stays
preserved. Preparation invoked no Make/compiler/linker/core or test.

## Required final evidence

The unexecuted `qa/ps1-memory` cases compile the real C bridge with the actual
memory store and pinned atomic headers. They cover complete round trips,
missing-versus-failed reads, unchanged caller NV, invalid paths/install, partial
write/flush/close/rename preservation, actual handle exhaustion and independent
card writers. The complete FrontIO/async/Windows/game/restore matrix is in
`qa/ps1-memory/ACCEPTANCE.md`; these standalone cases do not replace it.

Other cores, helper/driver caches, physical accounting, legacy ownership
recovery and all original platform/license/release requirements remain due.
This source does not close Windows resource containment.
