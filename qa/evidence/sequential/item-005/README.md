# Task 5 — accepted macOS ARM64 development Chromium build

Batch 005-build-g completed the full chrome target and final verification with
exit 0 at 2026-10-30T09:06:00Z (60845.03 seconds, two jobs). The frozen project
macos-005d resumed preserved partial objects after batch F's 72-hour timeout;
G used a 168-hour bound. Earlier failures, logs and prior lock evidence remain
preserved. No frozen source or GN workaround was used to obtain this pass.

Codex native acceptance and Antigravity's independent same-chat turn 0050 both
returned exit 0 and passed: true. Codex reviewed the raw completed run_command
output and independently retrieved the report, journal and tool hashes.
The journal is built with running/failed null. Receipt SHA-256 is
cd21d3780cab4e1233831f1a3eaa0ebd08edfd23bcb4c1bcabae48684f39d116;
batch result SHA-256 is
1c948b0e2300333fa4d4cfa174f9ed5c34ff2129ab97b630e7aa70a3d52cec3d.

The acceptance report records thin Mach-O ARM64 launcher/framework headers,
bundle identifier app.omniplay.browser, unchanged frozen inputs, dependency/GN
digest bindings and six selected output hashes. This component development
build has small launcher/framework entry points; the report does not inventory
or qualify all runtime-loaded component libraries.

- Preparation: 0c3955502c40f189853c84e994e572b5bbe98ee723888511fbe520a7762e31ed.
- Product source tree: a06d2d607fead2d412a5bbab29a4d5977dc7684e.
- Chromium: 0e66b94549a8bd85f03fbdf189b752d6e7b63c32.
- depot_tools: 4be4e4628e093c4ad50210db5dcfa05a88280bda.
- Independently hashed Git: fa7249764de6d26fefb1015793bb99d769f527ffe514d53f89c33f96add7d8f3.
- Independently hashed vpython: b8342ea17142a1f55276aa27bf6198bcb66f0a740b4bda877c2bb8372e56f9f7.

The Apple Silicon host used external APFS, Xcode 26.3 / SDK 26.2 and the
official Metal toolchain. Xcode 16.4 / SDK 15.5 failed the required module-map
checks in an earlier batch; the later supported installation replaced that
prerequisite for this build.

Acceptance covers the full development build and selected app/framework
integrity only. Browser/native bridge runtime, production cores, signing,
notarization, distribution and clean-clone reproducibility remain unaccepted.

Report: [acceptance-ag-g.json](acceptance-ag-g.json). Preparation/storage history remains in the adjacent summary files.
