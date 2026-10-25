# Item 11 — Web/PWA source work

The owner directed moving from deferred item-7/8 gaps to items 11 then 12.
See `DEFERRED_ITEMS_7_8.md`. This item is open; no build, browser, service worker,
CacheStorage, file operation or test was executed for these source changes.

## Authored changes

The service worker now verifies cached inventoried resources against declared
length and SHA-256 before serving them, reconstructing its own policy headers.
Corrupt cached resources are removed from the application cache and a verified
network fetch is attempted. If that fails, the existing explicit 503 is returned.
Application cache repair never touches IndexedDB game/save/credential records.
Concurrent requests for one path share a verification job and receive separate
Response clones. Job identities come only from the capped 4,096-entry inventory.
Each resource retains the existing maximum 512-MiB inventory bound. Whole-buffer
hashing can still consume substantial memory and needs real device qualification.

Settings now offers explicit preparation of an entire inventoried core directory
for offline use, including its module, WASM, manifest and auxiliary files. Only
the active worker's pinned inventory supplies paths. At most 128 resources and
1 GiB of declared content may enter one group; one preparation is admitted per
worker. The job has a 90-second cancellation budget, checks the owning client
between resources and uses abortable network fetches. The UI shows progress and
cancel, retires its MessagePort on completion/100-second timeout and requests
cancellation on pagehide. Cancellation retains already verified cached content.
It does not preempt an admitted hash/cache operation or guarantee immediate
reclamation. Worker termination can interrupt a preparation; retry reuses and
rechecks retained resources.

Cache status now requires presence of all inventoried files in each core's
directory, rather than its WASM alone. Status is explicitly
`verification:cache_presence_only`; it is not a fresh checksum or a promise that
browser storage will remain. The UI distinguishes presence from actual verification.
Every subsequently served resource is checked again. Game files, firmware and
saves are separate from offline app/core preparation.

Message requests require an app-origin client in the registered scope. Preparation
requires an app root/index document and no query string. Cancel is bound to the
same originating client and operation UUID. Updates retain the existing normal
activation policy; no skipWaiting or forced game interruption is introduced.

## Remaining scope

This is not full item-11 completion. Finish the full original Web product audit,
offline dependencies outside a core directory, installation/update/recovery UX,
quota/memory/performance behavior and remaining browser/device compatibility.
Existing runtime/core/storage/account obligations retain their own scope.
Final acceptance must include corrupt cache offline/online, duplicate fetches,
parallel resources, parallel resources, partial quota failures, cancellation and closed clients,
worker termination, update coexistence, slow hashes and actual offline launches.

## Offline Dependencies, PWA Lifecycle, Quota & Browser Compatibility (Batch 05)

Source implementation added covering items 73–76:
- `apps/web/platform/web-gamepad.js` & `audio-resampler.js` (Item 73): W3C Gamepad API bridge with 64-byte `StreamGamepadReport` serialization, 250ms gap check, neutral report fences; Float32 linear/cubic audio resampler for Web Audio API.
- `apps/web/platform/pwa-install.js` (Item 74): `beforeinstallprompt` and `appinstalled` event handling; non-intrusive service worker updates (never interrupts running game sessions); `recoverCache()` for clean cache re-synchronization on detected corruption.
- `apps/web/platform/quota-manager.js` (Item 75): `StorageManager` API integration, persistent storage grant requests, low-storage warning thresholds (80% / 100 MiB reserve), chunked streaming SHA-256 verification.
- `qa/web_pwa_qualification.py` & `docs/WEB_PWA_QUALIFICATION.md` (Item 76): Automated test harness and cross-browser qualification matrix (`WAITING_PERMISSION_OR_RESOURCE`).

