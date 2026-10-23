# Item 52 — partial qualification, still open

`qualification.json` records 28 passing native cases from `052a` and 16
passing Web cases from `052e`. Codex reviewed actual command exits, exercised
assertions and current source hashes; Antigravity executed the tests only.
The requirement matrix and limitations are in
[`docs/STORAGE_FAULT_QUALIFICATION.md`](../../../../docs/STORAGE_FAULT_QUALIFICATION.md).

`052b` exposed a cancellation-classification defect. `052c` confirmed the
product fix and exposed a quota-fixture timing issue. Both failed results are
retained. `052d` passed the earlier Web matrix; `052e` additionally crashes an
actual save-publication transaction, before or after commit. Only `052a` and
`052e` are used for current accepted coverage, without double-counting runs.
Native request/result logs and phase markers are synthetic fixture evidence.

Physical NTFS disk-full execution has **not occurred**. The owner authorized
UAC, but Windows PowerShell blocked `.ps1` execution before the helper created
its workspace. A separate process-only `RemoteSigned` permission request is
pending. No VHDX exists and no execution policy has changed. The prepared
physical test and volume helper are not passing evidence.

Frozen manifests identify actual execution inputs. `runtime_inputs` in the
qualification records the current relevant native and Web inputs compared
against those manifests. Test runners retain machine-specific toolchain and
workspace paths; reproduction requires new immutable source/output paths and
equivalent toolchains. Large runtime trees and browser profiles remain local.
No user games, personal profiles or credentials are included here.
