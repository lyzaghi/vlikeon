# Item 52 — accepted fault qualification

`qualification.json` records 28 passing native cases from `052a`, 16
passing Web cases from `052e` and four physical NTFS full-disk cases from
`052g`. Codex reviewed actual command exits, exercised
assertions and current source hashes; Antigravity executed the tests only.
The requirement matrix and limitations are in
[`docs/STORAGE_FAULT_QUALIFICATION.md`](../../../../docs/STORAGE_FAULT_QUALIFICATION.md).

`052b` exposed a cancellation-classification defect. `052c` confirmed the
product fix and exposed a quota-fixture timing issue. Both failed results are
retained. `052d` passed the earlier Web matrix; `052e` additionally crashes an
actual save-publication transaction, before or after commit. Only `052a` and
`052e` are used for current accepted coverage, without double-counting runs.
Native request/result logs and phase markers are synthetic fixture evidence.

Physical NTFS disk-full execution **passed** in `052g`: real `write` `ENOSPC`,
failed save without publication, original progress preserved, staging cleaned
on reopen and same-save-identity retry after space release. `052f` stopped
before fixture writes because a disk-image pipeline bound null `Number`;
`052g` verifies exact image `DevicePath` before explicitly resolving its disk.

The owner authorized UAC and process-only `RemoteSigned` for preparation.
The VHDX was created, attached and used. Account/machine policy was not changed.
The filler and reserve are gone. The exact image was detached after Windows
confirmation. `volume/volume.json` records the final receipt;
`volume/volume-052-detached-observation.json` records `Attached=false` and the
absent owned diskpart process. `volume/volume-prepared.json` is the earlier
preparation-only receipt. The image and synthetic profile remain retained.

`qualification.json` accepts item 52 with 48 passing cases and no pending
fixture work. It verifies 184 native inputs, 264 Web inputs and both physical
fixture sources against their executed manifests. This does not claim
power-loss durability or close other OS/device and product-release tasks.

Frozen manifests identify actual execution inputs. `runtime_inputs` in the
qualification records the current relevant native and Web inputs compared
against those manifests. Test runners retain machine-specific toolchain and
workspace paths; reproduction requires new immutable source/output paths and
equivalent toolchains. Large runtime trees and browser profiles remain local.
No user games, personal profiles or credentials are included here.
