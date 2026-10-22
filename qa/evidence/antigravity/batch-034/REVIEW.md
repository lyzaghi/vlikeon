# Device-free actor: actual contained media exchange

Codex reviewed the raw build, import and Rust logs against source
04cbd749592971cab78b053d542bfbb12010218c. Antigravity executed the fixed batch;
it made no source or dependency edits. Six configure/build steps, import checks
and one actual integration test all exited zero. Source/dependency inventories
remained unchanged. The eight runner steps are not eight product test cases.

The production Windows launch_presented path creates a fresh AppContainer,
actor/helper jobs and private pipes. The qualification core checks actual
DisallowWin32kSystemCalls and keyboard input. The headless peer checks every
RGBA pixel and audio payload. The Rust case checks active play, pause, memory
save, stop, both real exit codes zero, ended control-reader and profile retirement.
The actor's static import inventory is kernel32.dll only. The same verifier
rejects the earlier desktop worker and three truncated PE inputs.

This resolves the specific initial SDL/USER32 loader dependency in the tested
software actor. It does not qualify the production SDL presentation host,
third-party core dependencies/runtime DLL loading, hardware devices, full storage
accounting, other original routes or item2 as a whole. Static imports alone are
not a sandbox proof; the integration case is separate evidence.

Prior batch033 reached media but failed because its peer interpreted wire RGBA
as an ARGB integer. Source04cbd74 checks all exact RGBA bytes instead. No actor
mitigation or media expectation was relaxed. Batch032 was a stale storage runner
configuration (OLE32 probe and missing loader input), not this media test and
not evidence overturning the separate passing batch028 storage observations.

The runner and prompt are provenance records with local QA paths. They require
the exact source snapshot manifest and pinned SDK prepared by Codex; they are not
a standalone downloadable binary release. No compiler output binaries are kept
in this public evidence directory.
