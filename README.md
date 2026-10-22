# OmniPlay

Chromium-first gaming browser and shared gaming platform. Desktop and Android use a maintained Chromium fork; iOS uses WebKit. The platform coordinates local web games, isolated emulator adapters, installed PC games and authenticated remote sessions.

This repository is being implemented against the supplied master plan and its review. Completion is tracked in `docs/requirements.json`. Files or interfaces being present do not establish that a feature works. Build, integration, device, performance and release evidence are required before marking a requirement complete.

## Layout

| Path | Responsibility |
| --- | --- |
| `browser/` | Pinned Chromium checkout, patch queue and integration |
| `platform/` | Rust contracts, local data, permission broker and session lifecycle |
| `engines/` | Web, native and streaming runtime adapters |
| `apps/` | Game Hub, web and mobile product surfaces |
| `services/` | Optional account, encrypted synchronization and signaling |
| `security/` | Signing policy, dependency inventory and release trust |
| `tools/` | Reproducible bootstrap, packaging and final verification |
| `docs/` | Requirements, decisions and implementation evidence |

No commercial game asset or standalone manufacturer BIOS file has been supplied. Selected upstream engine sources contain original replacement firmware/fonts and, in bsnes, an embedded IPL array removed by the authored distribution adaptation. Corresponding-source and binary content/license verification remain pending. Third-party engines retain their upstream licenses; an engine must be approved in the distribution manifest before release inclusion.

The current development instruction is to author the complete implementation before running one consolidated verification pass. Tests may be written as part of the implementation but are not executed during authoring. No passing result is implied by the presence of a test file.
