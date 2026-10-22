# MD canonical progress v1 — authored, not qualified

The current native and WASM MD source candidates share the explicit in-game
identifier `ares-md-canonical-files-v1`. They emit a complete canonical
OPSF0002 file archive. This is a persistence format for the same immutable game
content, not an execution-point state or a generic cross-core converter. The
native candidate is `0.3.0+ares.9408cb43d494`; the Web candidate is
`0.3.0+ares.9408cb43d494.omweb1`. Both remain unbuilt and unapproved. No codec,
core, storage, sync, device or test has been executed for this feature.

## Loaded-game descriptor

The selected adapter exports `om_md_describe_save(uint32_t *output)`. The caller
owns six consecutive 32-bit words. The function clears them first, describes
only the loaded cartridge's declared Pak save images, catches failures and
writes the completion tag last. It does not run the emulated CPU or expose
memory pointers, filenames chosen by the game, firmware or other user data.
Caller and callee use their target's ordinary uint32 ABI; WASM reads fresh heap
views after the call. The native/JS descriptor readers cache the first accepted
layout and reject a changed layout during the same game session.

| Word | Meaning | Accepted values |
| --- | --- | --- |
| 0 | Completion tag | `0x3143444d` (`MDC1` in little-endian memory) |
| 1 | Descriptor version | `1` |
| 2 | Canonical SRAM bytes | `0` through `1 MiB` |
| 3 | Canonical EEPROM bytes | Exact size for word 5 |
| 4 | SRAM mode | `0` absent, `1` word, `2` upper byte, `3` lower byte |
| 5 | EEPROM type | Table below |

SRAM size is zero exactly when its mode is zero. Word-mode SRAM has an even
size of at least two bytes. All current file profiles must expose zero direct
libretro SRAM/RTC bytes; a memory-plus-files profile cannot opt into this
converter. The descriptor is source/ABI admission, not an attestation that a
game successfully wrote meaningful progress.

| Type | Upstream mode | Bytes |
| --- | --- | ---: |
| 0 | Absent | 0 |
| 1 | X24C01 | 128 |
| 2 | M24C01 | 128 |
| 3 | M24C02 | 256 |
| 4 | M24C04 | 512 |
| 5 | M24C08 | 1,024 |
| 6 | M24C16 | 2,048 |
| 7 | M24C32 | 4,096 |
| 8 | M24C64 | 8,192 |
| 9 | M24C65 | 8,192 |
| 10 | M24C128 | 16,384 |
| 11 | M24C256 | 32,768 |
| 12 | M24C512 | 65,536 |

These sizes follow the pinned
[ares M24C implementation](https://github.com/ares-emulator/ares/blob/9408cb43d4948fc3ea6e152a307a34348df3fe04/ares/component/eeprom/m24c/m24c.cpp).
The adapted Pak writers keep upstream word RAM in most-significant-byte-first
file order and byte RAM/EEPROM as bytes. Live host-memory layout is not the
portable representation. Actual compiler/game equivalence still needs the
final native/WASM comparison.

## Portable payload

All archive integers are uint32 little endian. The first eight bytes are
`OPSF0002`, followed by the file count. Each entry is kind `0`, UTF-8 name byte
count, payload byte count, exact name bytes and payload. There are no directories,
timestamps, compression, hidden SRAM prefixes or trailing bytes.

The only allowed names are `save.ram` and `save.eeprom`, with exactly the
nonzero sizes reported by the loaded-game descriptor. Every required image must
appear once; absent images must not appear. Names are exact ASCII, with no
aliases, subpaths or case normalization. Capture sorts by UTF-8 bytes, so EEPROM
precedes RAM when both exist. Readers accept either entry order after complete
validation. The complete raw archive is bounded at `2 MiB + 128 bytes`; exact
entry/length/EOF checks still reject unused padding.

Capture checks linked entry names/types/sizes before allocating/copying the
complete output. Native reads similarly preflight the bounded expected file set
before copying payload bytes. Restore validates the complete target against the
current loaded-game contract before stopping the healthy guest or replacing its
files. The existing stop/reload/prior-files-and-state recovery remains in use.
Unexpected private files are preserved by snapshots; they prevent a portable
capture instead of being silently dropped. A changed core layout is terminal.

An empty archive is a valid representation for old reads and full snapshots
of a cartridge without persistent memory. The current profile reports no in-game
save capability for such a game and refuses a new empty progress capture. Its
execution-point snapshot remains separate. Pending hardware EEPROM transactions
are not forcibly committed by the converter; opaque states retain their own
strict compatibility rules.

## Admission, metadata and history

Native catalog admission restricts this format to the exact selected MD
version/source, the checked file-only `save/MD` root, pause/I/O protocol and
single-system declaration. The worker must acknowledge the portable protocol,
resolve the real descriptor export and report the actual loaded-game layout.
Web admission requires its exact version/source, file and cooperative-result
protocols, unthreaded declaration and portable protocol. The player/worker
handshake repeats the selected format. These authored gates do not substitute
for release/device qualification.

In-game metadata uses the same format identifier on both targets and keeps the
exact game-content hash, payload hash and ordinary history/generation/parent
fields. Existing compatibility admits this explicit target format across the
two core IDs/platforms. Opaque snapshots still require their exact core,
version, configuration and platform. Region/controller settings do not redefine
this pinned cartridge file grammar; every target still checks its actual
loaded-game sizes/modes before mutation.

The existing Rust sync importer maps incoming saves to the local game by content
hash. Its encrypted transport remains opaque. Normal import/export and the
standalone Web compatibility reader use the same format relation. This change
does not implement or qualify standalone PWA account synchronization, all other
core converters or any server/device deployment.

## Explicit one-way older readers

The target admits these exact 0.2 in-game metadata combinations:

| Source core/version | Source format | Reader action |
| --- | --- | --- |
| `ares-md-native`, `0.2.0+ares.9408cb43d494` | `ares-md-opsf2-canonical-files-native64` | Validate the direct OPSF0002 archive |
| `ares-md-web`, `0.2.0+ares.9408cb43d494.omweb1` | `ares-md-omweb1-canonical-ram-eeprom.opwh0001` | Unwrap one OPWH0001 with an exactly empty 16-byte OPRM0001 base, then validate OPSF0002 |

Legacy wrapped input has at most 32 additional bytes. Nonzero SRAM/RTC bases,
nested wrappers, older raw-memory formats, arbitrary version prefixes and opaque
states are not conversions. Original stored blobs/metadata are not rewritten;
a subsequent capture creates the new format through normal history/CAS rules.
The reverse reader relation is not declared. Invalid targets leave original
records available for export or a compatible runtime.

## Required final evidence

The consolidated pass must cover independent byte fixtures in both languages,
all descriptor modes/types and malformed/torn/changed descriptors; duplicate,
missing, unknown, directory, zero, oversized, truncated and trailing archive
entries; the two exact legacy readers and every excluded relation; preflight
before copying/mutation and all rollback stages; no-NVM capability/UI behavior;
actual native/WASM cartridges with every relevant RAM/EEPROM mode and regions/
controllers; manual export/import, native encrypted sync, local remapping,
conflicts/tombstones and immutable history; SDK/ABI/memory/lifecycle/fatal queue
behavior; and real desktop/Android/iOS/browser/device evidence. No such execution
has occurred. Native sandbox/platform gaps, remaining core/runtime/browser/
streaming/sync/storage/licensing/store/release work remain original requirements.
