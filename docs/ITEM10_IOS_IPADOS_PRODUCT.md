# Item 10 — iOS/iPadOS source closure

## Authored in this assignment

- At most 16 queued/executing ordinary native renderer requests; admission lasts
  until native work returns, independently of reply timeout. A synchronized
  document lifetime and deadline check reject obsolete queued native/media work.
- Native document authorization/expiry has a separate 16-operation budget;
  single-file import/export I/O retains a four-operation budget across document
  replacement. Repeated detach requests coalesce. The existing folder-provider
  registry budget remains separate; provider I/O still has cooperative limits.
- Only the exact capability-prefixed Game Hub `index.html` document, with optional
  fragment and no query, can navigate as the privileged page or send bridge
  commands. Public asset routes are not privileged documents.
- Suspension has a unique identity and a cancellable deadline. Late WebKit/OS
  callbacks cannot finish another suspension. The owned OS background task ends
  at completion/expiry even if the native serial queue stalls; pending save
  success is never inferred from expiry. Foreground admission is restored only
  when visible/active and no suspension remains pending.
- A single retry control covers startup, load and WebKit process failure.
  Cancelled navigation is not shown as a library failure. Profile-identity faults
  produce a recoverable library screen while ordinary browsing stays available;
  identity is not silently reset.
- Bookmark rename/removal, confirmed history clearing and confirmed ordinary
  cookie/website-data clearing. Clearing ordinary website data closes normal
  tabs, preserves private tabs/game data, and waits until downloads retire.
- Persisted URLs/entries are validated; excess history is bounded by encoded
  storage size. Explicit bookmark/history actions can report persistence errors.
- Download deadlines use monotonic uptime; completed files are checked again
  before sharing. Cancellation retains admission/destination until WebKit's
  cancellation callback. A timer is not treated as a hard disk quota.
- Vlikeon display name; retained bundle/profile identifiers for data continuity.
- Explicit simulator build destination, development/release resource admission,
  refusal to archive development resources, fresh archive/XCFramework checks,
  and an explicitly supplied reviewed export classification. The default plist
  no longer declares encryption exemption without a review. The archive recipe
  creates a separate plist from that explicit choice, preserving source metadata.

## Outstanding source and external work

This is **not full item-10 completion**. The original remaining approved core/
applicable no-JIT adapter, complete mobile streaming, shared sync/provider,
localization/accessibility, production icon/notices and release/store requirements
are retained in `IOS_PRODUCT.md`, `MOBILE_SYNC.md` and `IOS_DISC_IMPORT.md`.
They are not moved into item 17 or relabelled as tests to close this item.

Apple SDK/Xcode compilation, physical iPhone/iPad and simulator behavior, provider
stall/cancellation, audio/controller/fullscreen, private browsing and migration
remain unverified. No SDK/compiler, signing, simulator, device or store operation
was run here. The Windows demo's results do not qualify this mobile source.
See `../qa/mobile/ITEM9_ITEM10_ACCEPTANCE.md` for the later execution matrix.
