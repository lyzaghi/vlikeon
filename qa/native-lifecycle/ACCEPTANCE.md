# Native lifecycle final qualification

Execute only in an explicit Antigravity batch or the consolidated item-16 pass.
This fixture intentionally violates a C ABI with C++ exceptions; do not ship it,
use real game data, or put it into a product runtime catalog. Match the worker
and DLL compiler/SDK/CRT and retain all source/artifact/configuration hashes.

For the worker temporary-directory ingress cases, use the current fixture DLL
and run `temporary_directory_cases.py --worker <absolute-worker> --core
<absolute-fixture> --output <fresh-output>`. It provides deliberately stale
TEMP/TMP values and a separate private scratch identity. The actual loaded DLL
checks Win32, wide CRT and narrow CRT observations for ASCII and Unicode paths.
A relative private path must terminate the worker with code 2 before core load.
This is direct-worker environment qualification, not an AppContainer/device
test or proof that every statically linked third-party CRT shares environment.

1. Build `engines/native` (or frozen `source/native`) using explicit pinned SDL,
   JSON, libretro and presentation sources. Set `OMNIPLAY_RUNTIME_ID` to
   `native-lifecycle-fixture`, `OMNIPLAY_CORE_FILENAME` to
   `omniplay-lifecycle-fixture.dll`, and video backend to `software`. This is a
   separate qualification build, never a replacement for a product worker.
2. Build this CMake project with the same toolchain and explicit
   `OMNIPLAY_LIBRETRO_INCLUDE`. Retain both DLL targets. Run the saved harness
   with `--worker <exact-worker.exe> --core <normal-fixture.dll> --void-core
   <void-fixture.dll> --output <fresh-absolute-output-directory>`. It owns and
   reaps only the subprocesses it creates and retains an `evidence.json` report.
3. Require all thirteen cases: normal, init-throw, load-throw, run-throw,
   serialize-throw, unserialize-throw,
   pause-refuse, pause-throw, pause-invalid,
   unload-throw, deinit-throw, void-only and protocol-zero. The normal case must
   observe actual core progress, exactly one preparation and one running pause,
   retained SRAM counters, successful stop and exit 0. Failure cases must not
   send a success reply and must exit 1; exit 86 means callback replay or unsafe
   DLL/static/detach execution, and timeout/forced cleanup is a failed case.
   Numeric protocol zero is rejected before core preparation, after which
   ordinary unprepared stop exits 0. Dummy SDL driver unavailability is a setup
   failure, never a passing lifecycle result.
   Unknown init/load/snapshot exceptions must reach EOF without a host error
   reply; termination happens before the host load/save buffers unwind. Test
   a refused termination with live borrowed buffers separately in case 26.
4. Inject GL make-current failure before pause entry on actual hardware routes.
   Retry while the DLL lives. Resume, AV updates and save/load must not reset
   audio or mutate saves before a successful barrier. Inject nested pause,
   resume, stop and shutdown during each pause/unload/deinit callback; there
   must be no nested release or callback replay. Test prepare failure before
   and after background producer creation, along with original boot/reset cases.
5. Qualify actual MD, PS1, PSP and the N64 candidate separately. Observe owner/
   fiber/thread state and producer join, PS1 pending card writes, PSP parked
   frame/readback queues and N64 healthy versus failed-stopped peripheral flush.
   Pause 1 is a producer-quiescence receipt, not a committed final save.
6. With actual Windows workers and the broker, prove fatal unknown lifetime
   does not invoke DLL detach/destructors and that the broker waits for actual
   process/job retirement before work/debt reclamation. Refuse self-termination
   through an explicit qualification injection; the actor must park without
   fallback, successful stop or DLL unload until its owning broker retires it.
   A timeout or a returned termination API is not process-exit evidence.
7. Reject catalog/receipt combinations with missing, boolean, negative or unknown
   numeric protocol, mismatched worker hello and missing checked export. Verify
   signed package hashes and new configuration identity; preserve old save
   history without inventing a cross-version restore exception. Legacy package
   rejection requires a real rebuilt package, not metadata relabelling.
8. Run `wire_cases.py --worker <exact-worker.exe> --core <normal-fixture.dll>
   --output <fresh-absolute-directory>` against that same qualification worker.
   Require all 19 retained cases. Most malformed headers deliberately omit the
   binary-length prefix while keeping stdin open: waiting for more input is a
   failure, as is emitting any reply to the rejected frame. Boundary EOF alone
   exits 0 after checked teardown; partial/malformed frames exit 1. Execute the
   Rust production channel cases too, and retain real broker/process/pipe,
   blocked I/O, cancellation and memory evidence from Windows case 26.

No standalone fixture result establishes AppContainer/job behavior, real driver
safety, all source resources, final-save durability or complete item 2.
