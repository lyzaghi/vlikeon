# N64 memory I/O final qualification

Authored source only. Configure/build/run after all product source authoring,
as part of item 16. Record exact source/SDK/toolchain/ABI/dependency/core/worker/
package hashes. No case below is passed by a source inventory.

1. Freeze `tools/prepare_n64_memory_io.py --project <fresh workspace out path>`.
   In the final verification only, configure `source/qa/n64-memory`, select
   OMNIPLAY_N64_SOURCE as that snapshot's source root and the matching
   OMNIPLAY_NATIVE_SOURCE as source/engines/native. Also explicitly select
   OMNIPLAY_N64_TEXTURE_SOURCE as the exact full integrated pinned texture
   candidate's source directory. A codec-only snapshot is insufficient.
   Build and run
   n64-memory-io-contract with sanitizer configurations where supported. The
   adapter target must compile as C++11, the real store as C++20 and the actual
   inventoried vendor inflate/deflate source as C. Missing input is a failure.
   On Windows also build/run n64-owned-fiber-contract, with the inventoried
   libco headers and actual C adapter. Observe ordinary-thread/root/guest
   identities, refused foreign-root adoption, independent floating state,
   one-guest admission, repeated root retirement and simultaneous thread-local
   pairs. Inject conversion/creation/retirement failures in the real contained
   core matrix; the standalone successful fixture is not full lifecycle proof.
2. Observe config formatting, lines/EOF, all admitted modes, sparse zeros,
   append-after-seek, input lookahead/tellg, binary NULs, rejected paths/modes,
   no uninstalled OS fallback and invalid-table preservation. Verify that
   flush does not publish a new name and old open readers retain old bytes.
3. Inject short writes, copy/read failure, format allocation failure, quota,
   flush/close/rename/remove faults and exclusive-name collisions. Prior complete
   target bytes must survive. No collision cleanup may delete an unowned node;
   retained handles/temporaries continue paying their actual store quota.
4. Exhaust the actual shared scratch page/handle/node limits and all 64 stream
   slots, including update/append private-copy peaks and unlinked open readers.
   Release capacity, retry and compare exact bytes/counters. Exercise distinct
   concurrent publishers and race table replacement against last close. An
   uncertain close must block reuse/uninstall; run that terminal case last in
   its process and prove process retirement releases its debt.
5. Observe caller and low-level failures through C++ close/destructors. A clear
   of ostream flags after a low-level short write must not clear the underlying
   writer's sticky fault. Exercise failed open, repeated open/close, positioning,
   exceptions-enabled streams and EOF without silent publication.
6. Read independent canonical gzip bytes and decode adapter output through
   independent direct inflate calls. Exercise levels 0/1/9, empty and multi-
   buffer payloads, short reads/writes, CRC/size/header corruption, truncation,
   trailing bytes and concatenated members. A negative read is not clean EOF.
   Exercise forward seek, rejected backward/overflow seek, early close footer
   validation, exactly 256-MiB decoded input and independent over-limit input.
   Decoder draining remains bounded without admitting an extra decoded byte.
7. Once core integration is authored, build exact native N64 core/worker/broker
   artifacts. Reject stale versions/receipts, missing ABI, install failure,
   false prepare acknowledgement and unretired producers. Observe actual
   Windows token/job/private-storage isolation, successful bounded memory I/O
   and denied OS save/scratch writes for the admitted final route.
   The separate configuration/shader source preparer currently produces an
   unbuilt gl2 candidate and no package admission. Compare all 256 reloaded
   ROM-database index slots, entry/string/ref/cheat OOM cleanup and failed
   read/close refusal. Optional settings must parse only completely closed text,
   retain prior Config on faults and retain every builtin line across reloads.
   Verify every shader truncation/count/string/version/options/duplicate/length
   and aggregate bound before GL calls, key-format 4/5 compatibility, GL object
   cleanup, fallback compilation and two-slot cycle-mode behavior. No cache
   parser result replaces real graphics/driver or guest/SRAM/state evidence.
8. Exercise generated ROM database/custom settings and all shader/texture/file-
   storage caches, malformed counts/strings/offsets, texture expansion budgets,
   optional assets/dumps and cache expiry. Preserve the configured features
   through normal use and quota/fault recovery. Filesystem/std::fstream/gz OS
   symbols in the standalone bridge are not proof that the compiled entire
   core and driver/helper closure have no writable OS path.
9. Qualify original cartridge/region/mempak/EEPROM/flash/transfer/media features,
   progress and snapshot compatibility, restarts/restore rollback, graphics/
   audio/input, real memory/pagefile/frametime and licensed release closure.
   No ROM, firmware, binary or gameplay evidence is supplied by this snapshot.

10. Build/run n64-actual-texture-cache-contract against the actual prepared
    TxCache.cpp, memory adapters/store and pinned zlib. Record that graphics
    enum values are fixture services; pool/TxMemBuf are now actual code. Cover all raw/
    compressed memory/file cache modes, same-checksum format variants, exact
    LRU ownership, failed-add preservation, reload and late index publication.
    Inject short writes/rename/read/reopen faults while live snapshots remain
    open. A failed reopen after successful publication is not a rolled-back
    file. Retry must retain every previous/pending logical record.
11. Run independent modern/legacy gzip and file-index fixtures, all truncated
    prefixes and malformed dimensions/upload format/length/count/offset/index
    format/overlap/trailing bytes. Bound nested expansion, aggregate expanded
    payload and outer decoded image; validate complete CRC/input consumption.
    Rejected input must preserve the caller's old image/index. Exercise exact
    64-MiB file/256-MiB residency/page limits, allocation failure at each staged
    node/list/map/swap boundary and read-only snapshot length after replacement.
12. In real core/OS qualification, cover source pool max/thread allocations,
    compression working buffers, transient old/new/import/restore peaks,
    physical/pagefile/driver usage and configured cache-limit behavior. The
    zero/unlimited hires source mode now has a finite 256-MiB ceiling and must
    report inability to admit further textures. No silent partial hires pack
    success, disabled feature or unbounded pool may substitute for this scope.
    Confirm readonly texture packs/fonts/media still load while native mutable
    cache names stay under scratch; qualify clear/reload/restore/shutdown and
    all remaining dump/peripheral/state writers before package selection.

13. Build/run n64-bounded-texture-pool-contract with the actual C++11 pool and
    TxMemBuf implementation. Observe both shared/workspace pairs, exact pointer
    retention after injected first/second init/growth failure, checked zero/
    negative/overflow dimensions, re-init refusal, lane/slot/count/scope bounds,
    owner/foreign thread behavior, 16-level nesting, retirement while borrowed,
    subsequent successful release/re-init and the real 256-MiB live ceiling.
    Measure the 384-MiB old/new growth bound independently with the qualified
    allocator/OS; a logical counter alone does not prove physical allocation.
14. Exercise every configured real filter, standalone deposterize, chained
    enhancement/smooth/deposterize, both pixel formats and min/max source/output
    dimensions against independent pixel fixtures. OOM must report failure;
    invalid scale cannot write beyond the actual shared pool. Verify init
    refusal after any partial constructor failure and busy shutdown retaining
    its actual filter object. Confirm normal table uninstall requires the real
    pool to be retired. Run Windows acceptance case 20 for actual refused-
    uninstall worker process exit before DLL/callback/static destruction. No
    pool unit case can prove that process/guest/core lifetime boundary.

15. Build/run n64-memory-png-contract against the actual C encoder, scratch
    bridge/store and pinned libpng/zlib. Decode output independently through
    the read-memory API and compare RGB/RGBA/alpha, padded rows and vertical
    flip fixtures. Verify complete PNG data appears only after final close.
    Inject wrong dimensions/extent/stride/channels/flip, lower codec budgets,
    short writes, flush/rename/close failure and actual scratch quota exhaustion.
    Repeat allocation failures under sanitizers/leak accounting and prove every
    encoder buffer/temporary is retired; test uncertain close last and retain
    its real handle/context until actor process retirement. Codec counters do
    not substitute for actual peak allocation evidence.
16. Build/run n64-bounded-diagnostic-contract with actual prepared native log
    and TxDbg.cpp sources. Messages containing percent sequences must be data
    for the literal "%s" frontend callback. Cover severity, null callback,
    oversized messages, wide/UTF-8 scratch payloads, 1-MiB full-log preservation,
    no retained streams and uninstalled-table behavior. In the real core, check
    both debug-disabled/default and expressly enabled diagnostic profiles,
    actual texture dump CRC/palette names, quantization, pool bounds and PNG
    pixels across faults. Inspect the real linked write closure and prove
    denied OS scratch writes; unselected upstream screenshot code is not a
    completed product screenshot feature or a runtime containment proof.

17. Build/run n64-persistent-file-storage-contract against the actual prepared
    file_storage.c/util.c, persistent adapter and real store. Cover separated
    save/scratch namespaces, path rejection, exact-size and faulted reads with
    unchanged caller bytes/pointer/size, short writes, flush/rename failure and
    update-copy read failure without truncating fallback. Observe first-access
    and later failed-chunk state, full dirty-image retry, invalid parent/child
    ranges, actual 272-MiB save quota, independent scratch and release/retry.
    Run unknown read-close last: outputs and old data remain, the real handle
    debt persists, and table/context reuse is refused until actor retirement.
18. In the actual core, qualify Transfer Pak's content/port filename across
    cart swaps/reloads, readonly legacy RAM import, retained-save precedence,
    missing/bad-size/read/close/allocation/publication failure and guest/no-RAM
    behavior. Preserve original primary SRAM/EEPROM/flash/mempak/snapshot wire
    contracts. Complete and qualify DD save identity/import/format and guest
    producer/flush/capture acknowledgement before package selection. Internal
    write_failed flags or these fixture cases cannot prove frontend save
    success or whole-core write closure. Confirm legacy files are never OS
    write destinations in the memory mode and import errors retain their bytes.

19. Build/run n64-dd-storage-format-contract using actual prepared disk.c,
    DD loader, persistent/file-storage/utilities, pinned MD5 and actual store.
    Qualify independent synthetic D64 bytes, every short prefix, invalid
    type/defect/IPL/LBA/RAM/extent, actual expansion allocation failure and new
    output length. Check MAME/SDK repeated data, physical backup blocks 8/10,
    development sectors and SDK RAM tail. Compare independent first/reverse-
    head goldens, all LBA roundtrips and staged table preservation on invalid
    metadata. Bound actual sector request bytes/coordinates/extent and RAM
    intersections. Record that only parser calloc is a fault fixture service;
    source/legacy path inputs in this suite are memory fixtures.
20. Cover actual full/RAM/readonly DD memory routes, original-source retention,
    legacy import, retained-save precedence, bad retained full/RAM rejection
    without fallback, memory commit failure, full dirty retry, failed close
    retaining its descriptor/ownership, successful retry and unstarted release
    without a manufactured default save. Test two old/new descriptors, duplicate
    mutable destinations, third-descriptor refusal and real handle retirement.
    In the actual core/OS matrix qualify all seven types, both heads, defect
    tables, full/partial/development images, readonly OS import and sharing/
    close/OOM faults, region/selected-system behavior, actual IPL size/signature/
    endian/path/callback cases and controller invalid/oversized byte requests.
    Verify failed close blocks deinit/uninstall and forces the real worker's
    confirmed actor-exit route before DLL/context destruction. Repeated deinit
    must not run the already finished upstream shutdown a second time. Complete producer/
    pause/flush/capture acknowledgement and exact package admission; a DD helper
    return or retained descriptor is not a frontend/durable save receipt.

21. Build/run n64-owner-peripheral-capture-contract with the actual C++11 owner,
    shared GB storage layout, prepared main-owner helpers, real persistent
    storage/bridge/store and DD retirement. Cover foreign OS thread and wrong
    root/guest tokens, early preparation/run, bounded snapshot borrow, malformed
    return retaining an in-flight operation, rejection/retry and guest end
    separate from producer retirement. Observe actual full RAM publications,
    rename failure with unchanged old image, retry, retained failed-close RAM,
    invalid initial read reaching admission failure and no default-data cleanup
    write. Keep a real readonly scratch stream through capture; distinguish
    settled writers from all-stream uninstall. Record opaque-token scope.
22. On actual Windows build/run n64-owner-actual-windows-fiber-contract with
    the real prepared fiber adapter, owner and store. Establish a checked first
    boundary without synthetic guest execution, snapshot memory publication
    without another synthetic step, a later run/boundary and guest/root/table
    retirement. Record that this is real OS primitive evidence with a synthetic
    guest, not full N64 CPU, core, renderer or broker evidence.
23. With the actual core/worker, require completed device/PC initialization and
    no guest instruction before finish-prepare. Qualify pure/cached interpreter
    initial, frame, interrupt-safe and repeated save/load checkpoints; snapshots
    may not execute another frame or release a borrowed buffer early. Cover bad
    magic/version/ROM identity/truncation/state metadata/OOM, then a valid retry
    and rollback; bad magic must not leave the snapshot mutex locked. Verify
    explicit init/destroy/failure/reinit and rejected asynchronous profile.
    Check runtime options cannot enable an unowned renderer thread. Preserve
    primary raw SRAM/mempak/EEPROM/flash and all original M64P save contracts.
24. Exercise real Transfer Pak cart swaps, failed dirty close, rejected imports,
    DD failure after stack unwind, retry, startup/partial-load/stop and repeated
    deinit. Verify actual aggregate capture receipt only after every active
    peripheral succeeds while paused. Inject an unexpected yield with a
    borrowed snapshot and refused mutex retirement in an expressly identified
    qualification actor: observe process exit before buffer/DLL/context release.
    With actual MD/PS1/N64 worker restore, refuse detach after void unload/deinit:
    no archive replacement/restart may occur, and the original files/context
    remain until confirmed actor retirement. Qualify normal detach/reattach,
    failed reattach, rollback and denied OS writes independently of these unit
    sources. Every original core/package/device/driver/license gate remains due.

25. Run the authored snapshot bounds target against the exact integrated
    candidate: known 1.0–1.9 wire versions, every Transfer Pak presence layout,
    unaligned/short queue records, absent sentinel, invalid event type, pool
    exhaustion, duplicate ordinary/SPECIAL events, poisoned/cyclic/foreign
    live links, exact/short serialization, initialized padding and unchanged
    old queue after rejection. The fixture uses actual pinned scheduler/pool
    bodies; it does not prove CPU or whole savestate behavior.
26. In the actual native core, repeat these malformed queue/PIF/format inputs
    and valid retries before/after guest frames. Observe rejection before any
    guest-state mutation or callback, and no output publication after refused
    capture. Cover PIF offsets 0/62/63/64/127 and negative bytes, exact Tx/Rx
    extents, overflowing payloads, old PIF special codes, reset dummy buffers,
    malformed guest-issued command streams and retained-pointer changes.
    Exercise small no-RAM D64, full D64, SDK/MAME backup ID blocks and invalid
    active ID extents; valid snapshot IDs must come from actual admitted
    offsets without reading outside storage. Bound non-NUL GB diagnostics.
    The queue/pointer preflight is not full semantic state validation: audit
    and qualify TLB ranges, all restored bank selectors, DMA/timers, DD zone,
    framebuffer/texture addresses, flash modes and resource ceilings separately.

27. Run the authored exact prepared TLB power-on/map/unmap and wire cases:
    every 12-bit PageMask pattern, reset entries/padding, hardware-width and
    Boolean/cache/region limits, modular six-field derivation, first/last
    read/write LUT slots, exact maximum physical mapping and forbidden lookup
    aliases. Cover the final virtual page, maximum half-span, wrapped/reversed
    halves, unchanged adjacent pages, KSEG/physical exclusions, independent
    read/write overlap history and invalid map/unmap indices. Then qualify the
    actual pure/cached interpreter TLBWI/TLBWR/TLBR/TLBP instructions, including
    WIRED>=32 and RANDOM results 32..63: no entry dereference outside 32 slots.
    Verify normal guest PC/register behavior and maximum-span cache invalidation.
    In the actual full loader, malformed late TLB/LUT data must reject before
    any live-state mutation or callback; qualify valid legacy state roundtrips,
    legitimate nonstandard masks, retry/rollback and ordinary game behavior.
    Source fixtures do not establish instruction, CPU, driver or game execution.

28. Run the authored actual prepared GB/Transfer Pak/file-storage/store cases:
    nonrumble MBC5 bank 15, rumble MBC5 bank 7 plus rumble action, camera high
    banks, independent persisted offsets and unchanged low banks, ROM bank 511,
    missing/out-of-range ROM/RAM data, zero/oversized/cross-window packets and
    disabled/absent/bad-bank Transfer Paks. Qualify complete bounded camera
    output/failure images, undersized RAM headers before open and wrong initial
    RAM descriptors discarded without creating a default save. Exercise both
    1.2/1.3+ selector wire layouts, first/last ports, truncated optional payloads,
    forbidden Boolean/bank/reset/latch values, valid raw RTC/time data and valid
    camera registers. In the actual core, late-port malformed selectors must
    reject before guest mutation/callbacks, then allow valid retry/rollback.
    Qualify real controller/camera/rumble, cart swap/dirty close/capture/restore,
    high-bank game saves and ordinary compatibility. The synthetic backend and
    no-op diagnostic fixture does not prove devices or full loader behavior.
    Complete RTC arithmetic/halt/carry and remaining upstream cartridge stubs
    and compatibility as separate source/product obligations.

29. Run the authored actual MBC3 RTC/synthetic-clock cases: current-time anchor,
    second/minute/hour/day boundaries, day 255/256/511/512, sticky carry and
    explicit clear, running-to-halt and halt-to-running writes, backward jumps,
    signed timestamp extremes, invalid raw-byte counter prefixes, frozen
    reads/live writes/exact latch edges, invalid registers and foreign-thread
    denial before clock callbacks. Qualify raw-byte behavior against hardware.
    In the real gl3 core, capture/restore 1.10 with latched_valid independent of
    the last full command, including command zero after capture, all 16 cart
    presence combinations, wrong/absent carts, halted time and malformed flags
    on the last port. Reject before guest mutation/PIF callbacks, preserve output
    on failed flag staging, and permit valid retry. Qualify all 1.0–1.9 imports,
    exact fixed-size wire offsets, actual emitted version/identity, rollback
    and wrong-core/package admission; no old gl2 candidate can stand in for
    gl3. RTC battery persistence, subsecond/device timing and real clock/game
    compatibility remain separate completion/qualification obligations. A unit
    fixture or format declaration cannot establish full-core save compatibility.

30. Run the authored actual RTC/battery/main-helper/file-storage/store cases:
    exact 32-byte CRC wire images with an independent pinned-zlib oracle,
    signed timestamp extremes/raw halted fields, RTC-only no-RAM cartridges,
    staged boot before mutation, offline active/halted intervals, repeated
    power-on and volatile latch reset. Cover corrupt magic/length/reserved/CRC,
    read errors, unstarted discard without default writes, bounded canonical
    content/physical-port names, four-owner/duplicate limits and foreign denial.
    Fail RTC publication before RAM retirement; fail RAM close after RTC flush;
    preserve both ownership and retry state, invalidate an old RTC receipt on
    subsequent clock/snapshot changes, withhold aggregate capture receipt and
    then retire cleanly on retry. In actual main.c and the real worker, qualify
    both initial and cart-swap bind sites, wrong/bad sidecars before first guest
    opcode, callback uninstall refusal, repeated save/stop/start, archived-tree
    restore, cross-ROM/port/profile isolation, unknown-close debt, every partial
    multi-file flush, process termination/crash/recovery and actual clock/game
    behavior. Confirm that `.rtc` is never written to the host filesystem and
    a per-file publication is never reported as an atomic multi-file commit.
    The source-only fixture does not establish real session persistence.

31. Run the authored actual flash/SI/C2/wire cases: flash last page/sector,
    guard bytes and exact save-callback ranges, complete chip erase, rejected
    page/sector arithmetic, missing/short storage, source/destination DMA ends,
    page-buffer staging, old silicon address doubling, ID alignment and MMIO
    output initialization. Cover complete SI 64-byte blocks/conversion, rejected
    launch before PIF/scheduler callbacks, exact last block and address changes
    during pending reads with bounded error/interrupt completion. Exercise C2
    sector underflow/last sector/zone limits and foreign device-copy denial.
    Qualify known SI/DP/SP/flash selectors in all snapshot layouts, inactive DD
    padding, current zero-ID DD context and an active PIF packet without an
    opcode. In the actual CPU/core/worker, prove rejection before mutation or
    callbacks, ordinary/legacy save and valid game compatibility, retry, actual
    PI/framebuffer/scheduler interactions, maximum SP transfer counts and all
    byte-order/4 MiB/8 MiB/device boundaries. Hardware-invalid commands/addresses,
    timing and real SI/PIF/MI behavior require qualification; fixture stubs are
    neither CPU execution nor hardware compatibility evidence.

32. Run the authored exact PI launch/actual framebuffer cases. Cover length
    rounding before admission, short writes and zero effective length, exact
    final 4/8-MiB byte ranges, invalid complete spans before any dispatch or
    graphics callback, missing handler directions and busy requests. Check
    dirty-page readback ordering across DMA and framebuffer boundaries,
    descriptor product overflow, canonical aliases versus unrelated segments,
    page 2047, compacted valid descriptors, paired mapping restoration,
    CPU-wrapper final words and callback intersection/step phase. Check all
    six graphics cache slots plus a seventh identity without OOB or lost
    required copy/write, identity churn, depth/color coherence, repeated reads,
    16/32-bit pixel conversions and original valid game behavior in actual GL.
    The authored fixture has observing substitutes for scheduler/raw memory/
    mapper/graphics/cartridge; it cannot prove actual CPU/device/GL behavior.
    Qualify direct SRAM/ROM/DD entry points and zero-length cached-code
    invalidations, pending-address growth, pixel-copy geometry, GPU budgets
    and full loader/worker/device fault behavior after source closure. Parent
    PI callbacks do not establish transferred-byte or persistence receipts.

33. Run the authored actual SRAM/ROM/DD/domain DMA cases. Cover both sides of
    every transfer, exact final 4/8-MiB RAM bytes, authentic base versus slice/
    null, undersized storage and missing write callbacks before mutation,
    last/unaligned/partial MMIO words and upper-half SRAM addresses. Prove that
    subword swapped SRAM writes publish every physically changed byte through
    actual file-storage incremental patches, first publication, failed patch,
    full retry, capture and restoration; compare raw legacy SRAM files byte for
    byte. Qualify widened ROM offsets, EOF padding, partial source views,
    busy-write latch, exact KSEG invalidation and zero DMA without global cache
    flush. Exercise full DS/C2 buffers, short/absent/oversized IPL, unknown and
    MS requests, source XOR bounds, wide cycle arithmetic and owner denial
    before automatic save-type selection. Fixture callback substitutes do not
    prove CPU/PI scheduling, actual cache invalidation, sequencer behavior or
    hardware-invalid request semantics. Complete ROM image admission/conversion
    and metadata qualification, real DS/C2/disk/interrupt behavior, valid game
    timing and the existing whole-core/worker/OS/fault matrix remain required.

34. Run authored fixed dirty-byte and actual production-converter cases.
    Qualify duplicate/max-RAM coverage, cross-word masks, partial consumption,
    other-frame retention, subpixel and multi-pixel writes, complete CPU source/
    destination bounds, odd halfword ends, exact 4/8-MiB geometry, color-sum wrap
    and opaque full-alpha black. In actual renderer/GL, exercise seven or more
    framebuffer identities, overlaps, buffer removal/replacement between writes
    and reads, 16/32-bit and float conversion, changing dimensions, driver limits,
    allocation failure/retry and initialization/shutdown across content changes.
    Prove visual/alpha/depth coherence against valid games and baseline images.
    Audit all CPU/GPU accounting, driver errors and context loss: void submission
    does not establish GPU completion, and the pure CPU fixture does not execute
    the actual renderer methods or qualify their lifecycle and failure receipts.

35. Run authored ROM image admission/conversion cases. In actual open_rom and
    frontend, qualify all three input formats, unaligned input, minimum boot
    extent, truncated/partial words, unknown signature and profile-size limit
    before header/boot reads and allocation. Compare normalized bytes and MD5
    against independently prepared images, check 20-byte unterminated names,
    metadata/database behavior, boot HLE and standalone DD. Exercise allocation
    failure, content replacement and owner denial without losing previous
    content/save ownership. The helper fixture cannot qualify the actual
    frontend initialization order, database or emulated boot/game execution.

36. Run authored frontend content helper receipt cases, then the actual frontend
    fault matrix: invalid ROM/path before saved-memory/audio/graphics setup,
    every companion allocation failure, ROM_OPEN failure, partial GET_HEADER
    output/failure, refused ROM_CLOSE and teardown retry. Prove no pending ROM
    ownership can be overwritten, invalid-state close is accepted only without
    a receipt, input/path allocations retire after confirmed owner teardown,
    and failed setup is not replayed within a poisoned session. Exercise the
    real header/global metadata semantics, subsystem loads, audio/context setup
    and callbacks, content replacement and final save capture. The helper's
    CoreDoCommand observers do not prove actual core, frontend or OS teardown.

37. Run non-Apple actual audio lifecycle body cases with allocation/resampler
    faults at every step. Qualify real sinc creation/free, all actual allocator
    budgets, SIMD initialization, repeated init/deinit, foreign calls and
    frontend failure propagation before graphics setup. Complete AI RAM extent,
    immutable source/channel order, resampling ratio/output-capacity admission,
    zero/overreported/partial batch consumption and backpressure source before
    product enablement; test actual sample rates, device loss, teardown and
    save/crash ownership. Qualify Apple converter lifecycle separately. The
    lifecycle-only substitutes do not execute the sinc backend or any device.

38. Run authored output-ring cases for zero, partial, slow and overreported
    receipts, exact stereo order, wrap, full queue and callback reentrancy.
    In actual root/guest execution prove a stalled consumer prevents further
    guest advancement while input/UI/video remains usable; qualify resume,
    device loss, changing callback registration, queue exhaustion as a session
    error, final save and explicit playback cancellation on stop. Prove no
    borrowed callback storage is freed, and qualify snapshot/load/reset audio
    discontinuities. Complete real resampler output/ratio and AI/source bounds
    before product enablement: this consumer suite does not prove producer
    memory safety or that queued frames reached a physical audio device.

39. Run expanded lifecycle/output-adapter cases where the real queue consumer
    attempts direct audio deinit. Prove its samples/buffers remain live and no
    retired receipt is issued until the borrow returns and later cleanup
    succeeds. In actual root exports attempt run, reset, serialize/unserialize,
    prepare, flush, unload, deinit, callback replacement and special content
    load from an audio callback. Verify no core command, guest switch, path
    cleanup, CoreShutdown or table detach happens inside the borrow; retry
    refused void requests afterward and qualify host behavior. Exercise real
    context reset/destroy, resampler/driver callbacks, owner/end/table receipts
    and worker/DLL lifetime. Local helper source cannot prove whole-core or
    asynchronous driver callback retirement.

40. Run authored actual AI publication and backend immutable-copy cases. In
    real LEN reads and DMA completion qualify last_read/remaining relationships,
    full active FIFO spans, 4/8-MiB endpoints, alignment and malformed snapshot
    addresses before pointer creation. Compare original RAM and AI registers
    before/after repeated partial sample callbacks; verify no channel swapping
    mutates the game image. Inject private-copy allocation failure, invalid
    pointers and nested/foreign callbacks. Complete duration arithmetic, FIFO
    admission/delayed carry, resampler ratio/output capacity and actual timing/
    audio compatibility; the helper processor substitute does not execute the
    AI scheduler, real converter, resampler or device. Measure transient copy
    memory/latency within the actual actor budget.

41. Run authored exact AI request/DMA cases for admitted timing truncation,
    zero/negative/NaN/infinite modifiers, zero rates, DAC divisor and VI product
    overflow, signed CP0 horizon, complete RAM before FIFO publication and
    delayed-carry bounds before backend/scheduling. In actual core qualify
    remaining-event consistency, FIFO full/pop/failed-start status, zero
    duration/length, snapshot timing/selector admission, admission-failure stop
    and final save/retirement. Compare real interrupt/sample timing with valid
    games and baseline behavior. Observer methods do not execute CP0, FIFO pop,
    remaining-length calculation or the CPU; full resampler/device and original
    product qualification remain required.

42. Run authored sinc phase-planner oracle cases. Then qualify actual pinned
    C, SSE, AVX and available NEON processors against predicted output and
    independent reference samples with guard bytes, startup/steady phases,
    short/maximum input, fixed-point limits and all admitted changing rates.
    Compare output/filter state across automatic chunk subdivision, test exact
    capacity refusal without processor/state mutation and one-frame expansion.
    Exercise actual staging allocation, selected backend identity, filter
    table/tap/allocator bounds and low-rate frontend queue exhaustion. Qualify
    Apple converter/accumulation separately; planner-only tests do not execute
    any sinc processor, sample conversion or device.

43. Build and run the authored actual pinned-sinc/allocator target only in the
    consolidated final pass. Compare admitted direct/checked samples and phase
    counts across qualities/rate changes, output guards, refusal without state
    mutation, retry and mixed-quality per-instance dispatch. Qualify invalid
    bandwidth/quality/tap expansion, exact allocation budgets/failures and real
    C/SSE/AVX/NEON capabilities on the target CPU; never infer ISA acceptance
    from compilation alone. Compare independent reference audio and actual
    caller subdivision, device/queue behavior and whole-actor memory/latency.
    Authored targets and direct-path comparisons are not executed evidence or
    an independent proof of filter quality and all SIMD memory accesses.

44. Run expanded production AI push/pop/start cases, then actual full-core
    failure injection for invalid first-slot carry, pending promotion, backend
    format rejection, queued/full-slot replacement and successful promotion.
    Verify descriptors/status remain unchanged on refused start and the CPU
    reports/stops/retains final saves without claiming a scheduled DMA. Audit
    void scheduler insertion and node allocation, completion interrupt after
    source failure, last_read debt, zero-event/full-FIFO/snapshot behavior and
    actual game timing. The observer fixture does not prove event insertion,
    CPU failure-stop or complete callback side-effect rollback.

45. Run exact scheduler/pool receipt and expanded AI refusal cases, then actual
    event exhaustion at AI start/promotion. Check all slots, count wrap, zero
    delay, duplicate type behavior and foreign denial; refused insertion must
    not publish a new DMA. Qualify real CPU stop flags in every selected CPU
    mode, interrupt checkpoint failure detection before frame yield, guest
    return, retained saves, final flush/retirement and host failure reporting.
    Audit all remaining void event producers and malformed/cyclic queue state.
    Node insertion receipts do not prove event delivery or successful shutdown;
    source-only fixture accessors do not execute the CPU or Windows worker.

46. Run expanded owner/peripheral failed-guest save-recovery cases. In actual
    worker/CPU/device faults, require confirmed guest end before post-stop flush,
    deny failed paused/in-flight flush, inject publication failure and retry,
    compare RAM/DD/RTC saves and prove successful persistence cannot resume or
    clear failed CPU admission. Cover reentrant flush/end denial, failed dirty
    close retention, worker capture/unload order and all save producers. Protocol
    token/real memory-store fixtures do not execute a guest fiber, establish
    crash durability or prove a corrupted session can continue safely.

47. Run updated output-ring cases with the native 16384-frame offer cap and
    multiple bounded drain passes. In the real worker, saturate its ring and
    prove partial/zero receipts leave unacknowledged core output pending.
    Compare exact stereo order across both rings and wrap points. Qualify
    latency trimming/device/presentation acknowledgement separately, as well
    as partial-batch behavior for other cores sharing the native callback.
    Source queue cases do not exercise concurrent SDL/device consumers.

Core integration, package admission, helper/driver caches, legacy recovery,
physical whole-profile accounting and every original item 1–16 requirement
remain open. This bridge suite cannot stand in for their actual verification.
