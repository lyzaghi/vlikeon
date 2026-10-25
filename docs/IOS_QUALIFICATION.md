# iOS / iPadOS Platform Qualification Matrix & Acceptance — Item 72

Status: Automated qualification harness authored (`qa/mobile/ios_qualification.py`); physical hardware execution deferred per project owner policy.

## 1. Test Environment Requirements

To execute complete qualification on Apple hardware:
- **Device Requirements:**
  - Physical iPhone (iPhone 12 through iPhone 16) or iPad (M1/M2/M4 iPad Pro, iPad Air) running iOS 17.0+.
  - Apple Silicon Mac running macOS 14+ with Xcode 15/16 and iOS Simulator.
  - External MFi, DualShock 4, DualSense, or Xbox Wireless Controller paired via Bluetooth.
- **Tooling:**
  - Xcode Command Line Tools (`xcodebuild`, `xcrun simctl`)
  - Python 3.10+
  - VoiceOver screen reader enabled on device or simulator.

## 2. Test Matrix Cases

| Case | Category | Description | Verification Gate |
|---|---|---|---|
| **I01** | Non-JIT Compliance | Verify all emulator cores run in pure interpreter mode without `mprotect(PROT_EXEC)` | App Store Guideline 2.5.2 compliance verified |
| **I02** | Memory Caps | Verify that core memory allocations are bounded at 512 MiB and handle `didReceiveMemoryWarningNotification` | Jetsam memory termination avoided |
| **I03** | Remote Play Input | Connect Bluetooth game controller; verify 64-byte `StreamGamepadReport` dispatching via `GCController` | Monotonic sequence, neutral reports on disconnect |
| **I04** | File Provider | Access saves via Files app / document picker; attempt directory traversal (`../`) | Sandbox boundaries hold; SQLite/Keychain files isolated |
| **I05** | Background Sync | Schedule background refresh via `BGTaskScheduler`; enable Low Power Mode | Task cancelled immediately on Low Power Mode |
| **I06** | Localization & VoiceOver | Navigate Game Hub with VoiceOver in English and Vietnamese | 100% of controls announce localized VoiceOver accessibility labels |

## 3. Qualification Execution Commands

When macOS/iOS hardware access is provided, execute:
```bash
# 1. Run automated test suite
python3 qa/mobile/ios_qualification.py

# 2. Build simulator application
python3 tools/build_ios.py --cargo /usr/local/bin/cargo --xcodebuild /usr/bin/xcodebuild --destination simulator --development

# 3. Boot iOS Simulator and install app
xcrun simctl boot "iPhone 15 Pro"
xcrun simctl install booted "out/ios/OmniPlay.app"
xcrun simctl launch booted org.omniplay.browser
```

## 4. Current Status

Items 67–71 are authored in source (`SOURCE_READY_UNVERIFIED`). Physical device testing (Item 72) is queued as `WAITING_PERMISSION_OR_RESOURCE` pending owner hardware authorization.
