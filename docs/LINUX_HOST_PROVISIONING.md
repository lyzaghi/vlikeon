# Linux delegated native-host provisioning source

This source implements the trusted in-process owner for a finite cgroup v2
subtree that has already been delegated legitimately to an OmniPlay user
service. It does not discover a cgroup from a renderer path, write an ancestor
controller, invoke systemd itself, elevate privilege,
move an unrelated process or treat filesystem writability as delegation.

The host accepts an already opened `O_PATH|O_DIRECTORY|O_NOFOLLOW` service
cgroup descriptor and a private ordinary 0600 single-link host lease. The user
service executable obtains that descriptor only from its kernel-reported
unified cgroup membership under `/sys/fs/cgroup`. It accepts direct membership
in the exact service top or the exact `manager` subgroup. For direct membership,
it creates/validates a private manager child and moves only its own PID there
before enabling delegated child controllers. It refuses
root, non-cgroup-v2 filesystems, mutable group/other ownership and service names
other than `omniplay-native-host.service`. The service top must become an empty
domain cgroup. Deployments may use `DelegateSubgroup=manager` to let systemd
perform the placement first; the supplied unit also works on delegated user
managers without that setting by doing the exact self-placement above. The service manager must
have granted the `cpu`, `memory` and `pids` controllers. Official systemd
documentation says `Delegate=` delegates controller management and
`DelegateSubgroup=` places the service processes into a subgroup; it also makes
clear that the top-level attributes remain managed by systemd:
<https://www.freedesktop.org/software/systemd/man/258/systemd.resource-control.html>
and <https://github.com/systemd/systemd/blob/main/docs/CGROUP_DELEGATION.md>.

Admission enables only the already delegated cpu/memory/pids controllers in the
empty service top, then creates or validates one private
`omniplay-native-profiles` domain. New trees receive finite read-back ceilings:
at most eight profiles, one to eight sessions per profile, 512 MiB to 16 GiB per
profile, no swap, OOM grouping, bounded tasks, one to eight CPU equivalents,
depth two and an exact descendant ceiling. The profiles aggregate is capped at
64 GiB plus its fixed 128 MiB host reserve. Existing aggregate/profile limits
are compared exactly and are never rewritten around descendants.

Directory inventory is fd-relative and bounded to 256 entries. Non-directory
objects are accepted only when the cgroup filesystem reports them as regular
control files. Every directory must have the exact
`omniplay-profile-<uuid>` shape, current-user ownership, mode 0700 and admitted
finite limits. Unknown names, links, special files, excessive entries or
profiles beyond the policy fail closed. The retained host lease serializes this
tree owner against another process; an in-process mutex serializes profile
creation and inventory. Neither lock is evidence that systemd actually granted
the service correctly, which remains a deployment qualification requirement.

Profile acquisition takes a separately opened private 0600 single-link profile
lease. A new exact UUID profile is created only below the held aggregate fd,
receives finite limits and enables the same three controllers for its child
session leaves. Existing profile ceilings are checked without altering them.
The result is a `RecoveryDelegation`, so it cannot launch or signal a process.
Native runtime composition first authenticates the installed release inventory
and requires Linux image policy on every selected package. It then canonicalizes
and reopens the private profile root, opens the bounded image cache, acquires the
profile recovery owner and runs the durable Linux work recovery audit. Only a
clean `finish()` conversion supplies normal launch authority. Unknown live,
visible or dying descendants and incomplete ownership proof therefore keep the
native runtime unavailable.

The source includes a socket-activated per-user service executable and user
service/socket units. The socket is `%t/omniplay/native-host.sock` beneath mode
0700 ancestry and has mode 0600. The daemon adopts exactly one systemd activation
descriptor, checks its path/type/listening state and authenticates peers with
`SO_PEERCRED`. Requests are fixed 48-byte frames containing only an exact profile
UUID. Replies are fixed 16-byte status frames with exactly one `SCM_RIGHTS`
profile cgroup descriptor. Both sides use five-second I/O deadlines. A received
fd must still be a current-user mode-0700 cgroup-v2 directory and must pass the
full profile identity/limit/recovery audit; the filesystem cgroup path never
crosses this protocol.

The legacy `NativeRegistry::installed()` path remains unavailable for Linux and
cannot infer or manufacture this host. `installed_linux()` remains the explicit
lower-level composition for a caller that already owns an admitted delegation.
`installed_linux_host()` supports same-process trusted composition. Normal Linux
broker startup now attempts `installed_linux_service()` once per opened browser
profile. It connects through socket activation, creates/validates the private
profile lease, admits the transferred fd, audits retained work and exposes native
packages only after clean recovery. Failure falls back to the unavailable Linux
registry while library/export/web-core features stay usable. Remaining startup
work includes uninstall/repair policy and durable migration of active trees
between future policy versions.
A renderer request, origin string, environment path or
ordinary native-messaging process must never become cgroup grant authority.

Linux package authoring and package admission now require the fixed executable
path `libexec/omniplay-native-host` plus both user units. The stable installed
launcher has a private `--linux-native-host-service` route. It verifies the
active signed release and fixed host entry, then uses Unix `exec` so the service
PID and systemd activation descriptor survive unchanged. This prevents a unit
from pointing directly at an obsolete version directory after activation or
rollback. The service is not counted as a browser health launch and cannot
trigger browser rollback policy.

The explicit per-user GNU/Linux installer admits the stable launcher and updater
with caller-supplied SHA-256 pins, then delegates release and package admission to
the signed updater. It rechecks the active package inventory and bytes before
rendering the service template to the stable launcher path, atomically publishes
private user units and the fixed-origin native-messaging manifest, reloads the
user manager and enables only the socket. It rejects root use, another target
architecture, links or special inputs, a different installed bootstrap,
malformed unit templates and an invalid active release. Package authoring and
unpacking both require the single placeholder launch route and fixed socket
identity. The service home view is read-only so the stable launcher and active
release beneath the user's data directory remain readable.

The daemon polls its activation listener with a finite interval. After five
consecutive idle minutes it exits only if every admitted profile reports no
populated or dying descendants. A running or dying native tree resets that idle
window. Updating and reloading the user service definition therefore never stops
an active delegated tree; the persistent socket activates the new signed host
through the stable launcher after safe idle exit. The service unit has no
install target and cannot be enabled independently; the installer enables only
the socket and never issues stop, restart or try-restart for the service.

Explicit unregistration disables and stops only the activation socket. Before
doing so, it reads the loaded service relationships and refuses a legacy unit
whose lifetime is still bound to that socket. The current service has ordering
only, so stopping the socket cannot propagate a stop into the service. The tool
then waits for the host's own cgroup-aware quiescent exit. A timeout restores
socket activation before returning and leaves every registration file intact.
Only after both units are inactive does it remove exact current-user regular
unit files and the byte-bounded fixed-origin manifest. Installed program
versions and all user content stay intact. Socket-option migration, program
purge or repair and active policy-version migration remain separate fail-closed
maintenance work.

Stable bootstrap replacement has a separate explicit maintenance source. It
requires caller-supplied hashes for both installed and replacement launcher and
updater files, holds a private nonblocking maintenance lock, stages and flushes
replacement bytes, and records the complete old/new transition in a private
atomic journal before changing either file. It replaces the updater first and
the launcher second, accepts only the old or new hash during recovery and
removes the journal only after both final hashes match. Atomic rename lets
already running processes retain their original executable inode; the command
does not stop sessions, change the active release or touch profile data.

Partial aggregate/profile creation has a narrow idempotent recovery path. If an
exact owned aggregate or UUID profile exists with mismatched/partial ceilings,
the host repairs it only when cgroup events, task count and descendant counts all
prove it completely empty and not frozen. Any visible or dying descendant blocks
all rewrites. Exact active trees are only compared. The source never removes an
aggregate/profile directory, relaxes a live limit, kills an unknown PID or claims
capacity reclamation. Existing per-session retirement and creator-bound work
recovery remain the only authored child cleanup paths. Migration between future
policy versions with active descendants remains unimplemented and fail closed.

No compiler, installer, systemctl, systemd/user manager, cgroup/kernel operation, profile recovery,
package admission, broker, helper, core or test has run. After all original
authoring, the one final verification must cover real systemd versions with and
without `DelegateSubgroup`, unified cgroup v2, controller absence, service and
manager placement, ownership/mode/fd replacement races, stale limits, full
capacity, concurrent processes, repaired partial aggregate/profile creation, active and
dying descendants, descriptor-leak rejection, lease loss, recovery conversion,
five-minute quiescent exit, socket-preserving upgrades, installer interruption,
bootstrap pin mismatch, unit rendering, shutdown/upgrades and real
kernel accounting. This source is not evidence of product enablement.
