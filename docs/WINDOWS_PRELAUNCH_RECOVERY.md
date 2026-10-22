# Windows pre-launch reservation recovery

Source only. No database migration, query, Windows file-sharing operation,
native process, compiler, application or test has executed for this change.

## Creation handoff

A new Windows preparation holds the profile's external
`native-retirement/creator.lease` exclusively before reserving staging debt.
It keeps that lease through directory creation and acquisition of the work's
`owner.lease`. Only after the per-work owner is held may it release the creation
lease and continue copying. A competing preparation/collector must retry rather
than waiting on the broker control thread for this lease.

Database source schema 15 adds immutable `windows_creation_protocol` to the
existing ledger. New gated Windows reservations insert protocol 1 in the same
transaction as their debt. Migration gives every older row protocol 0; generic
and Linux reservations also keep zero. A trigger prevents later promotion or
demotion. The marker is a broker implementation contract, not filesystem quota
or an authorization token supplied by the renderer.

## Recovery admission

The collector merges at most 64 profile-checked ledger UUIDs with its separately
bounded work-directory and retirement-proof inventories. Thus a reservation
whose creator died before mkdir remains reachable. UUID cursor rotation and the
32-candidate cooperative pass limit still apply. The two external lease files
are recognized control entries, not unidentified user content.

Before processing each candidate the Windows collector takes the creation
lease. If the literal directory is absent, only protocol-1 `reserved` debt may
use pre-launch release; existing confirmed `retiring` debt retains its separate
release authority. A staged missing directory is not an uncreated preparation.

If an ordinary work directory exists without `owner.lease`, the collector
requires exact profile/work protocol-1 reserved debt and no Linux ownership or
Windows container/process journal. That establishes a preparation which cannot
have reached OS launch under this protocol. It then uses the same retiring
ledger, external confirmed proof and resumable confined deletion path. An
existing owner lease must instead be acquired exclusively as before. A live
creator or owner, foreign identity, unexpected path, conflicting journal or
uncertain file operation retains the debt and data.

Legacy reserved rows are never inferred to follow the new creation protocol.
Missing acknowledgements after container creation or process-launch intent,
staged data with missing ownership and all ambiguous legacy work stay retained.
This change does not remove arbitrary AppContainers or signal stored PIDs.

## Final evidence

The authored Windows cases cover live-creator exclusion, ledger-only recovery,
pre-owner partial directories, live-owner handoff, retained legacy/staged debt,
conflicting journals/profile IDs and immutable protocol markers. The core
migration case starts with an actual schema-14 debt layout and requires schema
15 to preserve its values with protocol zero. All cases await item 16.

OS sharing semantics used by the source are documented in
[Rust OpenOptionsExt](https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html#tymethod.share_mode)
and [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
Real contention/crash/durability evidence, legacy ownership recovery, complete
physical scratch/AppContainer resource controls and all original item-2 gates
remain required.
