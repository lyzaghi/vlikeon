# Task 5 — macOS ARM64 host connected, storage provisioning pending

On 2026-10-24 the owner requested preparing task 5 while task 4 continues its
actual Linux build. The owner confirmed an Apple Silicon Mac is available and
asked how much storage to provision. The owner subsequently supplied a Mac
endpoint; Codex connected using Tailscale SSH with coordination-server host-key
verification. No password was entered, stored or published.
Task 5 remains open: no Mac dependency hook, GN invocation, compiler or acceptance
test has executed. Windows preparation and Linux compilation cannot prove it.

## Host and capacity planning

The [instructions at the pinned Chromium revision](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/docs/mac_build_instructions.md)
require a Mac, Xcode/macOS SDK and an APFS build volume. This project's selected
target is native ARM64 on Apple Silicon. Exact OS, CPU, RAM, storage and Xcode
must be read from the actual host before provisioning; user confirmation of a
Mac does not establish those values. The actual connected host reports macOS
15.7 (24G217), native ARM64, 16 GiB RAM and eight logical CPUs. The USB external
disk is writable ExFAT, 4,000,769,376,256 bytes with 3,938,676,899,840 bytes free
at inventory. The system APFS volume has approximately 20 GiB free.

Codex recommended reserving **250 GB free SSD space after macOS and Xcode** for
source, dependency downloads, objects and outputs. For a newly provisioned disk,
**350 GB or more total** leaves room for the OS/tools. These are project planning
budgets, not a claimed official minimum in the upstream Mac instructions.
Actual available capacity must be recorded and monitored during the build.

The owner subsequently selected an external 4 TB HDD for build storage.
`qa/macos/prepare-external-build.sh` is an authored setup helper: it verifies
an external mounted volume and at least 250 GB free, refuses existing target
paths, and creates a new 350 GiB maximum sparse bundle containing an APFS volume.
The sparse bundle and its setup temporary directory remain on the selected HDD;
it does not repartition, convert or erase that disk. This supports an existing
writable exFAT volume without treating exFAT itself as the Chromium filesystem.
The mounted build path `/Volumes/VlikeonBuild/work` has no spaces even if the
external drive's name does. Source, projects, logs and configured temporary/
language-package caches use that workspace. The generated `env.sh` must be
sourced by every actual bootstrap/build supervisor; it does not change global
shell profiles, relocate Xcode or move macOS swap/system writes. Both the APFS
image capacity and physical backing HDD free bytes must be monitored.

Antigravity executed a nonexecuting Bash parser check, exit 0, with no output.
Codex then invoked the helper on the actual Mac. The native diskutil/plutil
external-device, mountpoint and free-space admission completed. The helper is
stopped after waiting in its first mkdir on the external disk; no successful APFS
creation or cache placement has been established. A read-only macOS TCC log
attributes removable-volume access checks to tailscaled and records an AllFiles
policy denial; this does not by itself prove why mkdir is waiting. The owner
has been asked to inspect a possible removable-volume consent prompt. No
privacy setting was changed or access restriction bypassed. Codex terminated
the exact owned setup and diagnostic processes and verified their absence.
Inspect any partial setup paths after consent before a new run; do not overwrite
them or assume the native setup completed. Existing setup paths are never
overwritten. Final native behavior and storage placement still need inspection.

For this exact Chromium pin, `build/config/mac/mac_sdk_overrides.gni` declares
`mac_sdk_min = "15"`; `mac_sdk.gni` records SDK `26.5`/build `25F70` for official
builds. The selected configuration is an unbranded component debug development
build. Inspect actual Xcode/SDK compatibility and any diagnostics rather than
claim that every installed Xcode version is qualified. The owner handles any
required Xcode license acceptance; no agreement was accepted by this task.

`qa/macos/build-host-preflight.sh` is an authored read-only inventory of OS,
architecture, RAM, CPU count, build filesystem/free space, selected Xcode and SDK,
Git and Python. It takes an existing build-parent directory. The helper has not
been executed or qualified on a Mac; equivalent direct read-only inventory
commands established the host values above. Only Command Line Tools are selected
and no full Xcode was found in the checked application locations. The helper
does not install tools or modify settings.

## Frozen source and transfer

A separate source export uses public commit
`4d3003b93bbe3e359f8232ab1af5e54ecc2fe0c6`, Git tree
`a06d2d607fead2d412a5bbab29a4d5977dc7684e`. Dirty source from parallel work
was not copied. `generate_browser_patch.py --without-cores` and
`prepare_desktop_browser.py --target macos-aarch64 --development` ran only in
this fresh export. They author/copy source and metadata, not a native build.

- Local project:
  `E:\V10\work\vlikeon-sequential-01a16635\source-005a\out\macos-005a`.
- Prepared input count: 141 files, 1,362,744 bytes.
- Preparation receipt SHA-256:
  `dd9776d0bb8300894311d53788d3529b646382408ca75fb6abce5ae2b8f9bbc8`.
- Transfer archive: `E:\V10\work\vlikeon-sequential-01a16635\macos-005a.zip`.
- Transfer archive SHA-256:
  `d9b7dda20d8a0963e2050e50df8ded5b2a3586efe5340c489b8e9bd4a8b3144e`.
- Upstream revision: `0e66b94549a8bd85f03fbdf189b752d6e7b63c32`.
- Intended clean depot_tools revision:
  `4be4e4628e093c4ad50210db5dcfa05a88280bda`.

The preparation retains configuration/compiler `not_started`, browser
`not_started`, verification `not_run`, and distribution approval false. The
development core catalog is empty and is not a production core package.

## Execution and acceptance after host qualification

Use a fresh owned APFS directory. Transfer and verify the prepared archive,
bootstrap official tools at the recorded clean revision and keep the depot
wrapper on the process-local PATH. Provisioning must preserve any existing
source, profiles, credentials and unrelated host settings.

Run the exact frozen recipe under `source/recipe/tools/build_desktop_browser.py`
with the Mac project, explicit Git/bootstrapped vpython paths, depot revision,
bounded memory-appropriate jobs and `--final-verification`. Its owned workspace
hydrates the exact source, admits actual dependency checkouts, runs official
hooks, applies the exact prepared overlay, generates GN and builds full `chrome`.
Use a persistent supervisor with individual logs, process identity and actual
terminal result. An observation/SSH timeout is not compiler completion and
cannot justify a duplicate build or live-lock removal.

The authored `qa/macos/accept-chromium-build.py` is for Antigravity to execute
after an actual successful item-5 batch. It has not been executed or qualified.
It requires native Darwin ARM64, a completed journal/receipt bound to the
preparation, unchanged frozen inputs, dependency/GN digests, the app's expected
bundle ID and main Mach-O ARM64 executable, the embedded framework Mach-O ARM64
library and selected resource hashes. Normal framework `Versions/Current`
links are resolved only inside the actual app bundle. The pinned GN source uses
`v8_context_snapshot.arm64.bin` on Mac; the checker preserves that exact name.

The checker writes a fresh report exclusively and does not launch, sign or
notarize the app. Its selected output integrity cannot prove runtime behavior,
the native bridge, reproducibility, code-signing identity, notarization or
production distribution. Those gates remain separate tasks. Task 5 is accepted
only after actual full-target success, final receipt, independently executed
checks and Codex review of the raw results.
