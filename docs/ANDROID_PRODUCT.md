# Android product source

The Android product extends the pinned Chromium Android browser. Game Hub opens
from its native menu in an actual Chromium tab; the source does not substitute
the device's Android System WebView for the planned Chromium engine. The fork's
ordinary tabs, incognito UI, website permissions, downloads, renderer sandbox,
site isolation and certificate checks continue through upstream Chromium.

The app packages an inventoried, read-only Game Hub and approved WASM cores.
A per-controller IPv4 loopback server has a random 192-bit route prefix, an
ephemeral port, exact Host validation, bounded clients/headers/deadlines and no
HTTP write or RPC route. COOP/COEP and a restrictive CSP apply to every resource.
No content selected by a user is served from this origin. Bundled assets are
covered by APK signing; core integrity is checked against its build manifest.

Only the explicitly opened tab can receive a native MessagePort. The native
side checks its exact committed entry URL and transfers the other endpoint with
an exact target origin. Navigation, renderer loss and tab destruction revoke the
channel. Requests have correlated IDs, a 1 MiB bound, 16 pending slots and bounded
deadlines; the native side supplies the profile ID. Browsing pages have no
JavaScript interface, native object, filesystem method or application handle.
The normal Android permission model still controls access to application data.
The channel is established after the module installs a constant readiness marker;
it does not wait for the page load event, which depends on startup's `hello`
request. Same-document view fragments retain the channel, while a different
document revokes transient file grants, transfers and observed sessions.

The JNI library links the shared Rust mobile service and C ABI. A process-owned
profile under Android's no-backup storage reuses the local SQLite library,
indexed library pagination, immutable save generations, transfers, performance
records, preferences and local rules. File access uses the Storage Access
Framework. Native code copies selected content into private staging, grants Rust
an opened descriptor, then removes the staging path. Export writes a private
staging file first, prompts for a system destination, copies it and only then
reports completion. Cancellation, provider errors, size limits and timeouts are
reported rather than treated as successful imports or exports. No broad external
storage permission or persistent document grant is requested by this module.

Tab hiding and activity pause ask the shared player to pause and commit progress.
A native deadline marks the observed session paused even if the renderer does
not answer. Android may terminate the process without running these callbacks;
last-frame durability is not claimed. Existing periodic immutable saves remain
the recovery baseline. App data is excluded from Android automatic backup by
using the platform's no-backup directory, independently of browser history.

`tools/assemble_android.py` authors the source module and a separate revision-bound
patch queue. `tools/stage_browser_integration.py --android` stages it without a
build. `tools/build_android.py` is an explicit Linux/NDK/Cargo/GN/Ninja recipe for
the initial arm64 APK and optional standalone App Bundle. The JNI library uses
16 KiB ELF segment alignment; the final APK/native-library alignment must still
be verified on real 16 KiB devices. Other ABIs and Trichrome are not claimed by
this initial recipe. The release package identity, signing keys, Play account,
privacy declarations and store review require real release configuration.

Development assembly may have an explicitly empty core catalog. It cannot pass
release assembly/build gates. No dynamically fetched emulator code, bundled
commercial game or standalone manufacturer firmware is supplied. Native mobile
emulators, document-provider/coordination qualification, full background sync,
remote streaming, complete mobile UX/localization and real compatibility/device
lab work remain in the full project scope. No Android compilation, application
launch, emulator/device run or test has occurred during source authoring.

Pinned APIs are retained with Git blob identities under
`browser/android-snapshots/inventory.json`. The relevant upstream sources are
[WebContents](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/content/public/android/java/src/org/chromium/content_public/browser/WebContents.java),
[MessagePort](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/content/public/android/java/src/org/chromium/content_public/browser/MessagePort.java)
and [WindowAndroid](https://chromium.googlesource.com/chromium/src/+/0e66b94549a8bd85f03fbdf189b752d6e7b63c32/ui/android/java/src/org/chromium/ui/base/WindowAndroid.java).

Explicit device snapshots now augment the bounded Rust base report through ActivityManager, the sticky system battery intent and PowerManager battery-saver/thermal-status APIs. Native code retains collection source/time and does not register a persistent listener, request new permission or infer measured throttling/FPS. Source only; no APK, API call or device evidence has run. See `DEVICE_INFORMATION.md`.

Native profile opening now supplies a separate retained JNI secure credential provider: per-profile AndroidKeyStore AES-GCM wrapping, bounded authenticated no-backup ciphertext, exact-file publication/removal, nonblocking lease, explicit unlock checks and kept native callback signatures. The root/recovery key is not stored. Account commands and one explicit foreground sync worker are now authored with independent coalesced lifecycle cancellation through trusted tab/Activity hooks; legacy opens keep sync unavailable. No account, network, key, file, JNI, API, APK or test has run. See `MOBILE_CREDENTIALS.md` and `MOBILE_SYNC.md` for cooperative I/O limits and remaining background/device/store qualification.

The Android 11+ native hello now advertises the authored SAF disc-folder picker. A transient read-only selected tree resolves exact bounded document names; only referenced assets are streamed through JNI into the shared CUE/M3U import worker. Lazy stream tokens, asset subranges, known/unknown lengths, nonblocking descriptor reads, cooperative deadlines, native cancellation and bounded context/cleanup admission are authored. Normal single-file staging/export remains separate. No URI/provider/import, JNI/API, APK/compiler or test has run; actual provider, filesystem, core and device/store qualification remain required. See `ANDROID_DISC_IMPORT.md` and `NATIVE_DISC_PROVIDERS.md`.
