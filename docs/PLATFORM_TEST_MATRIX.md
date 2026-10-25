# OmniPlay Cross-Platform Qualification Test Matrix (Item 106)

This matrix defines the complete functional, performance, security, and hardware compatibility testing coverage across all supported platforms.

---

## 1. Platform Matrix Overview

| Platform | Target OS / Hardware | Primary Runtime(s) | Input Methods | Audio / Video Backends | Status |
|:---|:---|:---|:---|:---|:---|
| **Windows Desktop** | Windows 10/11 (x64) | Native (`mgba`, `beetle-psx`, `bsnes`), Remote Host/Client | XInput, DInput, RawInput, Virtual WinUHID Gamepad | DirectX 11/12, DXGI SwapChain, WASAPI | `WAITING_PERMISSION_OR_RESOURCE` |
| **macOS Desktop** | macOS 12+ (Apple Silicon M-series, Intel x86_64) | Native, Remote Client | GameController.framework, IOHIDManager, Virtual UHID | Metal, CAMetalLayer, CoreAudio | `WAITING_PERMISSION_OR_RESOURCE` |
| **Linux Desktop** | Ubuntu 22.04/24.04, Fedora 39+, Arch Linux | Native, Remote Host/Client | evdev, uinput virtual gamepad, SDL2 | Wayland, X11, EGL, PulseAudio, ALSA | `WAITING_PERMISSION_OR_RESOURCE` |
| **Android Mobile** | Android 12+ (API 31–35, ARM64-v8a, 16K ELF page alignment) | Native C++ libretro bridge, Remote Client | Touch overlay, Android Gamepad API | Android SurfaceView, AAudio, Oboe | `WAITING_PERMISSION_OR_RESOURCE` |
| **iOS / iPadOS** | iOS 16+ (A-series, M-series, pure interpreter mode) | No-JIT Native Core Adapter, Remote Client | On-screen touch controls, MFi / DualSense Bluetooth controllers | Metal, AVAudioEngine | `WAITING_PERMISSION_OR_RESOURCE` |
| **Web / PWA** | Chromium 128+, Firefox 128+, Safari 17+ | WebAssembly SIMD, Web Worker | Web Gamepad API (64-byte reports), Pointer Events | WebGL2, WebGPU, Web Audio Worklet | `WAITING_PERMISSION_OR_RESOURCE` |

---

## 2. Test Execution Domains

### 2.1 Emulation Accuracy & State Preservation
- Core load, state save, state load, and slot cycling.
- ROM content hashing (SHA-256) matching GoodTools / No-Intro verified dumps.
- SRAM battery save persistence across power loss / unexpected termination.

### 2.2 Remote Play Low-Latency Streaming
- ICE STUN/TURN binding and host-candidate connectivity checks.
- DTLS 1.2 handshake and ChaCha20-Poly1305 / AES-128-GCM ciphers.
- Replay window verification under network packet reordering.
- Video latency (target: < 20 ms render-to-photon on local Wi-Fi 6).
- H.264 / VP8 hardware encode/decode pipelines.

### 2.3 Cloud Save Synchronization
- Resumable upload/download with 256 KiB chunk streaming.
- Lost `upload_commit` auto-reconciliation (`already_stored: true`).
- 24-hour orphan staging sweep and 500 MiB quota enforcement.
- Immediate device revocation fencing across active sessions.

### 2.4 Accessibility & Localization
- 100% 1:1 Vietnamese and English localization catalogs.
- Screen reader announcements (Windows Narrator, macOS VoiceOver, Android TalkBack).
- High contrast themes and reduced motion settings.
