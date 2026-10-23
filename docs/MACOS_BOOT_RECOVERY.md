# macOS protected work journal and reboot recovery

This is source authoring for item 4. No build, test, XPC call, system restart,
signature operation or target verification was executed. Native execution remains
disabled because the independent resource/lifetime boundary is not complete.

## Authority and ordering

The broker's private profile database stores a closed `MacWorkJournal` in the
same FULL-synchronous transaction that reserves native copy debt. It binds the
profile, session, runtime, kernel boot-session UUID, reserved bytes and file count.
An exclusive profile lease stays held across cloned connections and session work.
Neither this database nor the lease is passed to the sandboxed owner or actor.
The owner receives only read-only input descriptors and bounded protocol fields.

Staging and launch follow this order:

1. Commit reservation and protected journal together before creating/copying work.
2. Write and flush the bounded broker work receipt and its directory.
3. Ask the authenticated owner to stage the declared read-only input descriptors.
4. Commit the staged phase and the journal hash in the private database.
5. Recheck the private receipt, staged phase, hash, execution capability and boot
   identity before sending any launch request.
6. The owner independently reads its kernel boot identity and flushes a schema-2
   launch fence before spawning. The fence binds profile/session/runtime/boot.

The owner-container fence is not itself reboot proof. Children currently inherit
shared writable container authority; a compromised current-boot child could
otherwise forge or remove metadata there. Reboot provenance comes exclusively
from the authenticated broker's protected database, outside that container.

## Recovery paths

Recovery scans database debts, including a reservation whose directory was never
created. A pass inspects at most four entries from the bounded 64-entry ledger.
A persistent rotating cursor prevents a retained first entry starving later work.

| Durable phase | Required action |
| --- | --- |
| Reserved | No launch path may have run. Ask the owner to discard staging; release only after confirmed removal of the exact local metadata directory. |
| Staged, current boot | Retain. An owner crash or missing numeric PID does not prove a previously launched child ended. |
| Staged, different boot | Validate protected journal/hash and ask the owner to recover that exact profile/session/runtime. The owner independently checks that its kernel boot UUID differs from the supplied protected prior boot UUID. |
| Retiring | Owner storage removal was confirmed before this phase. Finish only the bounded broker metadata removal and atomic debt/journal release. |
| Missing, foreign, legacy or invalid provenance | Retain; never promote writable container records to deletion/signalling authority. |

For a valid previous-boot fence the owner checks private lease/path/fence inode
identities, renames the final UUID to its exclusive `.retiring` journal, and
flushes the parent **before** clearing the fence. It then flushes that removal and
uses the existing flat bounded deletion routine. A crash before fence removal
requires the same protected reboot proof on retry. A crash after removal resumes
ordinary retirement; it never exposes a final UUID without its fence.

Regular orderly retirement requires an authenticated confirmed-ended reply for a
launch attempt, owner-container discard acknowledgement, the durable retiring
phase, exact local directory removal and parent sync. The final transaction drops
the copy-debt row and protected journal together. Generic collectors refuse rows
with macOS provenance, preventing an orphaned journal or premature quota refund.
Unknown files, links, subdirectories and old schema-1 fences remain refused.
Broker-local cleanup accepts only its bounded `mac-work.json` metadata file; it
does not remove imported game files, save history or another platform's journals.

The registry may establish authenticated **control/recovery** access while the
owner reports execution unavailable. This does not enable a runtime: owner launch,
Rust launch and product factory all independently retain execution checks. Any
remaining copy debt also blocks new native sessions. Web/library access remains
available through the normal unavailable-native fallback.

## Platform source and remaining limits

Both sides read the exported read-only `kern.bootsessionuuid` through
`sysctlbyname`, require a bounded non-nil UUID and normalize its hexadecimal case.
Missing/denied/malformed kernel data refuses launch/reboot recovery; there is no
environment, wall-clock, uptime or disk-UUID fallback. Apple's XNU implementation
registers this node read-only in
[kern_sysctl.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sysctl.c).
This reference informed the source; it does not establish behavior on a target Mac.

This implements a conservative reboot recovery route. The new diskless guest
source in `MACOS_DISKLESS_GUEST.md` replaces the native host-core child and its
shared write authority. Its helper lifetime reader is outside the untrusted core
address space. These are unqualified source changes; unknown legacy data remains
retained. The user explicitly authorized source activation after the earlier
automatic rejection; the execution gate is now true, but no runtime was rebuilt
or tested and verified item-4 completion is not claimed. Independent
item-6 media fixes are tracked in `GRAPHICS_AUDIO_INPUT_ITEM6.md`.
