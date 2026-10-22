# Creator-bound interrupted launch recovery source

New Linux work ownership records now use schema 2 and bind the creating broker's
observed boot UUID, PID and process start ticks before any work launch. The
sandbox observes its own live process through a pidfd and requires the returned
identity to match the current broker PID. The exact same closed ownership body
is published as both work intent and owned acknowledgement. No such observation,
record, process/kernel operation, compiler or test has run during authoring.

This evidence addresses one interrupted-launch gap: a valid launch intent may
exist even when the broker dies before committing the observed child identity.
The collector cannot invent that child's PID or conclude it never existed.
For a schema-2 work with a complete matching ownership/group chain, it can
instead prove the exact recorded creator broker ended using the existing
read-only boot/PID/start-tick query. It still independently acquires all work
leases and requires both recorded resource leaves to be observably empty or
already absent, with no dying charge before work removal. A dead creator alone
is not proof that its children or storage resources have ended.

The launch intent must still have the exact group identity, bounded executable
length and valid expected hash. An observed child acknowledgement, when present,
must independently match and prove exact child exit. An acknowledgement without
launch intent, malformed/mismatched record, live or unqueryable creator, occupied
group or unknown resource state remains retained. Neither creator nor child
identity ever authorizes signalling a numeric PID. No process is killed by
this recovery branch.

The external immutable retirement proof records an explicit per-role
unacknowledged-launch flag, separate from the earlier never-launched case. It
retains the schema-2 creator identity, and every resumable deletion retry
revalidates creator death for that flag before the usual resource/name/debt
checks. A flag alongside an observed child acknowledgement is rejected. Existing
schema-1 complete-pair records and earlier retirement receipts still use their
observed child proof; schema 1 cannot use the creator fallback. Ownership intent
and owned acknowledgement must match in full, not only by directory identity.

The work, group, bootstrap/media/owner protocols and database remain independent:
only Linux work ownership advances to 2. No installed catalog, default startup,
capability, release approval or verified target is enabled. Partial group
creation without an inode acknowledgement, missing original ownership, unknown
old resource parent, corrupt proofs and live/unconfirmed processes remain
conservatively retained. Full partial-origin recovery, legitimate provisioning/
startup selection, compositor activation, physical accounting and compiled SDK/
core/driver/license/device qualification remain required with every original
product/platform/release requirement.

The single final verification after all source authoring must cover creator
observation and record-before-launch ordering, crash before/after spawn/record/
gate, missing acknowledgement with live/dead/reused creator identities, boot
changes, old schema-1 records, strict ownership equality, actual lease/resource
quiescence and resumable proof retry. Source writing/copying supplies no evidence
that any target kernel/runtime behavior has passed.
