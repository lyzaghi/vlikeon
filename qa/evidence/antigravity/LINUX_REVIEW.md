# First Linux compiler and socket qualification series

Codex designed each exact batch, reviewed the raw compiler/test logs and made
all source/configuration changes. Antigravity only dispatched the fixed manual
workflow and recorded its raw result. Each run checks its exact Git commit and
uses disposable GitHub Ubuntu24.04 runners with read-only repository contents
permission. No WSL, local Linux distribution or permission settings were changed.

| Batch | Source | Actual outcome |
| --- | --- | --- |
| 036 | 9da9982 | GitHub rejected invalid runner-context expressions before execution. No build or test occurred. |
| 037 | b1f7439 | Actual sandbox Rust compile failed on borrowed temporary; bootstrap C compile failed on older Landlock SDK. Socket tests skipped. |
| 038 | d07305c | Sandbox compile and all eight actual Unix socket tests passed. Bootstrap failed missing wait header; broker prerequisite libdbus headers absent. Overall run failed. |
| 039 | 2008e16 | Static-pie C bootstrap built and the same eight socket tests passed. Broader broker check encountered absent Wayland headers. Overall run failed. |
| 040 | 1d1af96 | Both workflow jobs succeeded: sandbox check, all eight socket cases, runtime/broker all-targets check and static-pie C bootstrap build. Tracked-source checks passed. |

These eight distinct socket cases observe fragmented descriptor replies,
CLOEXEC/blocking-mode handling, extra or malformed descriptor closure, early EOF,
ordinary-fd rejection and an invalid client followed by a valid one on the same
listener. The pipe EOF assertions detect leaked received writer descriptors.
Repeated passes across revisions are not additional distinct test cases.

Compiler failures led to retained string ownership, exact upstream UAPI source
and license hashes, missing sys/wait.h inclusion and explicit dbus/Wayland
development prerequisites in disposable runners. No core feature or mitigation
was removed and no test expectation was weakened. These SDK package versions
are recorded by the workflow; this is not an approved production SDK receipt.

Batch040's `cargo check --all-targets` checks the Linux runtime and both broker
executables, including test targets; it is not a linked production broker build.
The socket tests do compile/link and execute their own test binary. Bootstrap
is separately compiled/linked by CMake but is not executed. No SDK was installed
on the user's Windows machine. Actual run:
https://github.com/lyzaghi/vlikeon/actions/runs/39878187910

No run in this table establishes actual user-service delegation, sandboxed
actor/core launch, Landlock enforcement, cgroup retirement, compositor/device
behavior, complete Linux source closure or release readiness. The bootstrap
compile does not execute its namespace/policy code. See the exact per-batch
result.json and raw logs, and docs/LINUX_ITEM3_CLOSURE.md for remaining work.
