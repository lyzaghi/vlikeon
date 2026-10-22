# Checked private Linux game and firmware staging source

`platform/native-runtime/src/linux_staging.rs` now authors the game/firmware
portion of the lower-level Linux pair. No source asset, firmware, private file,
database, descriptor, process, OS/kernel/API, compiler or test has been exercised.
Linux native execution is still unavailable in the product factory.

The intended source composition obtains StagedInputs::reservation from the
approved package/game and supplies that bounded reservation to LinuxWork::create.
The work owner commits its schema-14 debt before creating the UUID directory.
StagedInputs::prepare checks that reserved budget before any payload copy; it
does not create a second independent per-session budget.

The game and approved runtime declarations must agree on system, ROM limit and
content format. The existing canonical media manifest validates its aggregate
identity, sorted flat game/track/disc filenames, bounded assets and primary
entry. Only its primary descriptor is accepted from the preparation caller;
secondary assets are opened through held no-follow profile/asset components by
their validated content hashes. Raw-ROM naming remains the same game extension
contract used by other native/web paths.

Firmware comes exclusively from the user's existing FirmwareVault selection.
The signed core's allowed byte shapes, optional roles and at-least-one required
region groups are checked. Duplicate/reserved/non-flat names fail. Firmware is
never fetched or bundled by this path. The sum of actual selected payloads and
metadata allowance must fit the already reserved bytes/slots; package images
plus selected inputs must fit the bootstrap's 256-file ceiling. There are at
most 144 input files, including at most 128 canonical media assets and the
existing 16 firmware declarations.

Each ordinary source is copied cooperatively in 64 KiB blocks while hashing
the actual copied bytes against its expected identity. The cursor-independent
copy checks cancellation, exact source length and SHA-256. A successful copy
changes the held pending file to 0400, syncs and publishes it as a new file,
then reopens a no-follow/CLOEXEC/nonblocking readonly single-link ordinary
current-user file of the exact size. It records actual device/inode identity.
No writable staged descriptor is handed to the actor. Partial copy or failed
publication does not release its durable work debt or delete an uncertain work
directory.

The closed immutable linux-inputs.json receipt is at most 64 KiB. It binds
profile/work/game IDs, canonical game content identity, primary name/length,
actual payload total, each file's kind/name/hash/length/device/inode and selected
firmware hashes. The exact written receipt hash acknowledges the database's
staged phase. No pointer, arbitrary renderer path, executable or host credential
is encoded in the actor's media/input protocol.

LinuxWork::launch_pair now requires this typed input set. Before either child
can launch it checks the owned receipt bytes and staged ledger, refuses already
attached raw GameInput entries, reopens every private copy and compares it to
both its held descriptor and recorded inode, and adds only exact work/<name>
readonly inputs to the actor plan. The bootstrap's private root mounts those
selected files individually readonly/noexec; it does not expose the host work
directory, leases or journals. The existing pidfd/resource owner retains all
selected descriptors through actual process/tree exit. Files and reservation
debt remain until confirmed whole-work cleanup; dropping the staging object
does not declare them reclaimed.

These sources still need the actual NativeRegistry/NativeAdapter and full
control/prepare/save/final-save integration, legitimate delegation provisioning,
confirmed Linux crash recovery and bounded storage reclamation, compositor
activation and compiled SDK/core/driver/license/device qualification. The input
manifest is not a recovery collector or proof that a core executed correctly.
Native Windows preparation also clears previous selected firmware hashes during
cleanup so a later preparation does not inherit unused compatibility inputs.

The one final verification must cover canonical raw/disc content, corrupt or
changing source descriptors, optional/required firmware, cancellation and
ENOSPC/publication failures, readonly/private inode binding, manifest/ledger
consistency, file/byte/global reservation limits, gate/process/tree lifetime,
crash recovery and actual core/device behavior after all original authoring.
No such results exist; every original platform/product/release requirement
remains in scope.
