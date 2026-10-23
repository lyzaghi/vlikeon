# Explicit Linux runtime factory and adapter source

NativeRegistry has explicit GNU/Linux constructors and LinuxAdapter implements
the shared RuntimeAdapter interface. Normal Linux broker startup now attempts
installed_linux_service(); it exposes native packages only after the actual
service delegation, authenticated inventory and retained-work audit succeed.
The lower-level installed() path still supplies no Linux authority. Platform,
catalog and distribution acceptance remain pending. Current item-3 compiler,
test and source status is recorded in `LINUX_ITEM3_CLOSURE.md`; historical
source-authoring notes below do not supersede that evidence record.

## Trusted factory construction

installed_linux requires a typed, already admitted profile Delegation and a
bounded shared image-cache policy. It checks the profile UUID, then uses the
existing Store/channel/active-release path and the running broker's verified
entry identity. It reads only that release's inventoried catalog and requires
every selected Linux package to declare its actor image closure. It creates a
cache bound to the private canonical profile directory. It has no developer-root
fallback, renderer/IPC constructor, environment delegation override, implicit
systemd service or automatic permission grant.

The factory's availability decision is instance-specific: only that explicit
context plus the authenticated installed root/inventory can list/create Linux
adapters. Global sandbox::supported remains unchanged; default installed() has
no context and stays unavailable. The trusted host provisioning/startup flow
still must create/admit the actual delegated resource domain before this
constructor can be selected by a real product.

## Adapter and SessionManager composition

LinuxAdapter retains the signed catalog declaration independently of its
negotiated runtime capabilities/options. Preparation reopens the authenticated
catalog and requires equality of the whole declaration before staging. It loads
the profile's validated input mapping, reserves private copy debt before work
creation, prepares exact checked game/user-firmware inputs and passes their
owned work to the gated LinuxSession. The latter resolves selected options
through the signed policy, binds configuration/game identity and validates the
worker's hello/prepare contracts. Failed preparation drops the live process
owners; durable work/copy debt stays retained for confirmed recovery.

The adapter exposes the ordinary shared start/pause/resume/stop, save/load,
compatibility, status/close-cancel, metrics and disc interfaces. The existing
SessionManager can therefore use its normal prepared-adapter adoption, initial
save restoration, pause/final-save/conflict-preservation and stop flow once a
real host selects this factory. No separate final-save bypass or automatic save
deletion is introduced. Close/playtime observations remain explicit typed
worker data. The current hello deliberately has no performance measurement
collector, so inherited measurement methods remain unsupported.

Metrics require a closed bounded worker reply, a valid session ID, finite
nonnegative ordered frame-time percentiles and a window within the existing
300-second contract. CPU/GPU/power/thermal/network values remain unknown.
Counters and timing are the current authored software/helper observations,
not benchmark, driver latency or hardware render qualification. Disc selection
accepts only indices zero through seven, validates the returned inventory and
requires the acknowledged selected index to match. Save compatibility binds
the same launched configuration and staged game; actual effective core/options/
firmware/assets/persistent-file identity is supplied by LinuxSession.

Stop invokes the bounded graceful worker stop then the owned pair teardown.
The adapter does not retire its disk-copy debt or erase its work merely because
the primary process exited. Exact tree/leaf/asynchronous resource charge proof,
existing-tree crash recovery and bounded confirmed reclamation remain required.
Repeated preparation after stop uses the original signed declaration, not the
previous session's negotiated options/capability subset. A duplicate prepare
against a currently prepared adapter fails without tearing down that session.

## Remaining work and final evidence

The normal broker startup still needs legitimate host/service delegation
provisioning and migration, authenticated profile association and the explicit
factory selection. It must preserve the unavailable fallback on unsupported
kernel/libc/delegation hosts. Existing-tree crash recovery, confirmed bounded
work/debt/charge reclamation, compositor activation-token/user-gesture handling,
whole-profile physical accounting and compiled SDK/static bootstrap/dynamic UI/
core/driver/license/device qualification remain unfinished. Full GPU/JIT/macOS/
mobile and every original browser/runtime/streaming/sync/security/accessibility/
store/release requirement remain active.

Only source-copy/Git-index/hash inventory preparation is permitted so far. No
database/migration, asset/firmware, lease/cache/ELF admission, process/thread/
descriptor/kernel/SDL/OS/API, compiler/configuration/resolver/build, helper/core/
browser/device or test ran. The final single verification after all source
authoring must cover actual factory/profile/release association, adapter adoption
and final saves, immutable catalog equality, preparation/cancel/focus/error
paths, save/metrics/disc/close contracts, process/resource lifetime and confirmed
crash reclamation on real target systems. No such evidence is claimed here.
