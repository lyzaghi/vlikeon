# Windows private presentation launch source

This authors an optional broker-owned software actor/presentation pair. No Rust
compiler/Cargo resolver, CMake/SDK/compiler, executable, process/thread/pipe/ACL/
window/input/audio/SDL/OS API or test ran. No native binary was admitted or catalog
entry published. The default remains direct presentation; macOS/Linux native
isolation remains unavailable. Every original product requirement stays active.

## Signed package admission

The optional runtime `presentation` record has exactly protocol 2, an executable
in the same signed/inventoried package directory as its worker/core, dimensions
1..4096 and helper memory 256..2048 MiB. The executable differs from both worker
and core. Software video is required; OpenGL does not silently become a software
profile. Dimensions reserve sixteen bytes per pixel plus 128 MiB inside the
declared helper limit. That is an admission formula, not measured driver/RSS use.

All package files, including the helper, remain held by verified readonly handles
that refuse replacement on Windows. The worker's hello must name protocol 2,
`brokered_rgba` and measurement protocol 0. An undeclared brokered hello is
rejected. Existing catalogs with no presentation field retain the direct path.

The authored build recipe has an explicit `--presentation-mode brokered_rgba`
option, helper dimensions/memory, a same-build helper target and PE architecture
admission. Its default is direct. A brokered package inventories the helper and
omits the unqualified performance-measurement capability. First-party source
hashes before/after the future build and actual output hashes are retained in its
corresponding-source receipt. This recipe has not been run; a source inventory,
manual distribution flag or build receipt alone is not release qualification.

## Process and handle ownership

The broker creates two anonymous media pipes with a fresh canonical nonzero work
UUID. The normal-user helper inherits only its input/output and NUL stderr. The
AppContainer actor inherits its independent control stdio/NUL stderr and exactly
the two media endpoints. Handles, session and finite limits are fixed bootstrap
arguments; media packets carry no filenames, executable commands, credentials or
native handles. Private duplicates become noninheritable before SDL/core startup.

Each process starts suspended and is associated with its Job Object atomically
through `PROC_THREAD_ATTRIBUTE_JOB_LIST` in `CreateProcessW`. An owning process
guard exists before journal publication can fail. Separate kill-on-close jobs limit
each process tree to one process and its declared memory; the actor retains its
AppContainer with no network capabilities, existing mitigations and CPU limit.
The broker confirms actual membership with `IsProcessInJob` before recording
identity or resuming. Unsupported creation attributes or rejected job membership
fail launch; there is no later-assignment fallback. This closes the authored
create-before-assignment crash window for both actor and presentation helper.
Neither launch path relaxes Chromium or guest isolation. Both process identities
(PID plus OS creation time) are durably recorded before ResumeThread. Failure
while creating the actor drops/terminates the already created helper guard.

The brokered software actor additionally requests the immutable Win32k-system-
call-disable creation mitigation (SDK WinBase.h bit 28). The helper keeps its
desktop role; an actor/core cannot acquire that role merely by calling GUI APIs.
There is no retry with that bit removed. This depends on actual loader/SDL/core
qualification and is not established just by avoiding frontend window calls.
See Microsoft's [process creation mitigation policy](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute).

The helper is trusted normal-user desktop/audio code. It is not an AppContainer
or a second filesystem/network isolation boundary, and its Job Object is not
claimed to provide one. It loads no emulator core or game and exposes only the
closed media/input parser to the actor. Its fixed environment names System32 and
a separate medium-integrity temporary directory; its working directory is the
verified runtime package. The actor gets no write grant to that helper directory
or temporary directory and gains no helper process token/desktop socket.

Cleanup terminates and observes both actual processes before retiring the profile
and removing work. An uncertain exit marks the shared identity uncertain and
retains work instead of assuming that a queued Stop was acknowledged. Abandoned
reclamation checks both recorded PID/creation-time identities and refuses an
interrupted launch without a confirmed identity. It never kills a PID read from
disk. The pre-existing missing-acknowledgement retention cases remain conservative;
this does not claim that every interrupted journal can be reclaimed automatically.

## Focus and retry

An explicit broker start/resume enumerates only the exact live helper process's
visible root `SDL_app` window (the class in the pinned SDL Windows source).
Ambiguous ownership fails. It requests asynchronous restore and normal Windows
foreground activation, then requires actual foreground equality. There is no
synthetic input, thread attachment, broad foreground grant or policy bypass.
Windows may refuse an activation even when documented conditions appear to hold.
See [SetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow)
and [AllowSetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-allowsetforegroundwindow).

The actor waits at most 750 ms for a fresh, focused helper snapshot before queuing
running State. It consumes that admitted focus epoch, so a previous focus change
does not immediately pause a successful resume; any subsequent epoch still does.
A held Escape does not become a synthetic new press. Internal save/restart resume
does not ask the broker to steal focus and still needs current real focus.

A framed `permission_denied` remains nonterminal on the control channel. Public
Ready/Paused-to-Running focus refusal retains the idle loaded session. The Game
Hub keeps its controls/polling and explains the own-window Escape or Resume action
in Vietnamese/English. A Ready retry uses start; a Paused retry uses resume. Exit
can save/stop an idle Ready session without trying an invalid pause transition.
Unrelated crashes, malformed frames and timeouts retain their terminal handling.

## Creation-time ownership revision

The current item-2 source additionally requires creation-time job association
for both processes. Historical source checkpoints below precede this revision.
It remains uncompiled and unexecuted. The API contract is documented by
Microsoft in
[UpdateProcThreadAttribute](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
and [closing the create/assignment race](https://devblogs.microsoft.com/oldnewthing/20181116-00/?p=100245).

## Source state and remaining evidence

The source producer freezes the presentation/frontend sources plus the Windows
sandbox/native-runtime sources, workspace manifests/lock, session contract/manager
and shared native-player flow. Its broker selection is explicitly incomplete as
a standalone Cargo workspace; an actual build needs the complete product tree,
dependencies and qualified target SDK. A frozen source project enables no product.

The current candidate is `out/presentation/host-v4-windows-win32k-source`:
1,643 files / 33,809,102 bytes. The earlier
`host-v4-windows-owned-window-source`, `host-v4-windows-pair-ready-source`,
`host-v4-windows-pair-source` and all protocol-1/2 frontend/client/host source
snapshots remain preserved. All six shared resource/integration source assemblies
were regenerated without cores. No configuration/build or final verification ran.

The single final verification must cover actual helper/core builds and signed
admission; OS inheritance/AppContainer/Job behavior; partial startup/crash/parent
death and blocked pipes; PID reuse/uncertain exit/journal cleanup; foreground
refusal, held-input/epoch and fullscreen races; final saves after presentation
loss; driver mitigation compatibility, audio/presentation clocks and physical
controllers; dependency/source/license provenance and real device/resource bounds.
Real POSIX containment/launch/activation, GPU presentation, qualified measurements,
primary/mobile platform work and all remaining original release scope stay due.
