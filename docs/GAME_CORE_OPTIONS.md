# Local per-game core choices

Authored source now connects finite per-game/per-runtime choices across the
shared UI, desktop/mobile Rust preferences, standalone IndexedDB, native
package admission/preparation and the WASM worker. Choices apply on the next
launch; no active core is reconfigured. No parser, database transaction,
UI/browser/mobile API, worker/core, compiler, build or test has run.

## Declared choices and fixed execution policies

`CoreOptionPolicy` schema 1 contains at most 32 keyed definitions, 32 choices
per definition, 256 choices total and 64 KiB encoded metadata. Keys/runtime
identities are bounded ASCII identifiers; choice values are nonempty printable
ASCII up to 128 bytes. Both languages have bounded labels/descriptions. Keys,
choices and defaults must be distinct/declared. Rust requires keyed objects
and rejects duplicate profile keys, unknown fields and positional struct arrays.
Shared JS and Python admission enforce the corresponding schema/count/byte
rules. The finite list is a package declaration, not a guess from arbitrary
runtime-advertised variables.

Known CPU/JIT, unsafe fast-memory, backend/renderer/thread, network, firmware/
path and cheat key families cannot be exposed through these policies. Actual
core-by-core semantics, resource limits, source identity and release admission
remain required; this name filter alone cannot prove an arbitrary core option
safe. Qualified runtime packages remain the place to select an execution/GPU
profile. A UI request cannot replace those fixed package policies.

Web manifests need `core_options_protocol: 1` and explicit
`core_option_definitions`; the shared source assembler admits them. Native
packages use `user_core_options`; every exposed default must equal the signed
fixed `core_options` entry. Native preparation accepts stored overrides only
for those declared keys/values, before sending configuration to its worker.
The renderer does not supply the native allowlist. WASM repeats policy/choice
validation before core initialization and acknowledges core-options protocol 1.

The PS1 blueprint at `engines/policies/beetle-options.json` has boot-animation,
fallback-region and second-card choices. Their values/defaults follow the exact
pinned source. The Web candidate retains its policy file/hash alongside source;
its second-card declaration requires the persistent-file envelope/barrier route.
The native PS1 candidate now also exposes the second-card choice through its
checked combined SRAM/file save route; see `NATIVE_PS1_SAVES.md`. Both source and
worker/package admission reject an unsupported selectable second card. No core
binary is installed or approved by writing this metadata.

## Profile storage and recovery

`GameCoreOptions` is `{schema_version:1, values:{...}}`. Storage is local to the
current profile and keyed by game UUID/runtime ID, with at most 64 entries and
16 KiB per record. Saving an empty map removes the record. Transactions require
an existing library game, admit at most 32 nonempty runtime records per game
and 65536 globally, and record only the audit action/game ID. Values do not
enter sync, diagnostics or recommendation prompts.

SQLite uses the existing preferences table; IndexedDB uses the existing config
store with strict transactions and key-range counts. No schema migration or
whole-config value scan is needed. A bounded list API returns runtime IDs only,
so the UI can reset obsolete/unavailable runtime profiles and release their
quota. A game removal atomically removes its core-choice records with the
library entry. Existing save-retention requirements still apply to removal.

Reads preserve saved values. If the installed policy removed a key/value,
launch fails with a review/reset error instead of silently substituting another
choice. The panel leaves previous committed progress available and offers an
explicit local reset; missing/corrupt choices are not automatically rewritten.
Backend storage bounds are distinct from installed-policy validation at launch.
This permits retaining an unavailable runtime's profile without granting it
execution rights.

## UI and snapshot compatibility

The Vietnamese/English game-detail panel offers available browser/native
runtime choices and explicit controls from their policies, separate drafts,
save and reset. Unavailable saved runtime entries remain visible for cleanup.
Metadata/records load through bounded requests. Mount/revision ownership
prevents old asynchronous responses repainting a replaced panel. Controls are
disabled during mutations; descriptions use text nodes and accessibility
associations. No arbitrary path/text core-variable editor is exposed.

Web preparation resolves declared defaults plus overrides in stable key order,
then includes the actual effective map in its existing configuration hash.
Native preparation overlays the resolved map into its signed fixed options,
which already feed snapshot/measurement compatibility. Changing a setting
therefore preserves an older snapshot but prevents loading it under a different
effective configuration. In-game saves keep their existing compatibility policy.
No-policy/empty-profile raw configurations retain their prior hash shape.

A related source finding in pinned PS1 showed snapshot card bytes were omitted
when the core's private `data_used` flag remained false after direct frontend
SRAM restoration. Shared native/Web source adaptation now always includes
card storage in snapshots, while retaining the upstream flags/guest behavior.
The native recipe/package use an explicit adapted version; Web already has its
own adapted version. The exact source change and actual core states still need
the consolidated verification, including SRAM restored before any guest write.

## Remaining consolidated evidence

The single final pass must compile the Rust/native/WASM paths and cover strict
objects/duplicates/keys/values/defaults, all count/UTF-8/encoded bounds and fixed
policy families; profile ownership, concurrent transaction/quota boundaries,
empty/reset/list/obsolete/removed-game handling, malformed persisted records and
storage denial; policy upgrades and stale choices, unchanged raw configurations,
effective key ordering, snapshot/measurement hashes and preserved progress;
UI keyboard/assistive/touch/draft/reset/stale-response behavior; actual PS1
region/boot/card choices, card bytes with `data_used` false, exact firmware,
restart/rollback and desktop/Android/iOS/device memory/performance evidence.
Every other original product/release requirement remains active.

The SNES Web candidate adds finite aspect-ratio, image-blending, overscan and
existing game-hotfix choices. CPU rates, one-times HD scale, accurate PPU/DSP,
no runahead, no delayed coprocessor synchronization and its selected HLE route
are fixed inside the core adapter. The renderer option map cannot change those
execution choices. The retained SNES policy uses the same shared settings and
save-configuration identity; no policy/UI/core/device behavior was executed.
See `WEB_SNES.md`.

The native MD 0.3 source and WASM candidate share a finite region and
automatic/three-/six-button policy. Automatic retains the pinned compatibility
list; settings apply on the next launch and enter the existing configuration
hash. Backend/thread/TMSS choices remain fixed by the adapter. Native package
generation requires the matching policy/source/library receipt and checked
canonical-file save profile. No choice, device, core or UI was executed. See
`WEB_MEGA_DRIVE.md`.

The current MD canonical progress profile checks its loaded-game RAM/EEPROM
layout independently of region/controller choices. Opaque snapshots still use
the complete runtime/configuration identity. Portable capture does not serialize
controller settings or live host memory; see `MEGA_DRIVE_PORTABLE_SAVES.md`.

Catalog/save source validation now also separates the upstream library string from a SemVer-compatible product/save version, shared by browser save metadata and core resource admission. PS1 recipe metadata retains both values explicitly. Standalone resource paths now require a safe shared bundled directory for JS/WASM without guessing that directory from the runtime ID (whose existing IDs include a `-web` suffix). Source assembly repeats semantic-version admission. No validator, catalog load or save operation has run; the final pass must cover version/path boundaries and all existing raw-core catalogs.
