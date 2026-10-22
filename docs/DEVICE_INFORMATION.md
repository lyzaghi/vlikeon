# On-demand local device inventory source

`platform/device-info` now supplies a bounded native `device_snapshot` response to
desktop/mobile Game Hub. The Devices view explicitly requests it when opened or
refreshed, renders unknown values as unknown and keeps it separate from web API
availability probes. There is no background polling, telemetry submission or
automatic inclusion in diagnostics, save sync, measured performance reports or
trusted runtime-ranking/device-lab evidence. No probe, graphics API, native/mobile
service, compiler or test has been executed during this source work.

The report excludes machine/account names, serials, monitor identities, GPU LUIDs,
filesystem paths, installed-process lists and network identifiers. GPU model and
vendor/device IDs still reveal hardware characteristics and are confined to this
authenticated local surface. Strings and adapter counts are bounded. Reported
parallelism is the process's available parallelism, not an invented physical-core
count; process architecture is not represented as proof of host CPU architecture.

## Platform sources and gaps

Windows reads physical memory through `GlobalMemoryStatusEx` and power/battery
flags through `GetSystemPowerStatus`. DXGI 1.1 enumerates at most eight adapters
and returns bounded descriptions, vendor/device IDs, declared shared/dedicated
memory and software flags. The optional `IDXGIDevice` interface-support query
returns its UMD driver version as a labeled raw hexadecimal value. It does not
query D3D11/12 interface support with that method, create a rendering device or
claim a human-readable driver-package version. Enumeration identifies available
OS adapters, not the GPU actually selected by Chromium or the game. Windows build
and thermal state remain unknown in this adapter.

Linux/Android Rust source reads fixed `/proc/meminfo`, kernel-release and read-only
power-supply/DRM sysfs paths with byte/entry bounds. No shell, root access, arbitrary
renderer path, writable control or battery serial/model read is accepted. Missing
attributes stay absent. System-scope batteries use their published capacity/status;
multiple batteries are not averaged or mixed across charge/energy units. DRM
inventory currently covers enumerated PCI vendor/device IDs, not all SoC GPU,
driver-version or usable graphics capability cases. Android may deny these files.

The Android native layer augments only a successful explicit snapshot, using
ActivityManager memory, the system sticky battery intent and PowerManager battery
saver/thermal-status APIs. It stores no receiver or long-running listener and
requests no new permission. OS thermal enum values are presented as reported;
they do not become CPU/GPU utilization, temperature, measured throttling or a
performance-controller input. Platform version/source/time is separate from the
potentially cached Rust snapshot, so the mixed collection is not claimed atomic.

Apple Rust source uses fixed `hw.memsize`/`kern.osrelease` sysctls. The iOS main
queue then augments an explicit response with public ProcessInfo memory/version/
low-power/thermal-state and UIDevice battery state. Battery monitoring is briefly
enabled only if previously disabled, and the prior setting is restored. An
unavailable initial battery reading remains unknown; it is not manufactured as
zero. macOS graphics/power and iOS GPU/driver/available-memory inventory still need
their native adapters and qualification. Store privacy/API review remains required.

## Lifetime and evidence

One pending worker per native host performs the base probe. Calls wait at most two
seconds for that worker, returning busy while it remains pending; a timed-out
observation does not spawn another worker. Completed base reports are cached for
fifteen seconds. This contains request/thread amplification but cannot preempt a
hung kernel/driver call; dropping the host receiver only abandons the eventual
result. Collection time and timestamps describe that limitation. Mobile native
augmentation happens through the existing serialized native request path.

Neither inventory nor a boolean browser API probe proves game compatibility,
graphics performance, battery gains or driver reliability. No GPU allocation,
sensor temperature, clock, energy counter, active adapter or scanout timing is
invented. Controlled benchmark scenes, actual render-adapter identity, signed lab
captures and the full platform/device matrix remain required product work.
Final verification must cover absent batteries, USB/peripheral supplies, multiple
batteries/GPUs, permission-denied files, unknown statuses, integer/string bounds,
driver failure/stall, refresh/cache timing, mobile main/IO queues and privacy.

Primary contracts:
[Windows memory](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex),
[Windows power](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-system_power_status),
[DXGI enumeration](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-createdxgifactory1),
[driver-version query](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiadapter-checkinterfacesupport),
[Linux power supplies](https://docs.kernel.org/power/power_supply_class.html),
[Android battery](https://developer.android.com/reference/android/os/BatteryManager),
[Android power](https://developer.android.com/reference/android/os/PowerManager),
[Apple thermal state](https://developer.apple.com/documentation/foundation/processinfo/thermalstate-swift.property).
