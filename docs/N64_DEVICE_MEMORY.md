# Native N64 device memory source

The current gl3/1.10 candidate adds flash, SI and DD C2 memory admission and
known snapshot device selectors. It is source-only, unbuilt, unqualified and
not enabled. Existing primary raw flash/save formats and snapshot buffer size
remain unchanged. This document is not a whole-core memory-safety claim.

Flash chip/sector/program commands admit the entire destination against the
128 KiB logical array and actual backend size before mutation or a save
callback. Out-of-range page/sector commands cannot write beyond the array or
set a new program/erase success result. Valid commands keep their pinned modes,
status transitions and callback ranges. Invalid/unsupported command behavior
and silicon-specific edge cases still require hardware/game qualification;
the source rejects invalid complete ranges instead of inventing address wrap.

Flash DMA validates both complete source and destination extents, the actual
RDRAM base/capacity and byte-swapped buffer assumptions before copying. Old
silicon address doubling, exact page-buffer lengths, silicon-ID word alignment
and the pinned 0x1000 DMA cycle result are retained. A rejected request changes
no page-buffer/RDRAM bytes. Unknown MMIO reads return initialized zero. The
raw formatting API still requires its trusted caller's full FLASHRAM_SIZE
buffer. Primary save bytes remain in the existing raw save contract; an
emulated program-success bit is not external persistence acknowledgement.

SI validates a complete aligned 64-byte low-24-bit RDRAM extent before launching
either DMA direction or invoking PIF processing. It rechecks at deferred copy,
since a guest can change the address register while a read is pending. Failed
copies set DMA error without partial bytes; the existing end-event interrupt
and transfer retirement still occur. This preserves raw address-register states
in snapshots and handles them at use instead of banning every unused address.
The real scheduler/PIF/controller/MI behavior remains integration qualification.

DD C2 admits the entire output before byte-swapped writes. A sector below the
C2 range cannot underflow subtraction, and invalid zones cannot index the
geometry table. The DD host-sector register path also checks its zone before
accessing the size table. Primary disk parsing/geometry/storage admission remains
separate and unchanged.

Snapshot preflight now admits only known SI directions, deferred-DP flag bits,
SP FIFO directions and flash modes/status widths. It checks present DD
host-sector width, write/reset flags and zone. Absent DD padding stays ignored;
because a disk ID can legitimately be zero, actual save/load also supplies the
loaded-disk context for that case. Raw length/address registers and the full
erase-page field remain register data; actual device methods bound accesses.
An active PIF packet must own an opcode byte before any backend reads it, as
well as its complete declared Tx/Rx extent. Legacy skip/reset control bytes
retain their separate handling. The wire has not been extended by these checks.

Authored `peripheral_memory_cases.cpp` selects actual prepared flash methods,
exact SI methods with explicit scheduler/PIF/MI stubs, the actual C2 body and
independent register/wire fixtures. Cases cover last page/sector, guard bytes,
invalid command/DMA extents, old silicon/ID paths, exact final SI blocks,
pending address changes, C2 subtraction, foreign denial and selector layouts
including active zero-ID disks. The stubs do not execute a CPU or prove full
PIF/interrupt/device behavior. None of these cases has been compiled or run.

The current candidate additionally admits the entire adjusted PI RDRAM span
before handler selection, framebuffer callbacks, copy or interrupt scheduling.
Pinned low-24-bit/even addressing, >=127-byte odd-length rounding and short
write adjustment are retained. Missing graph/selected-handler callbacks and
invalid complete spans set PI error without a partial transfer or scheduling.
Zero-effective writes retain handler timing and register updates but produce
no framebuffer notifications. Each intersecting dirty framebuffer page is
read back before a DRAM-to-cartridge transfer, including a framebuffer which
starts after the DMA's first address. The handler ABI still returns cycles,
not a transferred-byte receipt: a parent notification is not proof of a
successful cartridge transfer. The direct SRAM/ROM/DD methods now have their
own complete bounds and zero-length invalidation handling, as described below;
actual full PI/cartridge/device coherence remains qualification work.

Actual core framebuffer descriptors now require physical/KSEG0/KSEG1 RDRAM
addresses, supported byte sizes, nonzero dimensions and complete products
which fit the actual <=8-MiB RAM capacity. Division admits dimensions before
multiplication. Invalid descriptors are omitted; valid descriptors are
compacted and canonicalized before memory mapping and dirty-page indexing.
Address zero keeps the pinned API sentinel meaning. The existing paired
unprotect/display-list/protect lifecycle retains old descriptors during the
display list. CPU wrapper accesses admit a complete aligned word before raw
RAM access. Read/write callbacks independently admit their request and each
descriptor. Writes iterate the actual intersection with the original 1/2/4-
byte step phase; reads consume each intersecting dirty page once. The first
valid mapping still intentionally invalidates all cached CPU code.

GLideN64's six-slot read/write identity cache now searches within its array
and returns an explicit full sentinel. Both stores check the sentinel before
indexing. A full observation cache does not suppress the underlying write or
copy; uncached identities may repeat reads. Actual graphics coherence across
descriptor/identity churn must still be qualified. This does not bound the
whole graphics pipeline or establish actual GPU completion. The separate
pending-address vector and CPU pixel-copy path are adapted below.

The authored `pi_framebuffer_cases.cpp` uses exact prepared PI launch methods
and actual adapted fb.c with real core structs. Explicit observing substitutes
stand in for the scheduler, memory mapper, raw RAM and graphics/cartridge
callbacks. Cases cover adjusted exact/overrun endpoints, 4/8-MiB capacities,
callback ordering across pages, zero-effective writes, missing handlers,
descriptor multiplication/aliases, final dirty-page slot, mapping pairs,
notification step phase, full six-slot lookup and foreign-owner denial. This
does not run an actual CPU, mapper, cartridge, GL context or product, and none
of the cases has been compiled or run. Real full-cache graphics behavior is
an integration acceptance obligation, not established by the helper case.

Direct SRAM DMA now authenticates the RDRAM base/capacity and both complete
logical byte ranges before obtaining storage memory, copying or publishing a
dirty range. The fixed 32-KiB logical array and the actual backend size are
checked independently. The pinned low-16-bit cartridge mask is retained;
upper-half accesses are rejected rather than inventing wrapping into the
lower 32 KiB. Missing write callbacks reject before bytes change. MMIO uses
checked four-byte memcpy and masked values, including unaligned complete
words, instead of unchecked uint32_t pointer dereferences. Unknown/short reads
produce zero. Zero-length DMA performs no dirty notification. Valid transfers
retain the pinned 0x1000 cycle result and raw SRAM file format.

SRAM DMA dirty notifications cover the physically modified bytes. On the
XOR-3 host layout a short logical transfer may modify different byte positions
within its first/last words; the notification covers those complete physical
words, within the logical array. Merely publishing the original logical offset
can miss the actual changed bytes during a later incremental file patch. On
the non-swapped layout the original exact range is used. This changes a dirty
notification range, not the raw primary save format or persistence receipt.

Cartridge ROM DMA uses widened source arithmetic and checks the complete RAM
destination before any copy, cached-code invalidation or random timing call.
The pinned cartridge address mask and logical-EOF zero padding are retained.
A partial non-word-sized backing view never supplies a physical XOR byte
beyond its declared length: an unavailable logical/physical byte supplies
zero. This is a bounded fallback for an inconsistent view, not proof that a
truncated image is a valid game. MMIO admits a complete source word, while
the existing busy-write latch and valid scheduling behavior remain. Zero DMA
retains the timing call but never turns size zero into global invalidation;
nonzero output invalidates exactly its two KSEG aliases. Invalid direct spans
return the pinned fallback 0x1000 without touching RAM or timing callbacks.
Writes to ROM remain ignored. Complete image admission/conversion still needs
its separate audit, including tiny signature/header reads, alignment, minimum
boot image requirements, metadata strings and the signed image-size contract.

64DD DMA now checks the entire RDRAM extent and exact DS/C2/IPL source or DS
destination extent, including XOR byte ordering, before any copy. Existing
supported DS/C2 entry addresses are preserved; unknown buffer addresses and
oversized/absent IPL views cannot read neighboring registers/storage. MMIO
requires a complete IPL word. Valid transfers retain `(length*63)/25` cycles;
the arithmetic is widened and saturates if an out-of-contract direct request
cannot fit unsigned cycles. Zero/invalid copies perform no code invalidation.
The upstream unimplemented MS transfer remains an explicit no-op and requires
feature/hardware qualification; this patch does not implement the sequencer.
Cart domain dispatch also checks the active owner before dereferencing the
cart or changing automatic SRAM/flash selection.

Authored `cartridge_dma_cases.cpp` links actual selected SRAM, ROM and flash
methods, the actual included DD body and exact prepared cart DMA domain
methods. Independent guard bytes, physical dirty-range mirror and logical
byte fixtures cover exact ends, missing callbacks/short backing, bad RAM bases,
partial words, ROM EOF/past-end, physical tail bounds, zero invalidation,
4/8-MiB RAM, DS/C2/IPL limits, wide cycles and foreign dispatch. A small-view
enumeration oracle checks swapped-span admission against every physical byte.
CPU invalidation, scheduler, random timing and PI validation are explicit
observing substitutes, not emulated CPU/device execution. These cases are
authored only and have not been compiled or run. Actual SRAM file-storage
patch/retry/capture behavior and byte-for-byte legacy saves remain integration
obligations; a backend dirty notification is not a durable-save acknowledgement.

RDRAM-to-color now records exact byte coverage in a fixed one-MiB bitmap for
the entire maximum 8-MiB RDRAM view. Repeated writes allocate nothing and do
not discard earlier writes. Both pixels of a four-byte write into a 16-bit
image are selected; subpixel writes select the containing pixel. A
completed image consumes only its own range, preserving other framebuffers'
pending bytes. No framebuffer pointer is retained by addAddress; processing
resolves the requested/current buffer synchronously. The six-slot identity
cache read path additionally checks pending byte coverage, including identities
which could not fit its observation cache. Shutdown/reinit explicitly clears
coverage; a failed allocation or invalid conversion retains it for retry.

CPU conversion checks RAM capacity, base alignment, nonzero geometry, complete
source and output extents before source pointer arithmetic. A partial odd final
halfword with no representable XOR destination rejects without submitting the
private result. The selected full/partial halfword permutations and production
RGBA converters are retained. A boolean nonzero flag replaces the overflowing
color sum; full-alpha all-black images remain valid uploads. The CPU scratch
allocation is limited to 64 MiB and at most 16 MiB temporary conversion storage
by the RAM geometry. Driver-reported maximum texture dimensions and matching
4-byte or 16-byte output formats are checked before resize/upload. Initial
allocation is one pixel, then resized for the admitted image, replacing the
unchecked fixed 640x580 staging assumption. Allocation failure retains prior
storage and pending writes; initialization failure records owner admission
failure. Source cleanup and its pending-range consumption are synchronous.

The graphics API exposes void upload/draw calls: returning from those calls
does not prove GPU completion or successful rendering. Consumption currently
follows submission, with separate actual driver/context-loss/retry qualification
required. No claim is made for whole GPU budgets, texture-cache accounting,
other rendering paths or all overlapping-framebuffer semantics. The bounded
bitmap adds one MiB per renderer singleton, which must be included in actual
actor memory qualification. The CPU fixture `color_memory_cases.cpp` uses the
actual prepared RGBA converters and selected pixel helper with independent
byte coverage/color/guard fixtures. It covers duplicate and full-RAM coverage,
partial consume, word boundaries, writes spanning pixels, invalid geometry,
XOR endpoints, summed-color wrap and opaque black. It does not run the renderer
methods, GL, CPU core or Windows. All cases remain uncompiled and unexecuted.

ROM image source admission now checks a complete 0x1000-byte boot extent,
whole-word length, the existing 512-MiB native profile limit and one of the
three supported signatures before signature/header access or ROM allocation.
The minimum follows the actual boot HLE copy [0x40,0x1000), not a promise that
an image of this length is a playable game. Whole-word admission matches the
later main byte swap and avoids silently leaving a partial last native word.
The profile limit is below the signed g_rom_size/MD5-length boundary. Previously
accepted truncated headers/boot images and non-word tails now reject.

The selected conversion uses byte loads and stages each four-byte group,
supporting unaligned input/output and exact in-place conversion without typed
unaligned integer accesses or strict-aliasing assumptions. Arbitrary partial
overlap is outside the helper contract; actual open_rom uses separate input
and allocated output. Format output commits only after a full copy. The
diagnostic header name uses an explicit 20-byte precision instead of reading
past the fixed field for a terminator. Header bytes and valid normalized
ROM/MD5 content are unchanged. Actual core admission also checks owner access.
The libretro frontend's earlier setup/allocation ordering, all database metadata,
boot modes, standalone DD and full product content lifecycle remain separate
audit/qualification obligations; this helper is not a whole-loader claim.

Authored `rom_image_cases.cpp` covers null/tiny/unknown/boot-truncated inputs,
whole-word boundaries, the size cap, all three orders, exact normalized bytes,
unaligned and exact in-place buffers, output guards and rejected short output
without format publication. It runs the production helper only, not open_rom,
allocation, MD5/database, boot HLE or the libretro frontend. No case has run.

Frontend source now preflights ROM format/extent and a bounded 1024-byte path
before companion discovery, saved-memory formatting, variable updates, audio
or context setup. Each discovered companion allocation is checked before
strcpy. This bounds discovery allocation and avoids null dereferences; it
does not turn optional companion absence/read errors into validated media.
Explicit subsystem paths and all subsystem allocations need separate audit.

The actual load-data helper tracks a ROM-open receipt. Successful ROM_OPEN
owns that receipt even if GET_HEADER fails; the header query writes a staged
local header which is published only on success. The copied input buffer and
its size are released/reset after open or an open failure. Failed header query
attempts ROM_CLOSE. Refused close retains the receipt and marks owner admission
failed, preventing another content load from replacing uncertain ownership.
Deinit can retry and may accept INVALID_STATE only when no open receipt exists.
Successful unload clears the receipt only after its real ROM_CLOSE succeeded.
Final confirmed retirement frees remaining input/path allocations. Failures
after setup starts mark admission failed, so audio/context/saved-memory setup
is not blindly replayed by another load in the same owner session.

This is conservative lifecycle source, not rollback of every frontend side
effect or proof of successful graphics teardown. The real core ROM_OPEN itself
updates global metadata; staged GET_HEADER is not a whole-core metadata
transaction. Audio/context setup and callback failure acknowledgement,
subsystem loads, context loss and save retention need full integration work.
Authored `content_load_cases.cpp` includes the production helper with explicit
CoreDoCommand observers and the real owner guard. It covers open/query/close
failure, exact receipt retention, failed-admission retry denial, staged query
output and foreign-owner rejection. It does not execute frontend setup, actual
CoreDoCommand/boot/graphics/audio or the Windows actor. No case has run.

Selected audio lifecycle source now bounds requested capacity to 1..65536
frames and requires successful resampler creation, callbacks and all buffers
before publishing readiness. Partial allocations are independently freed even
when no resampler handle exists; pointers are nulled and repeated retirement
is idempotent. Live reinitialization rejects without replacing its buffers.
Frontend load checks readiness before continuing context setup. Foreign owner
calls cannot allocate/retire these resources. The Apple branch checks converter
disposal before releasing dependent storage, but native converter creation,
retirement receipt and macOS integration still need their platform audit.

Actual frequency setup now checks owner/readiness, nonzero frequency and the
width of the integer frequency/byte-rate arithmetic before division. The
sample callback rejects absent buffers/output callbacks and incomplete stereo
frames. This is not complete audio admission: AI RAM/pointer bounds, source
mutation/channel order, sinc output bounds and ratio handling, zero/oversized
frontend consumption, backpressure, queued output, real resampler allocation
budgets and device/context teardown remain source and qualification work.
In particular existing batch-consumption loops must still be completed before
this candidate can be enabled. No whole-audio safety/completion claim is made.

Authored non-Apple `audio_lifecycle_cases.cpp` includes the actual lifecycle
body with explicit allocation/resampler substitutes and real owner admission.
It covers each partial allocation failure, a failed resampler returning a
partial handle, valid/max capacity, invalid capacity, live reinit, foreign
retirement and duplicate cleanup. It does not execute the real sinc backend,
SIMD setup, actual audio callbacks, Apple converter or an audio device. Cases
remain uncompiled and unexecuted; void resampler free is not an OS/device
retirement acknowledgement.

Audio output source now replaces both unbounded consumer loops with a fixed
262144-stereo-frame (one-MiB sample payload) ring. Zero consumption preserves
all pending samples and returns promptly; partial consumption advances only
acknowledged frames in exact order. Each drain performs at most eight callback
invocations. The actual root retro_run attempts a drain before another guest
step, so a stalled consumer pauses CPU advancement rather than repeatedly
running more frames into output debt. Wraparound uses two bounded copies.
Queue append is complete-or-refused, and callbacks cannot append/drain while
the queue borrows its sample storage. An overreported count faults the queue
without underflowing count or advancing pointers. Queue faults and exhaustion
mark owner admission failed rather than reporting a successful sample drop.
Existing pending frames remain owned until explicit session retirement; new
samples refused on exhaustion are not retained or reported as accepted. This
void producer ABI still needs full fatal-error/save/worker qualification.

Confirmed audio deinit cancels pending playback explicitly and refuses to free
storage during an active consumer borrow. This is playback cancellation at
session stop, not proof every queued sample reached a physical device. The
real resampler output-capacity/ratio, AI RAM and source mutation bounds remain
unfinished; a safe consumer queue cannot stop a producer from overwriting its
own undersized scratch before submission. No output-bounds claim is inferred
from a post-resampler queue check. Device loss, UI/video responsiveness during
stall, snapshot/load/reset audio discontinuity and callback registration/lifetime
also remain full frontend/worker qualification work. The fixed queue must be
included in whole-actor memory accounting.

Authored `audio_output_cases.cpp` exercises the production ring with scripted
zero/partial/slow/overreported/reentrant consumers, exact ordered stereo bytes,
full queue, wraparound and retained debt. It does not execute actual resampling,
root/guest switching or devices. No case has been compiled or run.

Audio callback retirement now exposes an owner-checked idle predicate and an
independent retired predicate. The retired predicate requires no active borrow,
no ready backend, no pending frames, no output/scratch buffers and no resampler
handle (or Apple converter/input storage). Root admission incorporates the
idle predicate, rejecting reentrant run/prepare/snapshot/flush/unload before a
guest switch or core command. Deinit checks idle before any upstream shutdown;
its final ownership release and table install/uninstall require retired audio.
Reset and audio-batch callback replacement also check idle. Special subsystem
loading cannot clear existing paths while callback storage is borrowed or a
content session is loaded/started. This is not complete subsystem validation.

Reentrant void API requests are refused, not queued or acknowledged as done.
The caller must retry after its callback returns; actual host retry behavior
remains qualification work. Direct backend deinit also refuses an active
borrow and records owner failure without freeing storage or reporting retired.
The expanded lifecycle fixture links the actual lifecycle and output adapter
with a callback attempting direct deinit, checking sample liveness and receipt
state before/after return. Root exports, actual resampler free, CoreShutdown,
context reset/destroy callbacks, frontend callback registration/lifetime and
the Windows process are not exercised by that fixture. Concurrent unexpected
driver/context callbacks, whole-core callback reentrancy, producer memory
bounds and all prior audio/GPU/device obligations remain open and unverified.

AI sample publication now admits the complete active FIFO RAM extent and the
last-read/remaining relationship before forming a RAM slice pointer. Whole
stereo-frame counts and actual RAM capacity are checked; out-of-range snapshot
addresses cannot be dereferenced through partial LEN reads or DMA completion.
Both real publication call sites share the helper. Actual AI register/read/
completion entry points reject foreign owners before mutation. Invalid ranges
mark admission failed and do not publish or advance last_read; interrupt/FIFO
failure semantics and scheduling still require their separate audit.

The backend independently authenticates a sample pointer inside its actual AI
RAM view using integer-address bounds without subtracting unrelated pointers.
Complete sample size/alignment/capacity is checked before allocation or copy.
It copies at most 8 MiB into private storage, performs existing channel/sample
conversion there and releases the copy after synchronous processing. Game RAM
and AI registers are no longer temporarily modified by the backend. Nested
source processing and publication during a consumer borrow reject. Copy
allocation failure is a session error before any processor call. This adds a
bounded transient allocation whose actual budget/performance still require
qualification; it is not a real-time allocation or whole-actor-budget claim.

Authored audio_source_cases.cpp includes actual publication and backend copy
bodies with real AI/RI/RAM structs and an explicit private sample processor
substitute. It covers exact final RAM slices, partial/final delivery order,
immutable original bytes/registers, invalid full spans and counts, misaligned/
null/end-overrun pointers, allocation failure and foreign/nested denial. It
does not execute the real AI scheduler/register handlers, channel converter,
resampler, device or worker. No case has run. AI duration arithmetic, queue
admission/delayed carry, malformed snapshot timing, actual resampler ratio/
output bounds, native device/context lifecycle and whole product verification
remain open; source range checks do not prove those behaviors.

AI timing source now computes the DAC divisor and VI counts in uint64_t,
rejecting zero sample rate, intermediate unsigned overflow and non-finite or
negative DMA modifiers before float-to-unsigned conversion. Pinned integer
division and float modifier order are retained for admitted requests. Delays
must fit the signed CP0 scheduling horizon; zero duration remains permitted
as in the pinned implementation. Full request RAM and backend graph admission
precede fifo_push mutation. Actual do_dma independently checks restored FIFO
length/duration and delayed-carry width, widens address addition and admits
the adjusted complete RAM extent before updating the descriptor, backend
frequency or scheduling. Backend admission failure prevents scheduling.

Remaining-length calculation rejects inconsistent duration/event horizons and
last_read exceeding FIFO length instead of deriving oversized output counts.
The current request FIFO may already have been published before a later
do_dma failure (including a delayed carry overrun); failure poisons admission
but is not an atomic rollback of all FIFO/status changes. Full FIFO overflow,
zero-length event semantics, snapshot selector/timing preflight, CPU error-stop
and retirement handling still need completion/qualification. No invalid-state
hardware semantics or universal timing compatibility is claimed.

Authored ai_timing_cases.cpp selects the actual prepared request/DMA methods
with explicit CP0/scheduler/backend observers and independent arithmetic
expectations. Cases include normal truncation, zero modifier, NaN/infinity,
large/negative modifiers, zero sample rate, VI product overflow, invalid RAM,
actual format/schedule publication and valid/invalid carry. The fixture does
not run actual CP0 events, remaining-length logic, FIFO pop, snapshots or CPU.
It remains uncompiled and unexecuted. Resampler producer bounds, original
audio/device/graphics/Windows qualification and all numbered scope remain open.

Non-Apple sinc source now dry-runs the pinned phase/time walk before invoking
its selected processing function. It checks phase widths, positive finite
representable fixed-point step, time-addition headroom, finite input/output
budgets and the predicted total output including startup-phase samples. A
capacity refusal leaves filter state/output untouched. The caller halves the
input block until the checked call fits, retaining the same stateful sinc
backend and admitted channel/sample conversions. It no longer estimates safe
input with MAX_AUDIO_FRAMES/ratio-1, which could underflow or become zero.
Output float/int16 staging is fixed at 131072 stereo frames (1 MiB and 512 KiB
respectively), independent of the admitted <=65536 input-frame buffer. A
one-input-frame expansion which cannot fit rejects as a session failure.

The checked entry authenticates the actual selected sinc backend and compares
its final output count to the prediction. That final comparison is diagnostic;
the pre-write dry-run is the capacity gate. Its proof depends on the pinned
C/SSE/AVX/NEON processing loops preserving the same phase walk and filter
state invariants; unexpected plugin/backend replacement is rejected. Actual
filter allocations/tables/taps/SIMD accesses still require qualification.
The Apple converter path is unchanged by this sinc source change and retains
its independent unfinished accumulation/rate/callback audit. Low-rate output
can exhaust the finite frontend queue within one guest interval; that remains
an explicit session error, not a sample-preserving unlimited stream claim.

Authored sinc_capacity_cases.cpp compares the production planner with a small
independent per-tick walk, checks exact capacity refusal without output commit,
startup phases, low-rate block subdivision needs and invalid fixed-point ratios.
It does not invoke the real sinc processor or backend caller. Actual C/SIMD
sample/phase equivalence across subdivision, guard bytes, all sample rates,
rate changes, queue pressure and allocator/fault behavior remain required in
the final pass. No case, processor or device was executed.

Sinc initialization source additionally rejects non-finite/nonpositive bandwidth,
unknown quality/window and tap expansion beyond 4096 before integer conversion.
The phase-table/delay allocation is capped at 64 MiB with stored element extents.
Checked processing validates the retained buffer layout, tap count and ring
pointer against those extents. Each instance captures its selected processing
function after initialization: creating another quality/window cannot redirect
an older instance's checked call through the global backend's new processor.
This retains supported filter qualities within explicit finite allocation.
Actual allocator failure, supported ISA selection, SIMD alignment/access and
table coefficients are still qualification obligations, not proven by guards.

The new authored sinc_actual_cases.cpp target links the actual adapted pinned
sinc and actual aligned allocator. It compares direct versus checked samples
and output counts across rate changes and qualities, checks output guards and
filter-state preservation after a rejected capacity, rejects bad bandwidths
and covers mixed-quality instance dispatch. C and compile-enabled SIMD paths
are selected explicitly; a compiled ISA must still be qualified on the target
CPU. The direct path is a compatibility reference, not an independent audio
quality oracle. The target is authored only: no configure, compilation or
execution has occurred. No real backend/device completion is established.

AI FIFO publication now stages new/next descriptors locally. do_dma returns
an explicit start-admission result; failed first-slot start does not publish
its descriptor or BUSY bit, and failed pending-slot promotion preserves both
slots and FULL status. Successful starts retain admitted address/carry/format
changes and scheduling, then publish the selected slot. A queued request while
BUSY still replaces the pinned pending slot and sets FULL without starting it;
hardware-specific full-FIFO write policy has not been redefined here.

These guarantees concern descriptor/status publication when local admission
or format callbacks fail, not rollback of external callback side effects.
Scheduler insertion is still a void API and no interrupt-node allocation
receipt is added by this change. A failed start poisons owner admission; actual
CPU failure-stop, completion interrupt, remaining last_read on prior source
failure, retry/retirement and malformed snapshot semantics remain open. FIFO
data is not itself evidence that an event was durably/successfully scheduled.

The expanded actual prepared request fixture now includes production fifo_pop
as well as fifo_push/do_dma. It covers failed first-slot carry, failed pending
promotion, queued second slot, successful promotion/empty retirement and a
format backend explicitly rejecting through real owner admission. CP0/event/
format calls remain observers, and no CPU or test is executed. Valid actual
game timing, repeated full writes, zero events and the final product matrix
remain required in item 16.

AI now uses an explicit checked interrupt insertion entry over the actual
pinned pool/scheduler. Under the existing single-owner execution contract it
admits pool capacity, invokes real count insertion and acknowledges the one-
node index increment. Pool exhaustion rejects before queue mutation and marks
admission failed. Wrapped absolute CP0 counts and duplicate-event behavior
remain those of the pinned scheduler. This receipt covers node insertion, not
event delivery, durable scheduling or arbitrary corrupted queue structures.
Other existing scheduler callers retain their void interface and separate
audit obligations; this does not close all event producers.

AI descriptor, last_read, carry and format-ready publication now follows that
receipt. Refused event insertion sets the actual CPU stop flag and leaves the
staged descriptor/status uncommitted; backend format callback side effects
cannot be rolled back. Guest interrupt checkpoints independently inspect owner
admission before the frame-pending fast path, so a failed session requests CPU
stop at its next interrupt checkpoint. This is not proof that every interpreter/
dynarec/device path reaches that checkpoint or final saves retire correctly.

Authored event_insert_cases.cpp uses the exact prepared pool/scheduler/accessor
fragment plus the new receipt, with actual CP0 structs. It covers all node slots,
full-pool no-mutation, count wrap, zero delay and foreign denial. The AI request
fixture separately injects a refused insertion and observes stop/unchanged
descriptor state. Scheduler fixture cases do not run CPU events; AI stop is
an observing fixture accessor. Actual stop/guest return/final-save/worker behavior
and all original snapshot/device/OS qualification remain unexecuted and due.

Owner source now admits an explicit peripheral flush after a confirmed stopped
guest, including a failed guest. An in-flight/paused failed guest cannot use
this recovery path. During the serialized stopped flush, the aggregate RAM/DD/
RTC publisher can retry retained saves despite failed CPU admission. Its receipt
reports only actual persistence result; a successful save does not clear the
session failure, restore readiness or permit another run/snapshot. Reentrant
flush/end remain blocked until the transaction finishes, and failures retain
data for another checked attempt. Outside that transaction aggregate persistence
remains denied for a failed session. Retirement remains separately checked.

The expanded owner fixture uses real owner state, memory store and prepared
RAM/DD/RTC aggregate methods. It stages dirty RAM, marks guest failure while
running, confirms an actual protocol guest-end, refuses early/unchecked flush,
injects failed publication, retries after stop, verifies committed bytes and
checks failed retirement retains storage. Guest tokens are protocol fixtures,
not OS fibers or CPU execution. No case has run. Actual worker capture order,
fatal device/CPU unwind, all primary save producers, file crash durability,
helper/device cleanup and failed-close retries still require full qualification.
This does not establish safe continuation of a corrupted CPU session.

N64 output-ring drains now cap each callback offer at 16384 stereo frames,
matching the actual native worker's batch admission. The eight-calls-per-drain
limit remains. Large wrapped pending output is delivered across bounded native
batches instead of causing the worker to reject a valid oversized offer. The
actual worker ring callback now returns only its copied frame count, including
zero when full, and rejects inconsistent ring counters before free-capacity
subtraction. Previously it returned the requested count even after copying
only a fitting prefix, losing the core's unacknowledged audio debt.

This receipt means enqueue into the worker ring, not physical playback. The
existing consumer latency trimming policy, intentionally disabled audio,
presentation-helper admission and device delivery are distinct contracts.
Actual direct/presentation routes, pause/resume/counter reset, slow consumers
and every affected core's partial-batch handling require final qualification.
The output fixture now asserts native-sized callback offers and exact ordered
drain over multiple capped passes. It remains unexecuted; no native audio or
SDL device ran. The common GL procedure-name scan also checks its 128-byte
limit before reading the next byte; actual pointer validity is still the
trusted core API's responsibility.

Actual PI/RDP/framebuffer interactions, all DMA/graphics/audio paths, maximum
SP transfers, valid legacy/new snapshots, malformed-state rejection before
callbacks, retries, raw-save compatibility, real clocks/devices/games/drivers,
other cartridge implementations and every original Windows/product/release
requirement remain open. No package, OS write grant, build/test or enabled
product route changed merely because these sources were prepared.
