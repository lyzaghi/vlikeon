# iOS / iPadOS Build Recipe — Item 71

Status: Authored build recipe and verification tooling; build and packaging deferred by project owner policy.

## 1. Prerequisites & Toolchain Environment

The iOS build recipe compiles the shared Rust `omniplay-mobile` archive into an XCFramework and packages the native Swift/UIKit/WebKit application using Apple's `xcodebuild` toolchain.

- **Host Operating System:** macOS 14 (Sonoma) or macOS 15 (Sequoia) running on Apple Silicon (M1/M2/M3/M4)
- **Xcode:** Xcode 15.4+ or Xcode 16.0+ with iOS 17.0+ SDK and Command Line Tools
- **Rust Toolchain:** `rustc 1.78.0+` with Apple targets:
  ```bash
  rustup target add aarch64-apple-ios
  rustup target add aarch64-apple-ios-sim
  ```
- **Signing & Provisioning:**
  - Unsigned simulator / development builds: `CODE_SIGNING_ALLOWED=NO`
  - Release archives: Apple Developer Team ID with Distribution Provisioning Profile and an explicit reviewed export compliance classification (`--export-compliance exempt` or `non-exempt`).

## 2. Compilation Flow

### Step 2.1: Rust Cross-Compilation
Compile static library slices for device and simulator:
```bash
cargo build --locked --release --target aarch64-apple-ios -p omniplay-mobile
cargo build --locked --release --target aarch64-apple-ios-sim -p omniplay-mobile
```

### Step 2.2: XCFramework Assembly
Combine target slices and C ABI headers into a universal XCFramework:
```bash
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libomniplay_mobile.a \
  -headers platform/mobile/include \
  -library target/aarch64-apple-ios-sim/release/libomniplay_mobile.a \
  -headers platform/mobile/include \
  -output apps/ios/Libraries/OmniPlayMobile.xcframework
```

### Step 2.3: Xcode Project Build

#### A. Simulator Build (Unsigned Development):
```bash
python3 tools/build_ios.py \
  --cargo /usr/local/bin/cargo \
  --xcodebuild /usr/bin/xcodebuild \
  --configuration Debug \
  --destination simulator \
  --development
```

#### B. Physical Device Archive (Official Release):
```bash
python3 tools/build_ios.py \
  --cargo /usr/local/bin/cargo \
  --xcodebuild /usr/bin/xcodebuild \
  --configuration Release \
  --destination device \
  --team TEAM_ID_12345 \
  --export-compliance exempt \
  --archive
```

## 3. Security & Non-JIT Compliance Verification

During build and packaging, verify:
- No JIT or debugger entitlement (`get-task-allow`) in release `Info.plist`.
- Privacy manifest (`PrivacyInfo.xcprivacy`) included in bundle with declared API usage (File Timestamp, System Boot Time, User Defaults).
- All emulator cores execute strictly in pure interpreter mode per Apple Guideline 2.5.2.
