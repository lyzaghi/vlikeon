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

Antigravity executes exact fixed commands only; Codex authors source and reviews
raw results. Bootstrap attempts `003-bootstrap` and `003-bootstrap-b` failed
before tool startup because of CMD invocation. Corrected `003-bootstrap-c`
completed the official bootstrap/gclient invocation with exit zero. It does
not establish Chromium configuration or compilation.

`003-build-a` failed its precheck before the build recipe was invoked: the
authored-source parser traversed generated `out/` and treated a Chromium schema
comment preamble as product JSON. The checker now excludes generated output
and Git metadata; product JSON remains strict. This is a harness scope fix,
not a product security or schema relaxation.

Fresh batch `003-build-b` passed authored-source language checks and entered
the recipe's `dependency_sync` phase. No build receipt has been accepted.
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
