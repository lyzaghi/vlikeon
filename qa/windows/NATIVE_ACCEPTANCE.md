# Item 2 Windows native qualification matrix

Authored cases only. No case has executed. Run in the single final consolidated
phase with approved disposable test content, isolated test profiles, exact built
artifacts and retained observations. A passing source/unit case does not replace
the required OS/process/filesystem evidence.

1. Build and admit the exact worker, selected core and optional presentation
   helper from their pinned source/SDK recipes. Check PE architecture, package
   hashes, corresponding source, ABI and actual mitigation compatibility.
2. Observe both direct and brokered process creation. The expected limited
   kill-on-close job must be assigned by `PROC_THREAD_ATTRIBUTE_JOB_LIST` during
   creation; `IsProcessInJob` and identity publication must precede resume.
3. Terminate the broker at each boundary around `CreateProcessW`, handle
   adoption, membership query, process-record publication and resume. Neither
   actor nor helper may survive outside its expected job. Unsupported nested
   jobs/attributes must reject without weaker fallback.
4. Race ACL-grant, launch and container-retirement callers. Retirement may not
   overlap setup, and no new grant may be added after the first launch attempt.
   Poison/error paths must preserve retirement uncertainty.
5. Verify inherited handle allowlists, empty AppContainer network capabilities,
   no writable executable/game/firmware files, private save/scratch grants,
   mandatory integrity, dynamic-code/image mitigations and software Win32k
   lockdown. Attempt prohibited access from the actual contained actor.
6. Exercise helper/core crash, malformed/incomplete/oversized frames, stalled
   reads/writes, runtime timeouts, process termination refusal and uncertain
   exit. Retained work/debt must never be released on a guessed termination.
7. Cover foreground refusal, helper window ambiguity, focus epochs, Escape,
   controller removal, rumble/audio stop, fullscreen/DPI and user close. Save
   failures must leave a recoverable paused state or preserve committed history.
8. Interrupt work cleanup before/after retiring-ledger transition, external
   proof publication, internal lease removal, each subtree deletion, literal
   directory absence, ledger release and final proof removal. Retry must resume
   the same confirmed work without requiring already-deleted internal journals.
9. Present proofs for the wrong profile/work, reserved/staged/missing ledger,
   nil/noncanonical UUIDs, unknown schemas, linked/reparse/special roots and
   partial directories. Retain uncertainty. Reused/unacknowledged identities
   must not gain deletion authority.
10. Run concurrent collectors and active preparations. The external collector
    lease must prevent overlapping deletion; the runtime owner lease must block
    first-time retirement of live work. A busy entry must not starve later
    acknowledged work across the persisted 32-entry cursor rotation.
11. Exercise disk full, read-only/locked files, database contention, malformed
    cursor, root inventories over their caps, external mutation and machine
    power loss. Record actual Windows publication and file-sharing behavior;
    retain proof/work/debt when stronger safety cannot be established.
12. Check final-save behavior and public UI status, recovery retry/cancellation,
    physical/private resource accounting and all legacy ambiguous records. These
    remain required even if atomic job creation and confirmed-proof retries pass.

13. Exercise the new Settings protocol and asynchronous startup/manual worker:
    unsupported host hiding; owner-mismatched aggregate rows; simultaneous tabs;
    unknown/late job IDs; stop before start reply; failed stop acknowledgement
    and retry; one actual worker until its thread finishes; blocked OS I/O;
    hidden/frozen/disconnected page generations; readonly busy observation;
    responsive normal native control during cleanup; locale and stable keyboard
    focus. Reservation bytes must never be presented as physical disk recovery.

14. Interrupt new creation before/after reservation commit, mkdir, owner-lease
    creation, creation-lease release, copy/staging publication and staged commit.
    A live creator must exclude collection even while its directory is absent;
    a live owner must exclude collection after handoff. Recover protocol-1
    reserved ledger-only and ordinary unleased partial preparations, including
    a crash during their later proof/deletion sequence. Retain legacy/default-0
    and staged missing/partial debt, foreign profiles, conflicting OS/Linux
    journals and reparse parents. Migrate a schema-14 fixture without changing
    any debt identity/budget/phase; its new marker must be zero and immutable.
    Exercise simultaneous startup/manual recovery and preparation; no new
    process may run after its reservation was recovered. Run the authored
    core migration and native Windows recovery cases in the final pass.

15. Build the separate `storage-probe.cpp` final-pass fixture and explicitly run
    the actual-actor case described in `STORAGE_PROBE.md`. Require real default
    filesystem write/create/WRITE_DAC/label-lowering denial and private scratch
    write/flush/delete success. Observe the exact actor SID, low integrity and
    no-write-up policy before resume. Exercise supported profile layouts,
    preexisting explicit/inherited child permissions, alternate streams,
    root replace/delete/rename, unknown/network/reparse layouts, oversized/deep
    fresh trees, permission failure at each partial sealing step and exact OS
    profile retirement. No ancestor/user-owned AppData tree may be relabelled.
    Token query/policy failure must prevent identity publication and resume.
    Check actual production core/helper/CRT/driver behavior; do not substitute a
    passing probe for registry, private-writes or physical quota evidence.

16. Observe private-registry sealing through the actual suspended actor token:
    exact effective thread SID/integrity/policy; one dedicated lookup thread;
    failure without privileged-identity fallback; successful revert and joined
    thread before security updates; no original-caller impersonation. Reject
    predefined/wrong-hive/missing-SID grants, inherit-only grants and another
    profile's key; accept applicable inherited exact-SID grants. Preflight must
    reject REG_LINK keys, oversized/deep inventories and inaccessible keys before
    mutation. Inject lookup/token/revert/join/label errors and verify no process
    identity publication/resume. Keep two profiles' unrelated sentinel values
    unchanged. Exercise readonly private lookup, actual child-key creation and
    KEY_SET_VALUE denial via storage-probe-v2, then production core/CRT/helper/
    driver compatibility, crash/retirement and broader granted registry access.

17. Execute `qa/native-memory/ACCEPTANCE.md` in the final pass. Build the exact
    memory-file MD core/worker and require protocol/header/receipt agreement.
    Confirm actual denied OS work save/scratch writes alongside successful
    canonical SRAM/EEPROM publication, capture, rollback and native/Web progress.
    Cover OOM, open/unlinked quota debt, full and partial restore failures,
    uninstall/core-producer lifetimes and real 1536-MiB job peak behavior.
    Other core routes, helper caches and physical accounting remain independent
    requirements even if the MD route passes.

18. Execute `qa/ps1-memory/ACCEPTANCE.md` with the exact native3 PS1 artifacts.
    Observe actual FrontIO read and sync/async/barrier writes, 128-KiB card
    preservation, old-job drain before swap, no silent blank-card recovery,
    read-error unload suppression, complete SRAM/files/disc recovery and denied
    OS save/scratch writes. Require receipt/ABI/version/configuration agreement.
    C bridge cases do not replace real core, device, physical-accounting or
    cross-version evidence; every other original requirement stays due.

19. Build/run the actual N64 bounded-pool and adapted-cache cases with exact
    inventoried sources, then real filter/GL/core/worker artifacts. Validate the
    existing 4096 dimension/one-lane configuration, both shared/workspace pairs,
    finite live/transient bytes, staged allocation failure, invalid dimensions/
    lane/slot/borrow scope, unchanged old pointers and busy-retirement refusal.
    Observe true initialization failure, all enabled filter/deposterize modes,
    unretired pool refusal at ABI uninstall and normal successful retirement.
    Pool cases do not establish total guest/GPU/driver/restore peak behavior.

20. With an explicitly identified qualification-only core fixture or injected
    real core fault, refuse the writable-table uninstall after deinit. Observe
    immediate nonzero worker process exit before DLL/static destruction,
    callback-context destruction or successful stop reply. Retain a live
    producer/handle until actual process death and prove the broker still
    waits for real process/job exit before storage reclamation. Exercise normal
    uninstall separately; no failed-uninstall path may claim a committed final
    save or successful stop. Never infer this behavior from source or a mock
    return code alone.

21. With actual memory-file worker/core fixtures, refuse writable-table detach
    after void unload/deinit during save restore. Prove no saved-file tree swap
    or content restart occurs before checked producer retirement. Retain the
    callback context and original archive through the error, then observe the
    existing nonzero actor-exit path if retirement stays unconfirmed. Qualify
    normal detach/reattach, transient reattach refusal, restore rejection and
    rollback for the real MD/PS1/N64 routes. An uninstall mock's return value
    does not prove absence of live producers or actual file retention.

22. Inject actual SDL make-current failure before a core context-destroy
    callback. Verify its registration remains owned and no callback ran; retry
    before core deinit/unload and observe one successful invocation. With an
    explicitly identified C++ qualification fixture, throw after callback entry
    and prove callback replay/new negotiation/reset are rejected. Attempt
    reentrant stop/shutdown during callback release; observe refusal or nonzero
    actor exit before nested core unload. Persistently fail context acquisition
    during shutdown and prove DLL/static/callback-context destruction and stop
    success do not occur before real process/job retirement. Qualify normal
    release, lost driver context, actual core callback side effects, GPU fences,
    final-save retention and broker collection. A synchronous callback return
    alone does not establish successful GPU completion or total cleanup.

23. Inject reset callback reentry and C++ qualification callback exceptions.
    Before callback return, attempt stop, reset, release, present and new
    negotiation; verify no nested core unload or successful reset/present.
    Throw after partial reset initialization and verify readiness remains
    unpublished, callbacks are not replayed and actor shutdown precedes DLL/
    context destruction without a successful stop acknowledgement. Separately
    fail make-current before reset entry, retry while the DLL is live and
    verify exactly one reset invocation and a later normal release. Qualify
    actual N64/other hardware cores, initialization side effects, final saves,
    broker job exit and driver lost-context behavior. Do not infer GPU safety
    or successful partial initialization cleanup from callback return alone.

24. Fill the actual native audio ring, submit partial/zero/maximum native
    batches and compare callback receipts with exact copied stereo frames.
    Inject impossible counter occupancy before subtraction; no buffer writes
    or false full receipt may occur. Exercise N64 pending output greater than
    16384 frames and ring wrap over multiple capped drains, then every affected
    core's handling of partial/zero receipts without hangs or lost ownership.
    Qualify concurrent SDL consumption, pause/resume/reset, audio disabled,
    latency trimming and presentation-helper enqueue failures separately from
    physical playback. Execute `qa/native-audio/ACCEPTANCE.md`: include SDL
    failure after a partial internal queue copy, non-replay/fault clearing,
    device-control failure receipts, no running-state publication on failed
    resume, same-epoch pause/media-clear races, and staging allocation failure
    before atomic presentation admission. Test GL procedure names of lengths 0/127/128 and invalid
    characters with exact accessible buffers, before any dynamic lookup.

25. Execute `qa/native-lifecycle/ACCEPTANCE.md` with the exact common worker and
    qualification DLLs, then real selected cores and the owning broker/job.
    Exercise checked pause success/refusal/invalid result/exception, same-core
    retry refusal after callback entry, pre-entry GL retry, unknown unload/
    deinit and callback reentry. No unconfirmed lifetime may reach DLL/static
    or detach callbacks, save replacement, successful resume/stop or exit 0.
    Verify numeric catalog/build/hello/prepare agreement, void-only export
    rejection and configuration identity. Preserve the N64 failed-stopped
    peripheral recovery route. Refuse self-termination through explicit
    qualification injection and observe retained ownership until actual broker
    process/job retirement. Dummy-driver fixtures do not prove real core,
    AppContainer, device, final-save or physical-accounting behavior.

26. Execute the production Rust `channel_tests` and the 19 actual-worker cases
    in `qa/native-lifecycle/wire_cases.py`, then the real broker/core/helper pair.
    Reject malformed/duplicate/deep/unknown/replayed headers before binary
    admission; error replies need empty payload framing before a recoverable
    code is accepted. Exercise wrong identities/status/result/error types,
    maximum/zero/truncated bodies and allocation/I/O failure at each boundary.
    Block the current command while pipelining another maximum load: retained
    live input buffers must not include a second speculative save body. Measure
    actual private/working-set/commit memory and preserve prior committed saves.
    Cancel/timeout at queue, partial write/read and late-reply boundaries; no
    future command may enter an invalidated channel, even when process
    termination is refused. Observe actual reader/writer threads and owned
    process/job retirement. Throw inside serialize/unserialize with borrowed
    host buffers and refuse self-termination: those host frames must remain
    parked until the actor dies, without an error/success reply from unwinding.
    Unit cursors/process counters do not establish real pipes, no-op outcomes,
    thread termination, hardware playback or physical resource bounds.

27. Execute `qa/native-lifecycle/CONTROL_WORKERS.md` using the exact broker,
    SDK and native worker artifacts. Fill reservation/active/retiring capacity,
    block actual pipe I/O and hold TLS cleanup after body return. No channel
    destruction may join a running I/O worker, and no capacity refund may occur
    before held Windows thread completion plus join. Inject spawn/query/owner
    transfer failure and race reservations/collection/close. Exhaustion must
    reject Windows preparation before new work/debt/container/process staging.
    Observe real thread/handle counts, memory and responsiveness separately
    from slot/stack declarations. Keep actor/helper/process/job/container and
    work-retirement proofs independent; final-save and physical-accounting
    requirements remain due even after IPC worker retirement succeeds.
    The independent recovery pool must retain its one process-wide worker
    across broker close/replacement, terminal-result publication and delayed
    TLS teardown. Status must not join it; no second collector may start until
    the original held thread object ended and its owner was joined.

28. Prepare an actual protocol-zero package declaring no filesystem save or
    scratch-system route. Require its suspended actor's private scratch to
    remain readonly, including create/replace/stream/write attempts. In-game
    progress stays compatible; opaque state identity includes scratch policy 1
    with no automatic conversion from the older policy. Exercise repeated
    prepare/failure/retry without a broader grant. Separately admit explicit
    legacy disk routes and protocol-one memory routes; their existing save/
    scratch permissions and state identities must not accidentally change.
    Exercise real cores that previously wrote undeclared cache/temp files:
    document failure/adaptation and do not count source flags as compatibility
    or OS access proof. Complete remaining legacy writer quotas separately.

Record target OS/build, toolchain/SDK, source/artifact hashes, exact failure
injection point, observed process/job/profile state, disk/ledger/proof state,
expected outcome and evidence location for every case. No item-2 completion is
established until the complete source and real matrix satisfy the product scope.
