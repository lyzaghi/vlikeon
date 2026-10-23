# Vlikeon Windows playable demo

The first demo is a Windows x64 application with its own Win32 window and an
embedded Microsoft WebView2 surface. It runs the original Vlike Grid Game Boy
game using SameBoy compiled to WebAssembly, the existing worker runtime, and an
audio worklet. This is a Game Hub demonstration, not the complete Chromium
browser or a qualification of the remaining platform roadmap.

## Use

Extract the entire portable bundle into a writable directory and open
`VlikeonDemo.exe`. Microsoft Edge WebView2 Runtime must already be installed.
No administrator privileges, development server, account or commercial ROM is
required. The executable is unsigned.

Select **Chơi ngay**. Use arrows or WASD to collect orbs; Space pauses/resumes.
**Lưu điểm dừng** stores a checkpoint; **Khôi phục** resumes it. Closing the
native window attempts a final save. Keep the adjacent `user-data` directory
to preserve progress. The release archive contains no test profile.

## Build

1. Run the manually dispatched `demo-core-build.yml` workflow for the reviewed
   source revision and download its SameBoy artifact. This does not activate
   the production catalog.
2. Obtain Microsoft.Web.WebView2 SDK 1.0.3537.50. Configure
   `apps/demo/windows` using CMake with Visual Studio 2022, `-A x64`, and
   `-DVLIKEON_WEBVIEW2_SDK=<extracted-sdk>`. Build the Release configuration.
3. Run `python tools/package_demo.py --core <artifact> --executable <exe>
   --webview-sdk <sdk> --output <fresh-repository-out-directory>`.

The packager creates the original ROM from `tools/build_demo_rom.py`, verifies
the core revision and digest, copies license notices and writes a build receipt.
The receipt records the actual JavaScript inputs. Rebuilding against later
runtime changes requires a new qualification; an earlier binary's evidence
does not qualify a later working tree.

## Qualification scope

The first local Windows x64 bundle passed batch 048 on 2026-10-23, including
reopening the real native app and restoring the exact saved frame. The test
observed approximately 60 frames/second and no page errors. Core build 042 and
native build 043 completed successfully. Batch 045 exposed the missing libretro
target sample-rate callback; the demo now supplies 48 kHz without relaxing
runtime timing validation. Batches 046/047 exposed close-test harness mistakes;
048 sends an actual close request to the owned, visible native window.

The qualified bundle uses the frozen runtime inputs recorded in its receipt;
other ongoing runtime edits in the development tree are outside this evidence.
Archive: `VlikeonDemo-0.1.0-windows-x64.zip`.
SHA-256: `387a4cad310485df1948c31818621f34ef0de7f0f10acf56a38438260fc3e13a`.

Antigravity executes the fixed tests; Codex designs the cases, reviews raw
evidence and decides fixes and acceptance. The native app is exercised through
its own temporary loopback CDP endpoint, with a disposable local profile.
The shipped executable does not enable that endpoint itself.

Required checks cover native launch, actual rendered game frames, directional
input, pause/resume, exact saved-frame restoration, native close with final
save, and restoration after reopening. This does not establish gamepad hardware
compatibility, audible output quality, every display scaling factor, other
Windows machines, or completion of the full product.

SameBoy revision: `26a56bd6a122a6318a456abd80558408742d5adf`.
Core build workflow run: `39896946342` (source `dbab0fe`).
SameBoy and its original open boot programs retain their MIT/Expat license;
Emscripten and WebView2 notices accompany the bundle. No manufacturer boot ROM
or commercial game is included. Original Vlike source/game is Apache-2.0.
