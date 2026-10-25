# Android Platform Qualification Matrix & Acceptance — Item 66

Status: Automated qualification harness authored (`qa/mobile/android_qualification.py`); physical hardware execution deferred per project owner policy.

## 1. Test Environment Requirements

To execute complete qualification on real Android hardware:
- **Device Requirements:**
  - Standard Android ARM64-v8a device running Android 10 (API 29) through Android 15 (API 35).
  - 16 KiB memory page size enabled device (Google Pixel 8, Pixel 8 Pro, Pixel 9 running Android 15 16 KiB developer preview or release build).
  - External Bluetooth or USB gamepad (Xbox Wireless Controller, DualShock 4 / DualSense, or Android HID gamepad).
- **Tooling:**
  - Android SDK Platform-Tools (`adb 34.0.0+`)
  - Python 3.10+
  - Google TalkBack screen reader installed and enabled.

## 2. Test Matrix Cases

| Case | Category | Description | Verification Gate |
|---|---|---|---|
| **A01** | ELF & APK Alignment | Verify 16 KiB ELF alignment on `libomniplay_mobile.so` and uncompressed APK entry offsets | `python3 tools/verify_android_16k_alignment.py <apk_path>` returns 0 |
| **A02** | Core/Runtime Bridge | Game Hub tab opens with origin-bound MessagePort; verifies `ApprovedCores` catalog | Catalog integrity verified, unapproved dynamic code execution rejected |
| **A03** | Native Request Queue | Dispatch 20 concurrent native operations; verify maximum 16 admitted and excess rejected with `busy` | Queue bounds strictly hold at 16 requests |
| **A04** | Lifecycle & Watchdog | Switch tabs or press Home; verify `suspend` triggered, fallback timer (25s) executes without memory leaks | No orphan processes; session pauses cleanly |
| **A05** | DocumentsProvider SAF | Query roots and child documents via SAF; attempt path traversal (`../`) | Path traversal returns `FileNotFoundException`; root boundaries strictly enforced |
| **A06** | Remote Play Input | Touch on-screen buttons and move analog sticks on physical controller; verify 64-byte `StreamGamepadReport` dispatch | Neutral reports dispatched on deactivate; 250ms gap check heartbeat active |
| **A07** | Background Sync | Schedule periodic sync with `JobScheduler`; trigger screen off or low battery | Task cancelled immediately without leaving uncommitted staging debt |
| **A08** | Accessibility & L10n | Navigate entire Game Hub and controller overlay using TalkBack in Vietnamese and English | 100% of interactive controls announce valid localized content descriptions |

## 3. Qualification Execution Commands

When hardware access is provided, execute:
```bash
# 1. Run automated test suite
python3 qa/mobile/android_qualification.py

# 2. Check 16 KiB page size compliance
python3 tools/verify_android_16k_alignment.py out/OmniPlayAndroid/apks/ChromePublic.apk

# 3. Install APK on connected device
adb install -r out/OmniPlayAndroid/apks/ChromePublic.apk

# 4. Launch Game Hub via intent
adb shell am start -n org.chromium.chrome/org.chromium.chrome.browser.ChromeTabbedActivity -d "about:blank"
```

## 4. Current Status

Items 61–65 are authored in source (`SOURCE_READY_UNVERIFIED`). Physical device testing (Item 66) is queued as `WAITING_PERMISSION_OR_RESOURCE` pending owner hardware authorization.
