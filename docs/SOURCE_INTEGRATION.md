# Numbered work item 1 — integrated source version

This item uses the owner's new 1–109 outstanding-work numbering. It integrates
the previously uncommitted product source into a single Git version; it does
not claim completion of the old platform roadmap or a production release.

The integrated scope includes mobile lifecycle work, the diskless macOS guest
route, native/web media and input protocols, save/library recovery, owned-game
capture, Web/PWA and encrypted-sync lifecycle, routing and updater changes.
Canonical source paths and shared asset generators remain the source of truth.
No ignored core binaries, user profiles, credentials, caches or local toolchains
belong in this source checkpoint. Third-party notices and source pins remain.

The repository enforces LF for text so immutable upstream blob checks, patch
generation and resource inventories do not depend on clone-time newline policy.
Binary artwork and other binary inputs retain their original bytes.

`generate_browser_patch.py` now composes patch 0003 after the generated base
with exact context/positions. Previously, regenerating resources silently
dropped its owned-game capture contract from the patch queue and staged overlay.
The source freeze recipe includes this new composition helper. Source-level
patch checks do not establish Chromium compilation or window capture behavior.

Qualification of this checkpoint covers clean source extraction, language and
resource consistency, deterministic development asset/patch generation, existing
Rust/JS/Python integration checks and the native frontend compilation assigned
in the fixed Antigravity batch. Platform browser/core/device acceptance remains
in its separately numbered work item. Exact results will be recorded with the
frozen Git tree and source manifest; no passing result is inferred from staging.

## Executed qualification — 2026-10-23

Antigravity executed batches 001a–001c; Codex reviewed raw logs and authored
all fixes. Final execution snapshot: Git tree
`b48294d1a685ba669b996cd51f1de3e9d085d48f` (1,291 files). Full source hashes,
raw-log digests, actual batch exits, input comparisons and scope limits are in
`../qa/evidence/sequential/item-001/`.

Batch 001a passed 53 Rust cases, 13 browser JavaScript cases, 13 release Python
cases, six exact-patch cases and the synthetic native audio-ring contract.
Four explicit Windows fixture tests remain ignored. The batch overall failed.
Batch 001b compiled the device-free worker and separate SDL presentation helper;
actual worker PE imports contained only `kernel32.dll`. Its overall result
also failed because generated tracked files differed in line endings.
Batch 001c passed language checks and canonical regeneration: 435 inventoried
outputs were identical across two runs, with no stale tracked generated files.
Its 816 literal relative imports resolved or matched three deliberate Web-only
branches in native bundles; computed imports and live behavior are not covered.

The native recipe now uses a root aggregate target so Visual Studio builds the
excluded helper subproject. SDL remains outside the device-free actor.
Browser/Android/iOS generators explicitly write LF text on Windows.
`input-equivalence.json` lists all snapshot changes: earlier passing Rust/unit/
audio and native compilation scopes retain unchanged relevant inputs. Failed
batches remain failed; individual applicable passes are carried explicitly.

Subsequent checkpoint changes contain evidence and documentation only. This
qualifies source integration, not full Chromium/core/device/platform behavior
or production release. Those requirements retain their own outstanding numbers.
