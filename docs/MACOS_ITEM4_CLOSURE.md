# Item 4 — macOS source completion

The owner directed moving from Linux to macOS and completing source authoring
before practical verification. No compiler, build, codesign, workflow, runtime,
test or Antigravity batch is executed for the changes recorded here.

## Current implementation

- `process.mm` reserves one of four owner slots before pipes or spawn. Failed
  launches and uncertain retirement keep their owners and capacity charged.
- Both direct children are created suspended with closed descriptor inheritance,
  reset signal state and separate process groups. The owner records their
  observed start identities, registers exit observation, repeats signed-bundle
  admission and strictly validates live child code/entitlements before resume.
  This extra admission does not change the existing XPC peer checks.
- Signalling and WNOHANG reaping share the same child lifecycle mutex. A returned
  owned-child wait receipt is required for confirmed exit. Failed observation,
  ECHILD, ESRCH or an uncertain child never authorizes retirement. Cleanup waits
  are finite; uncertain work remains retained rather than refunding its slot.
- Endpoint descriptors have move-only RAII ownership. Per-session stop requires
  the owning connection. Disconnect cleanup uses a fixed four-entry collection;
  connection-owner identities saturate instead of wrapping into reused authority.
- Staging writes internal receipt schema 2 and a private single-link owner lease.
  Admission reopens the exact UUID directory, holds its lease, validates closed
  receipt fields and bounded inventory, checks actual immutable input sizes and
  SHA-256 bytes, and retains descriptor/path identities through launch.
- A durable launch fence is flushed before any child spawn. Only confirmed owned
  child retirement removes that same fence, with directory sync before refund.
  Owner restart therefore does not turn lost in-memory process records into
  deletion authority. A retained fence, missing legacy lease or unknown shape
  refuses staging admission/discard; no disk PID is signalled or adopted.
- Staging cleanup is flat, link-refusing and bounded to the existing asset/byte
  budgets. It does not recursively traverse unknown directories. A held lease or
  launch fence blocks deletion. Partial deletion and unsupported legacy shapes
  remain retained for explicit recovery.

## Source obligations still open

1. Finish the authenticated XPC broker adapter and checked request/save-return
   composition. Low-level process source is deliberately not advertised as an
   operational session_launch capability while this source work is incomplete.
2. Complete descendant lifetime, owner-loss recovery and confirmed resumable
   storage deletion. Direct-child exit alone is not proof that all possible
   subprocesses or resource charges have ended. In-memory ownership does not
   reconstruct missing ownership after an owner crash.
3. Close actual actor/helper writable-resource and memory accounting. App Sandbox
   inheritance and private file modes alone are not per-core filesystem quotas
   or proof that a child cannot alter shared container data.
4. Finish activation/focus, input/audio lifecycle, final saves, error/cancellation
   and profile association through the owner boundary, then integrate signed
   runtime catalogs, install/upgrade/repair and the complete product adapter.
5. Retain later target SDK/compiler, signature/notarization, App Sandbox/XPC,
   resource/fault, core/device and full acceptance requirements. Source changes
   are not evidence that these requirements passed.

Automatic review rejected two proposed edits as potential relaxations: changing the
existing guest-lookup flags and dropping the uncertain-child early refusal.
Neither change was applied. The implementation retains those existing strict
boundaries and adds independent checks and ownership controls around them.

Item 4 is in progress, not 100% complete. The owner retains all original scope;
item 17 must account for these obligations and the deferred Windows/Linux work.
