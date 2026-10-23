# Task 4 — Linux x64 Chromium build, awaiting build host

Task 4 requires the full Vlikeon `chrome` target on Linux x64. It remains open.
The Windows executable, a source archive, a Windows-hosted fixture and source
preparation cannot satisfy this requirement. No Linux dependency hook, compiler
or test has run for this task.

## Observed host constraint

On 2026-10-23, the current Windows host's `wsl.exe --list --verbose` reported
that Windows Subsystem for Linux is not installed. No Docker executable was
found on PATH. Filesystem inspection reported approximately 63 GB free on C
and 35 GB free on E; these fluctuate during other authorized work. No Linux
host or SSH destination has been supplied for this assignment.

The [Linux build instructions at the exact Chromium revision](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/docs/linux/build_instructions.md)
were read from the hydrated pinned checkout. They require an x86-64 host,
at least 8 GB RAM (more than 16 GB strongly recommended), at least 100 GB free
disk space, Git and Python 3.9+; upstream infrastructure uses Python 3.11 and
Ubuntu 22.04. The selected recipe uses Chromium's supported Clang/libc++ tools.
The current disks cannot provide the stated minimum free space for another
independent build. Existing accepted Windows outputs remain preserved.

The owner has been asked for an existing Linux host and SSH username/address,
or additional local storage for a separately authorized WSL setup. No password,
new paid runner, VM installation, OS-feature change or restart is assumed.
Parallel source work's item-53/54/55 deferrals are preserved and not counted as
completion of task 4.

## Frozen transferable preparation

A separate export of public commit
`e527bf2a01df2a42d5cbd6bca91072719420329a` was used; dirty source from the
parallel chat was not copied or modified. Its product Git tree is
`c90b68c0cb3cf9bff4462473a3c8be5969bb54b0`. Within this export only,
`generate_browser_patch.py --without-cores` authored the overlay, followed by
`prepare_desktop_browser.py --target linux-x86_64 --development`.

- Local preparation:
  `E:\V10\work\vlikeon-sequential-01a16635\source-004a\out\linux-004a`.
- Frozen input count: 141 files, 1,329,785 bytes.
- Preparation receipt SHA-256:
  `a3a16d38cf0ee69a9171624d804c81dff1145bcd82a6e66407c9a3e639e540c3`.
- Transfer archive: `E:\V10\work\vlikeon-sequential-01a16635\linux-004a.zip`.
- Transfer archive SHA-256:
  `a6ed3df3723e4c22bf0777c59aed632a400356f2504a5d16664990d301de3c3a`.
- Chromium revision: `0e66b94549a8bd85f03fbdf189b752d6e7b63c32`.
- Intended clean depot_tools revision:
  `4be4e4628e093c4ad50210db5dcfa05a88280bda`.

The source-only receipt retains configuration/compiler `not_started`, browser
`not_started`, verification `not_run` and distribution approval false. This
is a component debug development build configuration with an empty development
core catalog, not a production core package. The frozen recipe includes the
actual Git dependency admission check introduced after the Windows preparation.

## Execution after a host is available

First record host OS/architecture, physical memory, free bytes on the selected
ordinary build filesystem, Git/Python identity and system dependency state.
Provision the prerequisites from the pinned upstream instructions on that host;
the recipe does not install OS packages. Do not transfer the Windows Chromium
checkout or reuse its generated objects as Linux build inputs.

Transfer and verify the archive before extraction to a fresh owned directory.
Bootstrap official depot_tools at the explicit clean revision, with
`DEPOT_TOOLS_UPDATE=0`; its normal `vpython3` wrapper sets up the platform CIPD
binary without a bypass. Preserve its bootstrap logs and tool hashes.

On the qualified Linux host, invoke the frozen recipe under
`linux-004a/source/recipe/tools/build_desktop_browser.py`, passing the absolute
`linux-004a` project, explicit Git and bootstrapped `.cipd_bin/vpython3` paths,
the clean depot_tools directory/revision, a memory-appropriate bounded job
count and `--final-verification`. The recipe hydrates its own pinned workspace,
admits actual Git dependencies, runs upstream hooks, re-admits the overlay,
generates GN and builds the complete `chrome` target.

Use a persistent supervisor that keeps actual compiler exit observable. A
supervisor timeout is a failed batch even if descendants survive; inspect them
before resuming. Do not delete a live lock, reset the journal or change the
source/configuration to turn an interrupted build into a passing result.

Completion requires actual full-target exit zero, a `built` journal and final
receipt binding the prepared source, upstream, tool identities, dependencies
and executable. Antigravity executes the independently assigned Linux output
checks; Codex reviews raw execution, ELF x86-64 identity, artifact hashes and
unchanged frozen inputs. Runtime browser/native bridge, release signing,
production cores and clean-clone reproducibility retain their separate tasks.

The authored checker is `qa/linux/accept-chromium-build.py`. It requires the
actual Linux x64 host, a successful item-4 batch bound to the preparation,
completed journal/receipt, dependency/GN digest bindings, unchanged prepared
inputs, ELF64 x86-64 executable header/mode and selected nonempty output hashes.
Its exclusive output creation preserves earlier reports. The checker has not
been executed or qualified; Antigravity must run it against the actual successful
Linux build before its result can establish task-4 acceptance.
