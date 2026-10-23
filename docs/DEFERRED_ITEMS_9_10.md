# Deferred items 9 and 10 — owner transition to 15 then 16

On 2026-10-23 the owner explicitly requested retaining the missing mobile work
and moving this chat to security/licensing/release preparation (15), then
consolidated verification (16). This is a work-order change, not acceptance or
completion of Android/iOS. Items 11–14 and other chats' work are not reset.

| Item | Retained missing implementation | Retained verification/release work |
| --- | --- | --- |
| 9 Android | Approved bundled core/mobile adapters; complete applicable mobile Remote Play/input/capture; remaining sync/provider integration; mobile localization/accessibility and launcher/product resources | Linux/NDK/Chromium/JNI/Rust APK build, SAF fault/provider matrix, physical-device audio/input/background/storage/16-KiB behavior, signing/store configuration |
| 10 iOS/iPadOS | Approved core/applicable no-JIT adapters; complete mobile streaming; remaining sync/provider integration; full localization/accessibility, production icons/notices | Actual Apple SDK/Xcode/simulator/iPhone/iPad builds and behavior, lifecycle/provider/download/privacy fault matrix, signing/export/privacy/store review |

Source authored in commit `39d4f7b` remains retained. See
`ITEM9_ANDROID_PRODUCT.md`, `ITEM10_IOS_IPADOS_PRODUCT.md`,
`MOBILE_SYNC.md`, `ANDROID_DISC_IMPORT.md`, `IOS_DISC_IMPORT.md` and
`../qa/mobile/ITEM9_ITEM10_ACCEPTANCE.md` for exact limits and cases.

The new direction to item 16 permits explicitly assigned frozen verification
batches in this chat. It supersedes source-only timing for those batches only.
Antigravity executes tests/builds; Codex reviews evidence and owns fixes and
acceptance. Missing devices/SDKs, failed tests and incomplete source remain
open. A local Windows demo does not qualify another platform or the full app.
