# Android Build Recipe — Item 65

Status: Authored build recipe and verification tooling; build and packaging deferred by project owner policy.

## 1. Prerequisites & Toolchain Environment

The Android build recipe produces an official `aarch64-linux-android` (arm64-v8a) APK and Android App Bundle (AAB) combining Chromium, Java/Kotlin classes, JNI C++ glue, and the static Rust `omniplay-mobile` archive.

- **Host Operating System:** Linux x86_64 (Ubuntu 22.04 LTS or Debian 12 recommended)
- **Android NDK:** NDK r26b (26.1.10909125) or newer (provides 16 KiB alignment by default)
- **Clang/LLVM:** Embedded NDK LLVM toolchain at `$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin`
- **Rust Toolchain:** `rustc 1.78.0+` with target `aarch64-linux-android` installed via `rustup target add aarch64-linux-android`
- **Chromium Dependencies:** Depot Tools (`gn`, `ninja`), Java 17 JDK (`openjdk-17-jdk`)

## 2. 16 KiB Page Size ELF Alignment Requirement

Android 15+ devices (including Google Pixel 8, 9 and ARMv9 devices) require all native ELF shared libraries to support 16 KiB memory page boundaries.

To enforce compliance, the build recipe passes:
```text
-Wl,-z,max-page-size=16384
-Wl,-z,common-page-size=16384
-Wl,--exclude-libs,ALL
```
In `chrome/android/omniplay/BUILD.gn`:
```gn
shared_library("omniplay_mobile") {
  assert(current_cpu == "arm64", "This initial Android native artifact recipe is arm64 only")
  sources = [ "native/mobile_jni.cc" ]
  include_dirs = [ "include" ]
  inputs = [ "native/libomniplay_mobile.a" ]
  libs = [ rebase_path("native/libomniplay_mobile.a", root_build_dir), "dl", "m", "log" ]
  ldflags = [ "-Wl,--exclude-libs,ALL", "-Wl,-z,max-page-size=16384" ]
}
```

Verification of built libraries and APK packages is performed using:
```bash
python3 tools/verify_android_16k_alignment.py out/OmniPlayAndroid/apks/ChromePublic.apk
```

## 3. Step-by-Step Compilation Flow

### Step 3.1: Rust Mobile Library Build
```bash
export ANDROID_NDK_ROOT=/path/to/android-ndk-r26b
export TOOLCHAIN=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$TOOLCHAIN/aarch64-linux-android33-clang
export CC_aarch64_linux_android=$TOOLCHAIN/aarch64-linux-android33-clang
export AR_aarch64_linux_android=$TOOLCHAIN/llvm-ar

cargo build --locked --release --target aarch64-linux-android -p omniplay-mobile
cp target/aarch64-linux-android/release/libomniplay_mobile.a browser/chromium/chrome/android/omniplay/native/
```

### Step 3.2: GN Configuration
Generate build directory with official production GN args:
```bash
gn gen out/OmniPlayAndroid --args='
  target_os = "android"
  target_cpu = "arm64"
  is_debug = false
  is_official_build = true
  is_component_build = false
  symbol_level = 0
  enable_resource_allowlist_generation = false
  ffmpeg_branding = "Chrome"
  proprietary_codecs = true
'
```

### Step 3.3: Ninja Compilation & Packaging
Build APK and App Bundle:
```bash
ninja -C out/OmniPlayAndroid chrome_public_apk chrome_public_bundle
```

### Step 3.4: Alignment & Signature Verification
Verify 16 KiB page size compliance:
```bash
python3 tools/verify_android_16k_alignment.py out/OmniPlayAndroid/apks/ChromePublic.apk
```

Verify signature and v2/v3 signing schemes:
```bash
$ANDROID_HOME/build-tools/34.0.0/apksigner verify --verbose out/OmniPlayAndroid/apks/ChromePublic.apk
```

## 4. Current Verification Status

Execution of NDK compilation, Chromium GN/Ninja build, and signing operations is deferred per owner instructions. Source recipes, configuration files, and alignment verification scripts are fully authored and ready for execution when qualification permission is granted.
