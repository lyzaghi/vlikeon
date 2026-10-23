# Roadmap

Active development, with no promised release dates or measured completion
percentage. The authoritative task sequence is
[`docs/OUTSTANDING_WORK.md`](docs/OUTSTANDING_WORK.md), numbered 1–109.
Source integration task 1 is complete at `35f3271`; task 2 documentation alignment
is recorded in [`DOCUMENTATION_ALIGNMENT.md`](docs/DOCUMENTATION_ALIGNMENT.md).
Task 3 [Windows Chromium build](docs/WINDOWS_CHROMIUM_BUILD.md) is complete for
its frozen x64 development inputs and output integrity. The next task in this
chat's sequence is 4 (Linux build); parallel assignments remain in the task list.
This does not close runtime acceptance or the historical desktop product area.

The table uses the historical 17 product-area identifiers for traceability.
[`docs/COMPLETION_SEQUENCE.md`](docs/COMPLETION_SEQUENCE.md) retains prior
assignments and deferrals; its historical ordering does not override 1–109.

| Order | Area | Status |
| --- | --- | --- |
| 1 | Chromium desktop and Game Hub | Windows x64 development build accepted; runtime and other desktop hosts pending |
| 2 | Windows runtime and containment | Source in progress; qualification pending |
| 3 | Linux runtime and containment | Completion review and qualification pending |
| 4 | macOS runtime and containment | Completion review and qualification pending |
| 5 | Retro engine integration | Candidates; distribution/compatibility pending |
| 6 | Graphics, audio and input | Completion review and qualification pending |
| 7 | Library, saves and storage | Completion review and qualification pending |
| 8 | PC remote play | Completion review and qualification pending |
| 9 | Android | Completion review and qualification pending |
| 10 | iOS | Completion review and qualification pending |
| 11 | Web/PWA | Completion review and qualification pending |
| 12 | Accounts and encrypted synchronization | Completion review and qualification pending |
| 13 | Performance, routing and assistant | Completion review and qualification pending |
| 14 | Installation, repair and updates | Completion review and qualification pending |
| 15 | Security, licenses and release | Review and qualification pending |
| 16 | Consolidated verification | Selected Windows/shared-source batches passed; full matrix open |
| 17 | Overall review and closure | Open; final decision follows remaining implementation and evidence |

## Established evidence

- [Windows Chromium development build](docs/WINDOWS_CHROMIUM_BUILD.md): full
  `chrome` target and independent selected output integrity; no runtime or release acceptance.
- [Windows Game Hub demo](docs/PLAYABLE_DEMO.md): scoped standalone WebView2 demo,
  not the full Chromium browser. Existing demo evidence does not qualify later
  product source.
- [Integrated source](docs/SOURCE_INTEGRATION.md): selected Rust/JS/Python tests,
  synthetic audio contract, fixture-configured native compilation and repeatable
  development resource generation. Four Windows fixture cases remain ignored.
- [Historical verification](docs/ITEM16_VERIFICATION.md): exact frozen inputs,
  actual failures and limited passing scopes. Counts overlap later reruns.

## Before a full-browser experimental release

- Establish a repeatable clean-clone build and record exact toolchains.
- Execute the verification matrix and fix failures with traceable evidence.
- Measure performance and compatibility on named devices.
- Complete dependency/content/source-obligation reviews for shipped components.
- Qualify installers, updates, recovery and platform containment.
- Publish release notes with evidence-backed limitations.

The separate demo does not satisfy these full-browser release gates. No dated
milestone, full-browser release tag or passing CI badge is implied here.
