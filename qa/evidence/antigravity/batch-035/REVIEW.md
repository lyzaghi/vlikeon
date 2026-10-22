# Direct SDL regression after actor separation

Codex reviewed all three per-case evidence files and raw logs. Source
04cbd749592971cab78b053d542bfbb12010218c was built with device-free mode OFF,
static CRT and pinned SDL/JSON/libretro headers. The worker and both qualification
DLLs built successfully. All 35 cases passed: three private temporary directory
cases, 13 lifecycle cases and 19 control-wire cases. Expected refusal/exception
cases have their required nonzero exit codes; they are not normal-exit claims.
Source and dependency inventories remained unchanged. Antigravity executed only
the assigned batch and made no source or dependency edits.

This checks the shared engine/core loader changes against the direct-worker
qualification harness using dummy SDL drivers. It adds no AppContainer, real
core, graphics/audio hardware or full product acceptance claim. The contained
device-free actor has separate evidence in batch034. These repeat the previously
established direct suites on the new source; they are not 35 new distinct cases.

Raw compiler logs retain their original whitespace. The runner/prompt record
the local frozen-source and SDK prerequisites; no output binaries are published.
