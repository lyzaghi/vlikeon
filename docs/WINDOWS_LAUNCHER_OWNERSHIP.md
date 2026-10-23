# Installed Windows launcher process ownership — item 57

Source authored only. Build, tests, native API execution and launcher/device
qualification are deferred by the owner. Item 57 stays open. Streaming policy
remains disabled and no new renderer command is introduced.

## Source path

`platform/stream-input/src/game_process.rs` owns a fresh unnamed Windows job and
the original launched process handle. `InstalledGames::launch` uses it on
Windows, after the existing local executable selection, hash check and retained
file guard. The Unix `Command` path is unchanged.

The executable and working directory remain explicit, with only quoted argv[0].
The startup attribute list admits one NUL standard-I/O handle and assigns the
job during `CreateProcessW`, before the first thread runs. A failed job/attribute
setup or process creation refuses the launch; there is no untracked retry.
There are no fallible admission operations after successful creation that would
report an already started game as a retryable launch failure.

This is an ownership group for locally selected software, not a sandbox. The
job has no kill-on-close, active-process/memory quotas or breakaway allowance.
Closing the broker drops its handles without deliberately terminating games.
The normal user environment is inherited, as with the prior launch path.

On Windows, installed-session polling reads job accounting. The session stays
running while any assigned process is active, even after its original launcher
exits. It becomes ended only after the group has no active processes and the
original process handle is signaled. If more than one process was assigned,
`exit_success` is unknown: successful launcher exit does not prove game success.
Existing durable session-ID exit accounting remains the only playtime commit.
Accounting refusal retains the owned session and reports an error.

## Descendant admission

The host-only `InstalledGames::stream_selected_process` takes a borrowed
process handle retained by native window discovery/capture. It duplicates
only query/synchronize rights, retains the original process object's creation
identity and checks liveness before/after membership in this exact session's
retained job. A foreign session, dead process or failed query is refused.

It accepts no PID, parent-PID tree, executable-name match, remote assertion or
serialized authority. It does not enumerate or automatically choose descendants.
The existing `stream_process` remains a live direct-root compatibility path;
once that launcher exits it refuses rather than substituting an arbitrary child.
Membership is necessary, not sufficient, for capture/input. The native chooser
must still bind local consent, exact HWND, exact retained process instance,
game/session/capture owner, expiry and ongoing liveness/revocation.

The broker input authorization path now admits the exact process handle retained
by its capture record through `stream_selected_process`, instead of asking for
the initial launcher again. It repeats exact-job admission and identity checks
after local input consent and installed-helper admission. The final page,
capture and certificate deadline check happens after those potentially slow
operations, before computing the worker's remaining lifetime. The two-phase
capture selection below now supplies the retained descendant handle in source;
it has not been built or exercised.

## Native window choice source

`game_windows.rs` now authors host-only top-level window discovery for a retained
game job. `InstalledGames::stream_windows` returns at most 32 private choices,
each holding its actual read-only process handle, exact creation identity,
window and native observation time. Discovery visits at most 8,192 windows and
checks a 250-ms budget between operations and on return. A limit/error discards
the result, rather than presenting a silently truncated choice list. Individual
unreadable/nonmember/dead processes are omitted; this is not proof that no other
game window exists. The OS calls themselves do not have a hard timeout.

The probe ignores the broker's own windows and invisible/minimized/non-top-level
windows. It reads a bounded 512-unit caption only after membership admission,
removing control and bidi-direction formatting characters for display. Titles
never establish ownership. Protected, cloaked, unsupported or newly created
windows may still be absent or refused by the later capture backend.

Choices have no deserializer, caller-controlled process/window constructor or
automatic first-choice behavior. `stream_window_process` rechecks the selected
choice against the exact game job, process identity/liveness, visible HWND/PID
and a 30-second lifetime. Another session's choice is refused. Final local capture
consent and revocation remain mandatory. HWND reuse within the same live process
is not proven impossible by these observations and needs capture-lifetime
binding/qualification rather than a claim of atomic OS selection.

## Two-phase capture selection source

`stream_capture_prepare` now verifies the existing signed pairing and obtains
native job-owned window choices, including descendants after launcher exit.
Its response contains a capture UUID, game/pair/session/expiry, a choice expiry
and at most 32 `{choice_id, title}` entries. The broker retains the actual
process/window objects; the renderer cannot submit a PID or HWND as a choice.
One pending choice group is allowed, with a 30-second original choice lifetime.

The Game Hub shows a placeholder and requires the user to select a window and
press Use this window. It then calls `stream_capture_select` with the existing
capture/session UUID and the opaque choice UUID. The broker re-admits the exact
choice before and after audit, checks original request/choice deadlines, consumes
the pending group once and retains that selected process/window as the capture
target. Expiry is inherited from the original pairing/preparation; selection
does not extend it. Other choices and their process handles are dropped.

The component worker owns prepare/select correlations per live document,
checks all returned capture/game/pair/session identities, keeps process identity
and expected HWND private, and strips them from renderer responses. Selection
from another document or with an unknown/expired choice is refused. Late
successful selection after page closure/cancel/timeout is retired rather than
published. Changing the selected game or refreshing releases the old selection.
The worker bounds retained capture records and prunes expired ones before a
new prepare. Broker close/release drops pending choices and selected captures.

After selection, the user must still approve Chromium's native window picker.
The existing Chromium patch checks the selected process's exact PID/creation
identity before registering a media token. The component worker additionally
requires its returned native HWND to equal the broker-selected window before
giving the token to the renderer. A different window is rejected and its unused
token is never returned for getUserMedia. This is not a new browser-side exact
HWND check before token registration; that distinction remains relevant to
qualification. Native-video handoff and input arm also require the recorded
selected HWND. No remote peer receives native process/window identities.

This connects the source selection path; no chooser, media token, process API,
input helper, application build or test was executed. Streaming policy remains
disabled. Original pending choices can be refreshed if a game window appears
later; external existing-client/service launches are not inferred as owned.

## Remaining scope

- Qualify the two-phase selector, component-document correlation and final
  Chromium/native capture/input binding, including cancellation and late replies.
- Resolve persistent launcher playtime policy. Background launcher/helper lifetime can keep the whole
  installed session open; this is not exact gameplay-time measurement.
- Launcher handoff to an already running external client, service, WMI or other
  process outside this job is unsupported. Do not infer ownership from PID/name
  or attach unrelated processes to make it appear supported.
- Qualify nested jobs, launcher/anti-cheat compatibility and denied creation on
  supported Windows versions; unsupported job setup currently refuses launch.
- Qualify direct launch, root exit before child, nested descendants, PID reuse,
  foreign/dead handles, concurrent sessions, group drain, broker close, accounting
  errors, duplicate exit accounting and local capture consent/revocation.
- Broker restart recovery and Linux/macOS descendant ownership remain missing.

## API references

The implementation follows Microsoft's documented
[creation-time job assignment](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute),
[job inheritance and breakaway rules](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects),
[exact job membership query](https://learn.microsoft.com/en-us/windows/win32/api/jobapi/nf-jobapi-isprocessinjob)
and [job accounting](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_accounting_information).
These contracts guide source review; they are not execution evidence.
