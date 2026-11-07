# Chromium 158 desktop source migration

The desktop candidate uses Windows Stable 158.0.8127.18 at commit `6449d9f9a6a906edff724b7b220199b3fde4615c`. Each of the 36 integration source inputs was checked against the corresponding Git object from the official Chromium repository. The source downloader, native picker recipe and generated patch queue now use this revision.

The picker keeps upstream's `CHECK(!picker_controller_, base::NotFatalUntil::M161)` and adds Vlikeon's capture policy state after it. The manifest allowlist insertion uses the surviving Web Store case after Chromium removed the reading-mode case. Both product patches apply with exact context. All nine source security invariants passed.

The three-sided review was produced against the previous 155 pin. Rehearsal with the migrated recipes and `--baseline-lock <review>/baseline-lock.json` recorded the old baseline, independently selected 158 candidate commit, exact Git blob matches and successful patch application.

The official cargo-audit scan found no Rust vulnerabilities in the updated 327-package Cargo.lock. The Windows Stable freshness check matched the 158 candidate version and revision. That source audit returned `CLEAN`. [Chromium Dash](https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Windows&num=1) · [RustSec](https://github.com/RustSec/advisory-db)

Android keeps its reviewed source revision in `browser/android-upstream.lock.json`. Its snapshot inventory and patch queue must match that revision. Android source assembly and staging read that explicit pin. An Android upgrade requires its own source migration and qualification.

The desktop 158 candidate still requires a full Chromium build and browser runtime qualification. The recorded live Windows WebRTC test used the existing frozen Vlikeon 155 binary while loading the current JavaScript modules through a dedicated local test server. The CI WebRTC fixture uses its runner's Chrome and local coturn.
