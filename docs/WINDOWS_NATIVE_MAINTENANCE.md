# Windows temporary-session maintenance source

This source connects the item-2 Windows recovery owner to Settings. No native
command, database query, worker, Windows/process/file API, UI, compiler or test
has executed during authoring.

## Capability and control protocol

Only the desktop Windows broker advertises
`native_work_maintenance_protocol=1`. Other desktop/mobile/Web hosts omit it or
report zero, and the common UI hides the panel without probing unsupported
commands. The versioned request envelope remains control protocol 1.0.

- `native_work_status` returns finite profile-local staging reservation totals,
  phase counts and fixed limits. A single aggregate statement checks the exact
  profile owner and rejects inconsistent/out-of-policy records. It returns no
  path, game title, process ID or secret. `physical_bytes` stays null; the values
  are reserved copy budgets, not allocated disk measurements. `recovery_running`
  comes from the held Windows thread object's actual completion state, including
  a still-retiring worker whose original broker owner has closed.
- `native_work_recover_start` starts one bounded recovery pass using the
  broker's own root/profile identity. The renderer supplies no path, process,
  work UUID or deletion list. A fresh job UUID is returned.
- `native_work_recover_status` reads the job's finite counters and lifecycle.
- `native_work_recover_cancel` sets a cooperative stop request and reports
  stopping/terminal state truthfully. It does not claim to preempt a current
  blocking OS/filesystem operation.

The first startup pass is system-owned and uses this same asynchronous worker.
Windows registry construction no longer executes work recovery on the broker
control thread before hello. A startup/system or another document's pass is
visible only as busy through aggregate status; it grants no cancellation token
to an unrelated document.

## Worker and lifetime

The native-runtime library retains one process-wide recovery slot, separate
from its 16 IPC slots; each broker retains at most one result snapshot. A new
worker cannot start until the previous actual OS thread has ended, including
TLS destruction. Cancellation, thread-body return and a terminal-looking result
are insufficient. A nonblocking Windows handle poll precedes any join, which
occurs outside the retirement mutex. Status polling never joins a thread.
Closing the broker requests cancellation and transfers the handle/permit into
fixed retirement storage. A replacement broker cannot bypass that owner or
start another collector while the earlier OS thread remains live. An unreturned
kernel call retains its slot; unexpected owner loss retains the charge rather
than claiming shutdown completion. See `NATIVE_CONTROL_WORKERS.md`.

The recovery owner checks cancellation during bounded inventories and between
entries, attempts at most 32 UUIDs and stops starting more entries after a
30-second processing opportunity. Those are cooperative limits, not a hard OS
I/O deadline. Progress distinguishes attempted, recovered, retained and unknown
entries and reports when more candidates remain. A completed pass may still
retain live, unknown or incomplete-ownership data.

Candidates include bounded ledger-only reservations as well as directory/proof
entries. The per-candidate Windows creation lease prevents recovery from racing
an active reservation-to-owner handoff. Only newly stamped protocol-1 reserved
debt gains the pre-launch admission described in `WINDOWS_PRELAUNCH_RECOVERY.md`;
old missing/partial ownership is not inferred safe.

Every actual deletion retains the exact lease/ownership/process/AppContainer,
retiring-ledger and external-proof conditions described in
`WINDOWS_NATIVE_COMPLETION.md`. No process is signalled from a stored PID and no
unacknowledged work is guessed safe to remove.

## Document and UI ownership

The component tracks explicit job IDs per live Game Hub document. Another tab
cannot poll/cancel a known ID just because it shares the native connection.
Closing the owner document, losing its port or receiving a late start result
requests cancellation through the same native-port generation. A late terminal
status is not consumed as a successful delivery; the original document can
still collect its result later. Native disconnect clears all handle ownership.

The localized Settings panel has explicit inspect/recover, refresh and stop
controls. Starting never retries automatically. Cancellation intent survives an
unacknowledged start; failed stop acknowledgement has an explicit retry. Polling
pauses outside Settings and while hidden, page exit requests stop, and native
disconnect invalidates pending response generations. A busy system/other-document
pass is followed through readonly aggregate status only. DOM controls are
retained while status changes so polling does not replace the focused button.
Untrusted response data is strictly bounded and inserted as text. The panel
describes reserved bytes separately from actual disk usage and does not equate
a successful pass with complete physical reclamation.

## Required final evidence

Item 16 must build the complete broker/shared contracts and exercise handshake
availability, foreign-profile rows, two tabs, stale IDs, late start/cancel/status,
worker completion versus stopping, actual blocked I/O, startup responsiveness,
control-loop continuity during cleanup, hidden/frozen/disconnected pages,
Settings keyboard focus and locale, counters/limits, partial deletion/proof/debt
recovery and physical resource observations. Remaining legacy ambiguous records,
actual filesystem durability and complete scratch/AppContainer physical resource
controls are still item-2 work; this panel does not declare those complete.
