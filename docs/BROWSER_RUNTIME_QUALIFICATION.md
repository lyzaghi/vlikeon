# Task 6 — browser runtime qualification

Windows development acceptance passed on 2026-10-24: **24 checks across four
actual browser launches**, with no fatal exception or Network Service crash in
those phase logs. The full cross-platform item remains open: Linux and macOS
runtime acceptance must follow their successful builds. See the scoped receipt
in `../qa/evidence/sequential/item-006/diagnosis-summary.json`.

The tested runtime is the frozen Windows x64 Chromium 155.0.8059.85 development
build, not the WebView2 demo or later parallel source changes. Its executable,
chrome.dll, content.dll and real broker hashes are admitted before testing.
The unsuccessful startup/COM candidates were reverted, including their source
and DLL/PDB/library outputs. No startup workaround was promoted.

| Area | Evidence from actual Windows runtime |
| --- | --- |
| Game Hub | Fresh startup and restart load the bundled component document and runtime ID. |
| Tabs | Actual bridge creates, lists, activates and pins tabs; target closure is observed asynchronously; session tabs restore after restart. |
| Normal profiles | Two normal profiles have distinct broker profile identities and separate web/component storage. Data persists across their independent restarts. |
| Private contexts | Real Chromium off-the-record contexts have isolated cookies/storage, do not appear in the Hub's tab list, cannot acquire component native privileges, and lose their data after disposal. |
| Permissions | Declared component permissions are available; undeclared debugger permission and privileged URL schemes are rejected. Actual Site Settings admit, block and reset notifications for an owned loopback origin. The grant persists after restart, can be revoked, and does not leak to another profile. |
| Native bridge | A real PlatformBridge hello reaches the registered broker. Missing-host errors are observed. Terminating the exact QA broker after verifying its executable and browser ancestry produces a disconnect event; a subsequent request launches a new broker and succeeds. |

Site permission checks use the browser's existing Site Settings handler in an
actual `chrome://settings/content` page. They do not use CDP permission overrides.
No notification is shown and no OS device consent is inferred. An ordinary web
page has no native messaging API. All profiles, data and permission changes
belong to the newly created QA directory and loopback origin.

## Execution-context diagnosis

Antigravity's command runner reports an isolated `exebox-*` Windows desktop;
Codex's normal supported runner reports `WinSta0/Default`. With the same restored
binaries, Antigravity's launch still reproduces the blank Hub/network failure,
while direct normal-desktop runs pass. Process job UI restrictions and Win32k
policy queries both returned zero in the two collector contexts.

Process-local debugger checkpoints locate the failing Windows path:
`NtGdiInit2` returns zero, GdiDllInitialize returns STATUS_UNSUCCESSFUL, and
USER32 fails DLL_PROCESS_ATTACH. This produces DLL_INIT_FAILED (1114), including
when USER32 is loaded as an OLE32 dependency. The exact desktop ACL or kernel
policy responsible has not been proven. Error 1114 alone does not establish a
Chromium sandbox-policy defect or a missing DLL.

Following the owner's instruction to continue independently when Antigravity
fails, Codex executed the successful GUI suite through its normal supported
runner. Antigravity separately audits the report, raw phase logs and diagnostic
consistency. That artifact audit is not an independent Antigravity GUI pass.
The final Antigravity artifact audit exited 0 and confirmed the 24-check report
and four clean phase logs; its scoped receipt is
`../qa/evidence/sequential/item-006/artifact-audit.json`.
Its isolated desktop, ACLs and execution restrictions remain unchanged. No
sandbox disable, security mitigation change, job breakaway or desktop rerouting
was used.

## Repeating Windows acceptance

Run `qa/browser/runtime-lifecycle.mjs` with Node.js 22+ on an authorized Windows
interactive test runner. Supply four absolute paths: the frozen browser EXE, a
new report directory, its admitted identity JSON, and the real broker EXE.
The identity JSON contains `windows_executable_sha256`, `chrome_dll_sha256`,
`content_dll_sha256`, and `broker_sha256`. The real development host must already
be registered for the exact bundled component origin.

The suite refuses an existing report directory or pre-existing broker process.
It uses fresh owned profiles, closes its browsers, and terminates only a broker
whose exact executable and ancestry lead to its own browser. Retain reports and
stderr from failed runs. An incompatible isolated GUI runner must not be made
to escape its containment to pass this test.

## Remaining scope

Canonical item 6 remains unchecked until Linux/macOS runtime acceptance is
available. Linux build task 4 is still compiling. Mac build task 5 stopped after
dependency sync because a read-only dependency Git diff exceeded the recipe's
90-second timeout on the external HDD; that failure is not a compiler result.
The old batch logs remain intact and no replacement was started in this task.

This acceptance does not establish a signed release, every native module,
device capture/OS consent dialogs, reproducibility or later unbuilt source.
Raw dumps, process memory, local paths and profile/device identifiers remain
in private QA storage. Public evidence contains only bounded, sanitized facts.
