# Item 9 — Android source closure

Owner assignment: complete Android, then iOS/iPadOS. This record separates
authored changes from execution and does not close earlier items 2–8.

## Authored in this assignment

- A maximum of 16 ordinary native renderer requests remains admitted until
  actual native completion reaches the main queue. Expiring a renderer reply
  cannot admit an unlimited backlog behind blocked native I/O.
- The shared native document-work queue also admits at most 32 operations until
  actual return; correlated errors retain the original renderer request ID.
- Queued requests check the owning channel's thread-visible lifetime and the
  request deadline before calling Rust. The independent single media-read lane
  checks the same conditions. Already-running native work remains cooperative;
  this is not hard preemption or rollback of an already committed operation.
- Each channel owns its deadline callbacks. Replacement/close removes them;
  late replies cannot open an export picker for an expired request/document.
- Suspend/detach maintenance coalesces into one queued operation plus sticky
  intents. Repeated navigation/lifecycle callbacks cannot grow that queue
  without bound. Native cleanup does not manufacture a successful save receipt.
- Game Hub's English/Vietnamese menu labels now use Vlikeon. Internal Java
  namespaces, package/profile identifiers and existing on-disk paths are retained
  to avoid discarding existing data or breaking pinned Chromium integration.

Existing SAF imports/exports, selected disc-tree readers, local Rust service,
protected credentials and foreground/background sync remain connected through
their existing source. Ordinary browsing remains upstream Chromium.

## Outstanding source and external work

This is **not full item-9 completion**. Still required by the original scope:

- Approved usable bundled core catalog and applicable mobile/native adapters,
  including their media/audio/input/lifecycle integration (shared with item 5).
- Full mobile Remote Play and owned-input/capture integration as applicable to
  platform APIs; existing browser WebRTC code is not evidence of this completion.
- Full mobile localization/accessibility, launcher/product/store resource wiring,
  final package/signing/update configuration (also item 15).
- Remaining shared mobile sync/provider requirements recorded in `MOBILE_SYNC.md`
  and `ANDROID_DISC_IMPORT.md`; no claim of complete device/provider coverage.

Initial build recipe remains Linux + pinned Chromium + NDK, arm64 only. No APK,
App Bundle, JNI/Rust/Java build or Android device test was run in this assignment.
Development source assembly is not an APK or a passing test. Exact acceptance
cases are in `../qa/mobile/ITEM9_ITEM10_ACCEPTANCE.md` for later owner-released
Antigravity execution. Do not infer Android readiness from the Windows demo.
