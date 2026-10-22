# Fresh Windows AppContainer filesystem storage

Source only. No profile, permission, process/token, compiler, fixture or test
has executed during this change. Item 2 and item 16 remain open.

## Closed default filesystem write surface

`CreateAppContainerProfile` creates a new OS-owned storage namespace in addition
to OmniPlay's private work directory. The native actor must not use that area
as an unaccounted second scratch/save root. The new launch source seals its
default filesystem package tree while the actor remains suspended and job-owned,
before publishing process identity or resuming it. Container construction still
establishes only fresh ownership and grants; no untrusted actor runs then.

The broker converts only the SID returned by successful fresh profile creation
to a string and obtains its local folder through `GetAppContainerFolderPath`.
It admits only an ordinary local-drive `Packages/<exact-session-moniker>/AC`
layout, rejecting network/unknown layouts, parent traversal, reparse/symlink
ancestors and any root not bound to that exact new moniker. It never guesses a
path from environment variables or edits the user's AppData/Packages ancestor.
Existing profiles are still never adopted. Unsupported layout fails native
preparation; there is no retry which drops the sealing policy.

Before changing security, a bounded preflight inventories and opens ordinary
objects without following reparse entries. At most 512 objects and depth 16
are admitted. Directory handles deny deletion/rename during the short sealing
pass. The complete preflight precedes security updates; child labels are
installed before the profile package root, and partial failure prevents launch.
Untrusted game code has not started at this point.

The source sets medium-integrity no-write-up mandatory labels, including
inheritance on directories, through the opened file handles. It preserves
discretionary permissions and ordinary audit policy. That filesystem area may
remain readable; the low-integrity actor's writes belong only in separately
granted low-integrity work subtrees. This does not apply a disk quota to those
subtrees or establish a physical byte measurement.

## Actual actor-token admission

After creation-time job membership and before process identity publication or
resume, the launch code queries the suspended actor's real token. It requires
AppContainer status, the exact newly-created SID, the exact low mandatory label
and the token's no-write-up mandatory policy. Requested startup attributes alone
are insufficient. Unexpected values or failed queries reject launch; the owned
job/process guard retains termination responsibility. A single-use exact-token
lookup then materializes/seals the private registry namespace before filesystem
sealing, as described in `WINDOWS_REGISTRY_STORAGE.md`. Both direct and brokered
presentation modes use this same pre-resume sequence.

The security design follows Microsoft's
[mandatory integrity control](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control)
and [AppContainer launch model](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer).
The returned folder has the ownership/lifetime contract documented by
[GetAppContainerFolderPath](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getappcontainerfolderpath).
Actual target behavior remains unproven until final qualification.

## Remaining boundaries and evidence

Private AppContainer registry sealing is now authored separately and remains
unqualified; broader externally granted registry access also needs evidence.
All allowed private scratch/save writes, trusted
helper/driver caches, physical whole-profile accounting and true storage quota
remain separate work. Source sealing is not proof that all Windows-managed
data is bounded. Current disk-copy reservations still describe reserved input
copies, not an OS-enforced writable filesystem cap.

The authored actual-actor fixture and explicitly ignored Rust harness are
described in `qa/windows/STORAGE_PROBE.md`. They require real file/directory,
security-change denial and successful private scratch writes, then confirmed
exit/retirement. The acceptance matrix additionally requires exact supported OS
layouts, inherited/explicit ACLs and labels, streams/replace/delete cases,
partial-update failures, same-user external mutation observations, crash and
profile deletion, actual core/helper/driver behavior and recorded artifacts.
No compile/run/verification is inferred from preparation receipts.
