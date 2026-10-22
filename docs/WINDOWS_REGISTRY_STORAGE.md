# Windows AppContainer private registry storage

Source authoring only. No registry API, token/impersonation, profile/process,
compiler, probe or test has executed. This does not close item 2 or item 16.

## Exact namespace and single-use lookup

The suspended, creation-time job-owned actor is first checked for the exact
fresh AppContainer SID, low integrity and no-write-up token policy. The registry
owner opens that actual process token with query/duplicate rights and copies
the expected SID into owned aligned memory. No renderer path, SID string,
registry hive name or stored PID can select a registry write target.

A new dedicated thread impersonates that token for one readonly
`GetAppContainerRegistryLocation` lookup. Its effective thread token must pass
the same exact SID/integrity/policy checks. Lookup failure never falls through
to the broker identity. The thread calls `RevertToSelf` before returning a key
and is joined before any registry security mutation. The broker/preparation
thread and reusable thread pools are never impersonated.

A failed revert returns no key, closes local handles and ends that single-use
thread. A panic also ends it before the joining caller proceeds. This design
uses the documented thread-lifetime scope of impersonation; it does not continue
serving work on a thread whose security context is uncertain. Actual injected
failure/termination behavior still needs real OS evidence.

The returned key must be a real positive opened handle, not a predefined hive
pseudo-handle. Its bounded valid DACL must contain an effective ordinary allow
ACE naming the exact new SID with query/enumeration rights. An inherit-only ACE
or another SID is insufficient; an inherited ACE applying to this key is valid.
This additional scope check occurs before requesting broker security-write
access. The broker reopens that exact key, never a guessed HKCU path, for
readonly inventory plus READ_CONTROL/WRITE_OWNER.

## Bounded preflight and mandatory labels

Before mutation, the owner inventories at most 256 keys and depth 16, opens
children with `REG_OPTION_OPEN_LINK` and rejects a REG_LINK SymbolicLinkValue.
Unexpected names, permissions, oversized entries or enumeration errors reject
launch. Kernel key handles have unique RAII ownership and no handle is passed
to the actor. Complete preflight precedes medium/no-write-up mandatory labels,
installed on descendants before the returned private root with subkey
inheritance. Discretionary rights and unrelated audit policy are preserved.

After registry materialization/sealing, the exact profile's filesystem backing
tree is sealed through `WINDOWS_DEFAULT_STORAGE.md`. Both operations occur
inside the still-suspended actor launch interval, before its process identity
record is published or its first instruction resumes. Failure retains the
existing owned job/process termination responsibility; no weaker launch retry
exists. Direct and trusted-helper presentation modes share this actor path.

## Evidence and limits

The authored storage-probe-v2 performs real private subkey creation and a
KEY_SET_VALUE acquisition in the actor context, requiring ACCESS_DENIED.
Readonly private-root lookup must work so a missing/broken API cannot count as
successful write isolation. The create attempt uses RegCreateKeyEx's actual
descriptor check rather than treating a readonly parent handle as proof of
denial. The Rust harness also requires the original caller thread to have no
impersonation token before and after launch. See `qa/windows/STORAGE_PROBE.md`.

Required final evidence includes fresh supported OS layouts/DACLs, two profiles
with untouched sentinel data, exact token/lookup/revert failures, link keys,
deep/large keys, inherited and explicit ACE variants, partial labeling, original
caller identity, callback/resume ordering and real core/helper/driver behavior.
Source receipts are not evidence that any of these operations succeed.

This source closes writes to the selected private registry namespace; it is
not a general registry syscall filter, proof of every externally granted key,
a hard disk quota or physical accounting. Complete Windows access qualification,
scratch/save resource controls, helper/driver caches, ambiguous legacy recovery
and all original product/release requirements remain due.

Primary contracts:

- [GetAppContainerRegistryLocation](https://learn.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getappcontainerregistrylocation)
- [ImpersonateLoggedOnUser](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-impersonateloggedonuser)
- [RevertToSelf](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-reverttoself)
- [RegOpenKeyExW and descriptor-versus-handle checks](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regopenkeyexw)
- [GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)
