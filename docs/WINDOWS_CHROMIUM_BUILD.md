# Sequential task 3 — Windows Chromium build

Status: **in progress; no successful full Chromium build is established**.
This is task 3 in `OUTSTANDING_WORK.md`, not the historical Windows-runtime
product area. The existing WebView2 demo does not satisfy this task.

## Fixed inputs

- Chromium `0e66b94549a8bd85f03fbdf189b752d6e7b63c32`, version
  `155.0.8059.85`, from `browser/upstream.lock.json`.
- depot_tools `4be4e4628e093c4ad50210db5dcfa05a88280bda`, separate Windows
  checkout with automatic source updates disabled.
- Development Windows x64 GN configuration, ordinary full `chrome` target,
  component/debug build with eight compilation jobs and no remote execution.
- Canonical input Git tree `14f1b8d0e7c68aed365cf4d5c05ee3ed7c34f935`;
  preparation receipt SHA-256
  `7960511a6c9ac5ccfb7bb59bdfc13d079a2c19627d2ebb81931c62963912a9da`.
- Frozen overlay retains the generated Game Hub patch and owned-game capture
  patch. Development mode has no approved production core distribution.

The owner explicitly approved Microsoft prerequisites and their terms. Build
Tools 2026 `18.11.12315.225` is installed alongside existing versions, with MSVC
`14.51.36231`; installer discovery reports complete and no reboot required.
The separate Windows SDK 28000.2270/debugger installation completed with exit
`0x0`, restarting `No`. The SDK28000 library directory exists and x64 dbghelp
reports `10.0.28000.2270`. Installer bootstrapper signatures were Microsoft-valid.
No reboot, browser installation or browser launch is part of this build run.

## Execution and retained failures

Antigravity executes assigned tests only; Codex authors source and reviews raw
results. Initial build preparation also ran through Antigravity; compilation
is now supervised directly by a persistent Codex exec session, as described
below. Bootstrap attempts `003-bootstrap` and `003-bootstrap-b` failed
before tool startup because of CMD invocation. Corrected `003-bootstrap-c`
completed the official bootstrap/gclient invocation with exit zero. It does
not establish Chromium configuration or compilation.

`003-build-a` failed its precheck before the build recipe was invoked: the
authored-source parser traversed generated `out/` and treated a Chromium schema
comment preamble as product JSON. The checker now excludes generated output
and Git metadata; product JSON remains strict. This is a harness scope fix,
not a product security or schema relaxation.

Batch `003-build-b` passed authored-source language checks and entered the
recipe's `dependency_sync` phase. Its CLI returned an active-background report,
but subsequent OS inspection found the runner, recipe, gclient and scoped Git
processes absent, with no terminal result/receipt. The interruption record and
original logs are retained. A live-status message or journal alone is not proof
of a surviving process and is not accepted as completion.

Codex resumed the unchanged recipe in batch `003-build-c`, supervised by an
actual persistent exec handle. It failed dependency sync on an orphaned Git
config lock. After confirming no scoped processes remained, the 14 stale locks
in the owned workspace were inventoried, copied into failure evidence and
removed individually with path/time checks. No authoring checkout, source,
dependency pin or user-global Git setting changed. Batch `003-build-d` resumes
the same workspace; its actual terminal build result is still required.

Batch `003-build-d` completed sync/hooks but failed GN because the interrupted
NASM checkout lacked `nasm_assemble.gni`; the Dawn hook also reported an invalid
HEAD. Antigravity's read-only `003-dependencies-a` audit checked 155 Git entries
and found 12 incomplete checkouts. The pinned depot_tools update logic can skip
checkout when a parent's gitlink already matches the pin, despite absent HEAD.
Codex used the ordinary `gclient sync --reset --nohooks --no-history` recovery
inside this owned build workspace. It completed with zero exit; upstream tooling
preserved the 12 broken directories under `_bad_scm` and re-created them.

Antigravity's independent inventory and new production admission check then
passed all 155 dependencies with unchanged check inputs (`003-dependencies-b`).
The authored recipe now admits actual dependency HEAD and tracked source before
hooks, freezes that checker and binds its inventory digest into the final build
receipt. This new recipe is for subsequent preparations; the active build still
uses its original frozen recipe with the separately executed inventory evidence.

Exact upstream hooks reran successfully in `003-hooks-b`. Recovery also reset
Chromium's tracked overlay, causing `003-build-e` to refuse changed integration
bytes before GN. Codex preserved its result/journal and rewound only overlay
admission to the completed-hooks phase. Batch `003-build-f` rechecks each target
against frozen original/new bytes, reapplies the original overlay and continues
GN/compilation. No check was waived, and no product source or pin was substituted.

Batch `003-build-f` completed GN configuration and entered the actual full
`chrome` compilation phase. Its supervisor reached the eight-hour subprocess
limit on 2026-10-23 at 14:56 UTC and returned `ETIMEDOUT`; the batch result remains
failed. Windows retained the scoped autoninja/Siso/compiler descendants, whose
process identity and advancing build log were independently observed after the
recipe parent exited. The existing journal remains `configured`, running
`chromium_compilation`; this cannot establish a surviving recipe or completion.
Codex continues read-only supervision of the exact surviving compiler process.
No duplicate build, source/configuration change or live-lock removal is allowed.
After those processes actually finish, their logs and descendants must be
reconciled before a fresh invocation of the same frozen recipe can confirm the
full incremental target and produce its receipt. Retain the timeout result even
if that later invocation succeeds. This remains progress, not a successful build.
Dependency check reports and raw-log digests are published separately under
`../qa/evidence/sequential/item-003/`; all earlier failed attempts stay retained.

## Accepted Windows x64 development build

The surviving compiler completed on 2026-10-23 at 16:19 UTC with
`Build Succeeded: 57301 steps` after 9h22m42.18s. Codex then verified every
scoped parent/descendant absent, retained the stale lock/journal evidence and
removed only the exact stale build lock. Batch `003-build-g` invoked the same
frozen recipe without resetting source, configuration or the journal. The full
`chrome` target reported `ninja: no work to do` and `Build Succeeded: 0 steps`;
the recipe exited zero and created a `built` journal and final build receipt.

Antigravity executed the independent acceptance checker once (turn `0011`,
exit zero). Codex reviewed the raw execution event, receipt, journal and output
inventory. The checker confirmed unchanged 138 prepared inputs, AMD64 PE32+
entry point and hashes/sizes of seven selected browser outputs. Task 3 is
accepted for this development build. The original timeout batch remains failed.

- Frozen product Git tree: `14f1b8d0e7c68aed365cf4d5c05ee3ed7c34f935`.
- Chromium: `155.0.8059.85`, revision
  `0e66b94549a8bd85f03fbdf189b752d6e7b63c32`.
- Preparation SHA-256:
  `7960511a6c9ac5ccfb7bb59bdfc13d079a2c19627d2ebb81931c62963912a9da`.
- `chrome.exe`: 3,910,656 bytes, SHA-256
  `ef891f1d2a673fbe21e0156d95e95c0ca62d3f5efd04be831bdbde4bc686b907`.
- Local output: `E:\V10\cw3b\out\w\workspace\src\out\OmniPlay`.
- [Published receipts, checker, selected output hashes and raw-log digests](../qa/evidence/sequential/item-003/build/qualification.json).

This component debug build is not a signed installer or a production release.
The receipt retains `browser: not_started`, `verification: not_run`,
`reproducibility_verified: false` and `distribution_approved: false`. The
separate acceptance report establishes build integrity only. Runtime Game Hub,
profiles, permissions and native bridge remain task 6; Linux/macOS builds and
clean-clone reproducibility remain open. This build qualifies the frozen tree
above, not later source changes from parallel work.

Original failed snapshots/logs remain retained in the assigned local QA area;
do not reinterpret them as successful build attempts or count prechecks as
compiled Chromium. Update this record after inspecting terminal build evidence.

## Build prerequisites and completion

Requirements were read from the exact pinned source's
[`docs/windows_build_instructions.md`](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/docs/windows_build_instructions.md),
not inferred from earlier VS2022 fixture builds. They require VS2026, C++ and
MFC/ATL, SDK28000.2270 and appropriate debugging tools. The host has about 32 GB
RAM, 16 logical processors and initially 148 GB free on the build drive; actual
remaining capacity must be monitored during dependency and object generation.

The recipe uses a separate short-path owned workspace, official bootstrapped
vpython, process-local LF/long-path Git configuration and `--no-history` sync.
Omitting unrelated repository history does not omit product source or targets.
See `DESKTOP_BROWSER_BUILD.md` for journal/receipt and ownership rules.

Task completion requires successful hooks, overlay admission, GN configuration,
the full `chrome` target and an actual entry-point hash bound to the frozen
receipt/dependency/tool identities. Real-browser profile/native bridge tests,
release signing, production core admission and broad platform/device acceptance
retain their separately numbered tasks.
