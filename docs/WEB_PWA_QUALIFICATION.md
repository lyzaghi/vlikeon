# Web / PWA Qualification Matrix & Acceptance — Items 73–76

Status: Automated qualification harness authored (`qa/web_pwa_qualification.py`); live browser grid execution deferred per project owner policy.

## 1. Browser & Device Target Matrix

| Platform / Browser | Engine | Minimum Version | Tested Features |
|---|---|---|---|
| **Google Chrome / Edge** | Chromium / Blink | 100+ | Service Worker, CacheStorage, Web Audio Worklet, Gamepad API, StorageManager |
| **Mozilla Firefox** | Gecko | 115+ ESR | Service Worker, Web Workers, IndexedDB, AudioContext, Gamepad API |
| **Apple Safari (macOS)** | WebKit | 16.4+ | Service Worker, Standalone PWA, OffscreenCanvas, Gamepad API |
| **Apple Safari (iOS / iPadOS)** | WebKit Mobile | 16.4+ | Add to Home Screen, WebKit MessagePort, Fullscreen API fallback, Touch Events |
| **Android Chrome** | Chromium Mobile | 100+ | PWA Web Install Prompt (`beforeinstallprompt`), Persistent Storage, Touch Controls |

## 2. Test Verification Matrix

| Case | Category | Description | Verification Gate |
|---|---|---|---|
| **W01** | Offline Dependencies | Verify non-core assets (audio resampler, web gamepad, fonts, icons) work offline | CacheStorage contains required dependencies; 503 returned only on corruption |
| **W02** | Installation & Updates | PWA install banner displays; service worker update found during active game session delays reload | User gameplay is not interrupted; update applies cleanly after save |
| **W03** | Cache Recovery | Corrupt an application cache entry artificially; service worker detects hash mismatch | Corrupt entry deleted, clean fetch executed or `recoverCache()` resets state |
| **W04** | Storage Quota | Simulate storage estimate near 80% usage threshold or < 100 MiB available | Warning dialog alerts user without deleting saved games or credentials |
| **W05** | Memory Bounded Hashing | Verify SHA-256 calculation on multi-megabyte assets uses chunked buffering | Browser does not crash or exhaust RAM |

## 3. Qualification Execution Commands

When automated browser testing (e.g. Playwright / Selenium grid) is authorized:
```bash
# 1. Run unit qualification suite
python3 qa/web_pwa_qualification.py

# 2. Host local HTTPS test server
python3 -m http.server 8080 --directory apps/web

# 3. Execute browser qualification tests
# playwright test qa/browser_matrix/
```

## 4. Current Status

Items 73–75 are authored in source (`SOURCE_READY_UNVERIFIED`). Live browser matrix qualification (Item 76) is queued as `WAITING_PERMISSION_OR_RESOURCE` pending owner test authorization.
