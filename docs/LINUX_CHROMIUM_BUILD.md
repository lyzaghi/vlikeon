# Task 4 — Linux x64 Chromium build, provisioning in progress

Task 4 requires the full Vlikeon `chrome` target on Linux x64. It remains open.
The Windows executable, a source archive, a Windows-hosted fixture and source
preparation cannot satisfy this requirement. A remote Linux build host is now
available and dependency provisioning has started. No Linux full-target success
or acceptance test is recorded yet.

## Observed host constraint

On 2026-10-23, the current Windows host's `wsl.exe --list --verbose` reported
that Windows Subsystem for Linux is not installed. No Docker executable was
found on PATH. Filesystem inspection reported approximately 63 GB free on C
and 35 GB free on E; these fluctuate during other authorized work. This was the
initial constraint before the owner supplied a remote host on 2026-10-24.

The [Linux build instructions at the exact Chromium revision](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/docs/linux/build_instructions.md)
were read from the hydrated pinned checkout. They require an x86-64 host,
at least 8 GB RAM (more than 16 GB strongly recommended), at least 100 GB free
disk space, Git and Python 3.9+; upstream infrastructure uses Python 3.11 and
Ubuntu 22.04. The selected recipe uses Chromium's supported Clang/libc++ tools.
The current disks cannot provide the stated minimum free space for another
independent build. Existing accepted Windows outputs remain preserved.

The supplied Ubuntu 22.04.5 x86-64 host initially had about 1 GB RAM and a
10 GB disk. After the owner requested a recheck, read-only SSH diagnostics
confirmed its configuration changed to 4,101,849,088 bytes physical RAM,
30,064,766,976 bytes swap and 175,720,919,040 bytes available disk on a 200 GiB
volume, with two logical CPUs. Storage now exceeds the pinned minimum; physical
RAM still falls below upstream's stated 8 GB minimum. Codex is attempting one
compiler job using the existing swap, recording this resource deviation rather
than claiming the host meets that minimum. Compiler/linker success remains
unproven. No swap, VM, OS-feature or restart change was made by this task.

The frozen archive transferred to a new task-owned remote workspace and passed
its SHA-256 check. Batch `004-build-a` bootstrapped the clean pinned depot but
failed its vpython invocation because the process PATH did not include the
depot wrapper directory; its result/logs are retained. Codex authored a fresh
supervisor for `004-build-b`, which admits the unchanged prepared inputs and
clean depot revision before resuming with the corrected process-local PATH.
Official vpython bootstrap passed and the exact pinned
`build/install-build-deps.py` is provisioning Ubuntu packages. The script's
SHA-256 is `a71e5ed8e3cbcaf87b255ff124c424cda73b48bcb3fd6a4d44bed2de42c68f19`.

The supervisor persists each stage's stdout/stderr, process-group identity and
terminal result independently of SSH, with a bounded 48-hour full-target stage
and one job. Bootstrap/provisioning is not Chromium compilation or acceptance.
The supplied host address and access credentials are not part of public evidence.
Parallel source work's item-53/54/55 deferrals remain separate assignments.

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
