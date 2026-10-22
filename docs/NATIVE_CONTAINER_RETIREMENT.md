# Per-session Windows container retirement source

Native launches now create a fresh `OmniPlay.Session.<32 hex UUID>` AppContainer
identity for each private work directory. The random identity separates simultaneous
and later sessions, including two launches of the same game. Existing OS profiles
are never adopted after an already-exists result. Legacy `OmniPlay.Native.*`
profiles are not enumerated or deleted by the new path. These are unexecuted source
changes: no profile creation/deletion, process, ACL, compiler or test has run.

The sandbox retains the immutable SID/profile owner through both the adapter and
its owned process guard. A profile tracks its active process count, whether a
termination result is uncertain and whether retirement succeeded. Only the exact
owned worker/job is terminated. Setup failure after process creation is protected
by that guard even before job assignment or thread-handle validation completes.
The launch is single-use, and the worker cannot resume before confirmed
creation-time job association and
the broker's process-record callback succeed.

Preparation records creation intent and, after successful OS creation, a separate
ownership acknowledgement in the broker-owned work root. Launch intent is recorded
before attempting process creation. While the child is still suspended, its actual
PID and creation time are recorded through the fixed callback. These bounded
immutable records bind the browser profile UUID, work UUID and derived container
name. The contained core receives only read access to the work root and cannot
rewrite its ownership records through the declared ACL grants.

Normal cleanup requests termination, checks the owned process exit, closes IPC/file
guards and retires the OS profile before removing its work directory. The SID is
freed exactly once. If termination cannot be established, the identity records an
uncertain state, the work/journal remain and a blocked IPC thread is abandoned
rather than joined indefinitely through that cleanup path. This does not prove
that a hung kernel operation or OS failure can always be cancelled or reclaimed.

Startup recovery inspects at most 64 work entries and first acquires the exact
`owner.lease` exclusively. It validates ordinary non-reparse roots and records,
canonical UUID confinement and the expected profile/work/name relationship.
A recorded process is queried by PID **and creation time**. Recovery never kills
a PID from a file: a live matching process, access failure or uncertain observation
retains the profile/work. An ended process or a later process reusing the PID lets
recovery request retirement of the exact recorded session profile. A failed OS
retirement keeps the journal for retry. Missing profiles may be retired idempotently
through the documented Windows API.

An ownership record without a launch-intent marker can retire a never-launched
session. An interrupted launch with no durable process acknowledgement is retained.
Likewise, creation intent without ownership acknowledgement is retained because an
already-existing or ambiguously created profile must not be guessed to be ours.
These narrow crash windows still need an explicit recovery mechanism/evidence;
they are not silently swept by namespace alone. Old unmarked private work may be
removed under its existing owner-lease contract, but unrelated OS profiles and
ambiguous new intent records are not adopted.

Creation-time `PROC_THREAD_ATTRIBUTE_JOB_LIST` association is now required for
both actor and trusted presentation process. The job has kill-on-last-handle-close
set before `CreateProcessW`, the child never receives its job handle, and actual
membership is checked before journal publication/resume. This removes the
separate create/assign interval in new launch source. Missing durable process
acknowledgements remain conservatively retained during recovery; the stronger
launch primitive does not retrospectively prove an old ambiguous journal safe
to delete. No Windows call or test has been executed.

Work removal first checks the resolved UUID directory stays directly beneath its
private profile root, bounds depth/entries and refuses symlinks, junctions, reparse
points and special entries throughout the tree. Only after the worker/OS-profile
conditions succeed does it remove that ephemeral owned tree. Unknown linked or
oversized trees remain for recovery. These checks do not establish handle-relative
resistance to an external same-user attacker replacing ancestors during cleanup.
Power-loss durability of journal publication, filesystem/ACL aliases and Windows
directory-flush behavior still require implementation/qualification where necessary.

This source reduces session data/identity retention; it does not implement a kernel
disk quota. Fresh default filesystem sealing and actual actor-token admission
are now authored in `WINDOWS_DEFAULT_STORAGE.md`, but remain unqualified.
Private registry sealing is now authored in `WINDOWS_REGISTRY_STORAGE.md`;
broader externally granted registry access remains unqualified. Native scratch/save writes,
unrecognized orphan files, ambiguous journals and OS cleanup failures still need
complete quota/recovery/product controls. Automatic retry visibility, legacy-profile
migration and the full remaining platform/security/release scope remain required.

The final consolidated pass must cover concurrent games, creation collisions,
failure at every setup boundary, callbacks before resume, owner close/crash, PID
reuse, access-denied queries, stalled termination/IPC, busy OS-profile handles,
partial deletion, reparse/large trees and all journal crash/power-loss boundaries.
No containment, deletion effectiveness or release-readiness claim is made.

Contracts: [CreateAppContainerProfile](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-createappcontainerprofile),
[DeleteAppContainerProfile](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-deleteappcontainerprofile).
