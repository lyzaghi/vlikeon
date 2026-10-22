# Actual worker/core temporary environment and regressions

Codex inspected every case in the three evidence files: 3 temporary environment,
13 lifecycle and 19 wire cases passed. All three harnesses exited zero; artifact
and harness hashes matched batch029 and remained unchanged.

The loaded DLL observes Win32 TEMP/TMP, wide CRT _wgetenv and narrow CRT getenv
against the explicit private scratch identity, starting with different stale
temporary values. ASCII and Unicode paths succeed and the worker exits zero.
A relative private path terminates the actual worker with code2 before core load.
The lifecycle/wire regressions use the rebuilt worker and both rebuilt DLLs.

Scope: direct worker, qualification DLLs and dummy SDL drivers. No AppContainer,
production core file operation, static third-party CRT, device or complete
product acceptance is implied. Actual OS storage evidence is separate batch028;
contained USER32/SDL dependency closure and other item2 obligations remain open.
