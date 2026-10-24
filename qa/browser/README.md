# Actual browser runtime suite

`runtime-lifecycle.mjs` tests a real, hash-admitted Windows Chromium build and
its real registered native broker using fresh owned QA profiles. It exercises
tabs, two persistent normal profiles, off-the-record contexts, actual site
permission settings, component authority boundaries and broker crash recovery.
It requires Node.js 22+, Windows PowerShell/CIM, and an authorized interactive
Windows test runner.

Usage (all four arguments are absolute paths):

```text
node runtime-lifecycle.mjs <browser.exe> <new-report-directory> <identity.json> <broker.exe>
```

Identity JSON must contain SHA-256 values named `windows_executable_sha256`,
`chrome_dll_sha256`, `content_dll_sha256`, and `broker_sha256`. The two DLLs must
reside beside the browser EXE. This verifies the full component runtime pair,
not only the small launcher EXE.

The report directory must not exist. No pre-existing broker may be running.
The suite creates only owned profiles and loopback test data; it changes site
notification permissions only within those profiles. It displays no
notification. It deliberately terminates the owned broker after validating its
exact executable and browser ancestry, then tests recovery. It never disables
the Chromium sandbox, overrides CDP permissions or changes the runner's desktop.

A passing report qualifies its recorded Windows development scope only.
See `../../docs/BROWSER_RUNTIME_QUALIFICATION.md` for accepted evidence and the
observed limitation of Antigravity's isolated Windows desktop runner.
