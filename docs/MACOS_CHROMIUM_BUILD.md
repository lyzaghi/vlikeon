# Task 5 — external APFS ready, full Xcode pending

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
The first native setup waited in its initial external mkdir. Codex stopped the
exact owned processes and verified their absence. After the owner confirmed
allowing removable-volume access, Codex verified both setup destinations were
absent and ran the helper again. It completed successfully: the APFS image is
mounted and its generated build directories and environment are present. No
privacy setting was changed by Codex and no access restriction was bypassed.

Antigravity's first native storage check exposed that Apple's `/usr/bin/python3`
launcher replaces TMPDIR with an internal system directory. Codex verified that
executing the real interpreter returned by `xcrun --find python3` preserves the
external TMPDIR. The second native check used that interpreter, retained all
storage assertions and emitted `passed: true`. It verified image-to-external-disk
binding and 17 configured temporary/cache variables beneath the build root.
This qualifies the current storage/environment, not every future tool's cache.
Actual supervisors must select the bootstrapped Python interpreter explicitly;
do not assume the Apple launcher preserves the environment.

The old Python statvfs measurement underreported free bytes on the large ExFAT
volume. Codex cross-checked `df -kP` against native diskutil: physical free space
was 3,938,610,839,552 bytes; the new sparse bundle consumed about 64 MiB.
Use native df/diskutil for backing-disk capacity monitoring. The mounted APFS
image's capacity is 375,599,882,240 bytes, not preallocated physical usage.
`qa/evidence/sequential/item-005/storage-summary.json` records the bounded result.

For a fresh Terminal on the Mac:

```bash
source /Volumes/VlikeonBuild/work/env.sh
cd "$VLIKE_BUILD_ROOT"
```

After a restart, mount the existing image first (do not rerun setup):

```bash
bash "/Volumes/New Volume/VlikeonBuildHost/mount-vlikeon.sh"
source /Volumes/VlikeonBuild/work/env.sh
```

Full Xcode is still required. Apple's [compatibility matrix](https://developer.apple.com/xcode/system-requirements)
lists Xcode 26.3 as compatible with macOS 15.6 and later. Obtain the official
`Xcode_26.3.xip` through [Apple Developer Downloads](https://developer.apple.com/download/all/?q=Xcode%2026.3)
using the owner's Apple sign-in, saving it directly to
`/Volumes/VlikeonBuild/work/tools`. No account access or license acceptance has
been performed by this task. Extract on that same external APFS volume:

```bash
source /Volumes/VlikeonBuild/work/env.sh
cd "$VLIKE_BUILD_ROOT/tools"
xip --expand Xcode_26.3.xip
open Xcode.app
```

The owner reviews the first-launch license and required components. After that,
select this Xcode only for the build process, without a global xcode-select edit:

```bash
export DEVELOPER_DIR="$VLIKE_BUILD_ROOT/tools/Xcode.app/Contents/Developer"
xcodebuild -version
xcrun --sdk macosx --show-sdk-version
```

This is a compatible host-side candidate; the actual pinned Chromium GN/build
must still establish SDK compatibility. macOS swap and system-managed writes
remain on the system disk; the external workspace does not relocate them.

For this exact Chromium pin, `build/config/mac/mac_sdk_overrides.gni` declares
`mac_sdk_min = "15"`; `mac_sdk.gni` records SDK `26.5`/build `25F70` for official
builds. The selected configuration is an unbranded component debug development
build. Inspect actual Xcode/SDK compatibility and any diagnostics rather than
claim that every installed Xcode version is qualified. The owner handles any
required Xcode license acceptance; no agreement was accepted by this task.

`qa/macos/build-host-preflight.sh` is an authored read-only inventory of OS,
architecture, RAM, CPU count, build filesystem/free space, selected Xcode and SDK,
Git and Python. It accepts an existing build-parent directory, resolves its
device with df, then asks diskutil about that device. The native Mac rejected
diskutil info on the nested work directory; that admission bug was corrected
without changing any frozen recipe input. Antigravity executed the revised
preflight on the actual external workspace: remote exit 0, APFS reported,
SDK 15.5/build 24F74, Git 2.39.5 and Python 3.9.6. Only Command Line Tools are
selected; xcodebuild reports that full Xcode is required. This is inventory
success with an incomplete Xcode prerequisite, not full build-host acceptance.
The helper does not install tools or modify settings.

## Pinned depot bootstrap

Codex provisioned depot_tools on the external workspace at the intended clean
revision `4be4e4628e093c4ad50210db5dcfa05a88280bda`. A detached supervisor used
the real CLT interpreter, the sourced external environment, per-stage logs,
bounded process-group timeouts and exclusive run directories. No global Python
installation, Xcode installation, license acceptance or frozen source mutation
was performed. The initial batch failed during filesystem admission before
creating a depot checkout; its logs/result were retained. Batch
`005-depot-bootstrap-b` passed git init, exact-revision fetch/checkout and base
vpython bootstrap. The base vpython invocation selected Python 3.8.10 in the
external cache; it is not evidence that the depot's Python 3.11 gclient TOML
environment has been populated or that the Chromium recipe can use that default
runtime. The recipe supervisor must use an explicitly suitable interpreter.

Antigravity then independently checked the terminal batch, actual clean depot
HEAD, selected Git executable hash and all 141 transferred source inputs
(1,362,744 bytes), rejecting unknown files and symlinks. Native JSON reported
`passed: true` and the remote exit sentinel was 0. These checks establish base
tool/bootstrap and transfer integrity only. They do not establish dependency
sync, hooks, GN, compilation, runtime or release approval. See
`qa/evidence/sequential/item-005/tool-bootstrap-summary.json`.

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

The archive was transferred directly into the external
`/Volumes/VlikeonBuild/work/projects` and its SHA-256 matched on the Mac. It was
extracted into a previously absent `macos-005a` directory there; the extracted
preparation receipt also matched the SHA-256 above. Independent source inventory
verification subsequently passed as described above. Admission by the frozen
build recipe, gclient dependency sync, hooks and compilation remain pending.

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
