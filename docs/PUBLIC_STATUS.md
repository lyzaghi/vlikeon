# Public status — 2026-10-23

Vlikeon Browser is the public identity of the source project called OmniPlay
internally: a Chromium-first gaming browser by Vlike. Internal identifiers and
wire formats retain `omniplay`; their migration is not complete.

## Development and task sequence

Development is active. [`OUTSTANDING_WORK.md`](OUTSTANDING_WORK.md) is the
current 1–109 task list. Task 1 source integration is complete at `35f3271`;
task 2 alignment is recorded in [`DOCUMENTATION_ALIGNMENT.md`](DOCUMENTATION_ALIGNMENT.md).
Task 3 [Windows Chromium build](WINDOWS_CHROMIUM_BUILD.md) is accepted for its
frozen x64 development inputs and output integrity. Task 4 is next in the build
sequence; parallel source assignments and deferrals remain in the task list.
Older 1–17 numbers in implementation records refer to product areas and
historical assignments. No evidence-based completion percentage is available.

## What exists and what ran

The public source repository is [lyzaghi/vlikeon](https://github.com/lyzaghi/vlikeon).
It includes browser integration, Game Hub, Rust platform/services, native/web
adapters, mobile source, packaging recipes, acceptance cases and logo/icon
assets. Source presence does not establish readiness.

| Evidence | Established scope | Limits |
| --- | --- | --- |
| [Windows Chromium build](WINDOWS_CHROMIUM_BUILD.md) | Full development `chrome` target; frozen source identity; independent x64 entry-point and selected output hashes | No runtime, clean-clone reproducibility or release acceptance; later source changes unqualified |
| [Windows demo](PLAYABLE_DEMO.md) | Separate Win32/WebView2 Game Hub; original game; launch/input/pause/save/close/reopen checks on frozen inputs | Not full Chromium or broad device/game coverage |
| [Integrated source](SOURCE_INTEGRATION.md) | 53 Rust, 13 JavaScript, 19 Python cases; synthetic audio contract; fixture-configured worker/helper compilation; repeatable 435-file development generation | Four explicit Windows cases ignored; no full browser/core/device or other-OS acceptance |
| [Earlier batches](ITEM16_VERIFICATION.md) | Exact source identities, scoped passes and retained failures | Counts overlap later reruns; do not add them as new distinct coverage |

Antigravity executes assigned tests only. Codex reviews raw results, diagnoses,
authors fixes and decides acceptance. Failed or ignored checks stay visible.
Current source evidence is in `../qa/evidence/sequential/item-001/`; earlier
indexes remain under `../qa/evidence/item16/` and `../qa/evidence/antigravity/`.

## Published technical demo

GitHub metadata checked on 2026-10-23 confirms a published, non-draft prerelease:
[v0.1.0-demo.1](https://github.com/lyzaghi/vlikeon/releases/tag/v0.1.0-demo.1).
Its Windows x64 ZIP is `VlikeonDemo-0.1.0-windows-x64.zip`, SHA-256
`387a4cad310485df1948c31818621f34ef0de7f0f10acf56a38438260fc3e13a`.
The executable is unsigned and requires WebView2 Runtime.
See [use/build details](PLAYABLE_DEMO.md) and
[Vietnamese trial instructions](DEMO_USER_TEST.md). Existing demo evidence
qualifies its frozen inputs, not later product source; tasks 1–2 do not rebuild it.

## What remains unestablished

- Linux/macOS Chromium builds and a verified full-product clean-clone build.
- Runtime qualification of the Windows development Chromium build (task 6).
- Production core, mobile, Linux/macOS, real-device and end-to-end acceptance.
- Controlled performance/compatibility results or completed security audits.
- Qualified production install/repair/update, signing or notarization.
- Complete artifact license/source review, final SBOM and distribution approval.
- Established adoption, production use, independent review or third-party support.

The demo prerelease does not close these gates. No OSS grant submission or
acceptance is recorded in project evidence; no sponsorship is implied.

## Licensing, security and history

Original material is Apache-2.0 unless a file has another notice. Upstream and
per-file licenses remain in force; see [licensing scope](../LICENSING.md).
Profiles, secrets, dependency caches and large build outputs are excluded from
Git. Publication checks and bounded marker scans are not security audits.

Follow [SECURITY.md](../SECURITY.md) for private reporting. Earlier publication
records report private vulnerability reporting enabled; this document is not
a live service check or a support SLA. Operational support and independent
security review remain open tasks.

[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) preserves the source
journal; [`COMPLETION_SEQUENCE.md`](COMPLETION_SEQUENCE.md) preserves older
assignments. Local `out/` references describe preparations, not release downloads.
Historical snapshot claims do not supersede later scoped evidence.
