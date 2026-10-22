# User firmware in shared WASM sessions

The shared desktop/mobile WASM player now has authored firmware-vault reads,
bounded worker transfer/admission and a read-only `/system` mount. No vault/file,
transfer, hash, filesystem, WASM/core, compiler, browser/mobile application or
test has run. No firmware is supplied, downloaded or synthesized by this change.
An empty development core catalog remains explicit; this does not make PS1, NDS,
SNES or another advanced core playable or qualified on any platform.

The dedicated SNES WASM source recipe now removes the embedded upstream IPL
array and requires the exact 64-byte selected `snes_spc_ipl` role at
`/system/ipl.rom`. It rejects failed required resources and does not admit a
binary by preparing source. Additional enhancement-chip firmware/sidecar paths
and real game/device qualification remain required; see `WEB_SNES.md`.

The authored MD WASM candidate also stages an optional selected 2-KiB
`mega_drive_svp` item as `svp.rom`. The adapter requires that exact file for
SVP content while ordinary cartridges do not request it. The selected source
omits the resource aggregator and automatic upstream firmware fallback. No
firmware/core was executed or supplied; see `WEB_MEGA_DRIVE.md`.

A native `firmware_read_start` command accepts a library game ID, supported role
and expected selected hash. The native host derives the system from that game,
requires the role in its existing bounded vault policy, opens and hashes its
currently selected object and rejects a changed selection. It returns a normal
bounded transfer with actual hash/size and retained descriptor, never an
arbitrary host path, historical unselected object or renderer-chosen filename.
The vault also rechecks its selected object's role size before reading it.
Its ordinary profile/origin boundary and transfer expiry/abort remain in place.
This is the existing trusted local firmware permission, not a network download.

Core metadata may declare at most eight distinct, bounded flat system filenames,
supported roles and allowed role sizes. SNES SPC IPL, Mega Drive SVP, three PS1
regional BIOS roles and DS BIOS7/BIOS9/firmware follow the native vault's existing
size policy. An entry is required by default; an explicit optional entry can be
absent, and a required group needs at least one selected member. Optional and
required-group declarations cannot conflict. A declaration must belong to the
core's listed system(s) in resource assembly. The source assembler refuses
malformed declarations or missing firmware protocol 1 before packaging a core.
Assembly admission is not runtime/device verification.

Before copying game media, the player reads the local selected-role inventory,
checks ambiguity/sizes and requests each relevant selected firmware with its
observed hash. A missing required selection gives a Settings action message;
there is no silent manufacturer file or replacement-firmware fallback in this
transport. Transfers reuse the 48 KiB chunk/offset/hash checks and preparation
cancellation. Individual firmware is at most 512 KiB and worker total admission
is at most four MiB. A selection changing midway fails rather than silently
loading a different version. Multiple selected group members are all passed as
declared files; a core can still require the actual region matching its game.

The runtime transfers ordinary ArrayBuffers into its dedicated worker. The worker
validates the policy, distinct filenames, role/size/byte agreement, required
groups and each SHA-256 again before mounting anything. It does not accept
absolute/nested/traversal paths, custom roles, SharedArrayBuffer firmware or
unlisted files. It mounts `/system` before `retro_init`, so core initialization
and game loading see the same admitted bytes through the ordinary libretro
system-directory environment. Exact returned files and their memory remain owned
by that worker; no asynchronous task borrows a native or WASM pointer.

The existing read-only FS backend now accepts only its two explicit mountpoints:
`/game` and `/system`. Firmware memory records provide positional reads, checked
seeks and immutable metadata; write/create/delete/rename/link/metadata operations
remain denied and mmap remains unsupported. The linked C constants frontend is
required whenever this mount is used. Core-specific mutable configuration or
calibration files need a separate persisted-data path; this read-only system mount
does not claim to implement those writes or a full directory of PSP system files.

The player binds sorted filename/role/hash/length identity into its existing WASM
configuration hash. Choosing another firmware version or different optional
files therefore changes save compatibility rather than loading a snapshot under
different system data. Cores with no firmware requirements keep their prior
configuration-hash shape, so unrelated raw-ROM save metadata is not needlessly
changed. Unload/deinit occurs before the system mount is retired, including
partial-preparation cleanup. Actual core constructor/initialization/unload access,
rejected snapshots and cleanup still need qualification.

The existing GB/GBC/NES/GBA source recipes record firmware protocol 1 with an
empty policy; they do not declare a new external BIOS requirement or advanced
core support. Future advanced WASM build recipes must supply their actual pinned
filename/role policy, core options and source adaptations, and pass the same
artifact/license/release admission. Standalone Web still has no firmware vault;
adding this native transport does not create a Web firmware-import UI or claim
that unavailable capabilities work there. Firmware remains outside save sync.

The single final verification pass must compile all affected native and WASM
frontends and cover role/game/profile ownership, wrong/stale/missing hashes,
selected role-size corruption, descriptor lifetime and transfer expiry/abort;
policy duplicates/unknown fields/filenames/types/groups; each supported size,
tampered/partial/oversized payloads, required/optional combinations and worker
hash equality; read-only system lookup/read/seek/write refusal before/after core
initialization; bounded memory, transfer cancellation and partial cleanup; save
configuration compatibility, region mismatch and mutable-data failures; and
unchanged no-firmware sessions. Real core builds, licenses/provenance review,
mobile/desktop filesystem/device/store evidence, disc-aware state, native mobile
engines, full background sync/streaming/browser/release work and every original
remaining requirement stay in scope. No source policy proves firmware provenance
or hardware compatibility.

Primary interface contract: [libretro system-directory and game-loading ABI](https://github.com/libretro/libretro-common/blob/master/include/libretro.h).

The standalone Web/PWA backend now also authors these same commands through its local origin vault; see `WEB_FIRMWARE_VAULT.md`. Its IndexedDB version-4 source keeps metadata, payload Blobs and selections separate, hashes actual immutable bytes and checks expected game/selection/revision before transfer. No storage/browser/core operation or test has run. Native vault and transfer implementations remain distinct; their full original qualification/release work stays required.
