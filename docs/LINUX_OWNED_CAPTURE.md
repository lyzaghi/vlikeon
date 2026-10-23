# Item 53 — Linux game-window and application-audio capture

Status: open. There is no Linux owned-capture adapter in the current product.
The host probe below is prerequisite tooling, not an implementation or a
capture acceptance result. Streaming policy remains disabled.

On 2026-10-23 the owner deferred Linux build/runtime work and directed this chat
to the next numbered item, macOS capture (54). Retain all implementation and
runtime gaps below. No Linux build, SSH login or VM installation is being run.

## Current source boundaries

`platform/broker/src/stream_input.rs::capture_available` is Windows-only.
`prepare_capture` refuses non-Windows targets; the retained child-handle
accessor in `platform/core/src/installed.rs` is also Windows-only. Chromium
patch `0003-owned-game-capture.patch` checks HWND/PID/creation FILETIME and
rejects other platforms. The private component worker in
`apps/game-hub/background.js` expects this Windows process/window binding.
Generic display capture in `engines/streaming/capture.js` does not establish
which locally launched game owns a selected Linux window or audio source.
Enabling a capability flag cannot supply those missing adapters.

## Required implementation and evidence

1. Retain the launched Linux child through a pidfd and validated process
   lifetime. Keep process/window/node identities private to the host. A title,
   renderer PID, app ID or unverified audio-node property alone is insufficient
   ownership evidence. Launcher descendants retain their separate item 57.
2. Add native local source consent and a window-only capture path for the
   supported X11 and Wayland backends. Connect the selected source to the
   broker-owned game through backend-supported identity evidence. A compositor
   lacking that evidence must not advertise the owned path as supported.
3. Consume the portal's granted PipeWire remote and selected stream without
   broadening its permission. Revalidate stream lifetime/identity; refuse a
   monitor, another window, stale source and reused numeric node identity.
4. Add application-only audio with explicit local consent and verified source
   binding. Do not substitute a default sink monitor, microphone or system mix.
   Handle multiple game streams, format/rate changes, device loss and game exit.
   Video-only operation remains available only when explicitly selected.
5. Integrate bounded frame/audio delivery with the product capture/encode and
   WebRTC paths. Retire real streams, descriptors, portal sessions and input
   grants on cancellation, expiry, page/broker exit and source loss. Independent
   encode-service and codec work remains item 56; this item must provide usable
   media through the supported product path rather than a disconnected sample.
6. Antigravity executes frozen tests on a Linux desktop with real synthetic
   game windows/audio. Codex reviews and fixes. Verify two distinguishable
   applications: only the selected game's pixels/audio are shared. Exercise
   cancellation, source close/recreate, process exit/reuse, PipeWire/portal loss,
   reconnect, concurrent prepare and cleanup. Record desktop/compositor/kernel,
   portal/PipeWire versions, actual media observations and exact source hashes.

The [ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html)
provides session/source selection and a granted PipeWire remote. Its documented
stream metadata does not itself supply a retained game-process ownership proof.
Version 6 deprecates numeric node IDs for targeting in favor of
`pipewire-serial`/`PW_KEY_TARGET_OBJECT`; older targets need an explicit lifetime
strategy. The [PipeWire loopback module](https://docs.pipewire.org/page_module_loopback.html)
can connect audio streams and sinks, but that capability alone is not evidence
that captured audio belongs exclusively to the authorized game.

## Host prerequisite inventory

The current Windows host reports WSL is not installed. No Linux desktop or SSH
target has been supplied. The owner has been asked to select an existing Linux
host or a local VM; no environment installation or remote login is assumed.

`qa/linux/capture-host-probe.py` gathers a small report as the logged-in Linux
desktop user, without sudo. It reports session type, known desktop family,
tool/service presence and versions. It does not collect usernames, hostnames,
environment values, process/window lists, audio graph properties or profiles.
The optional portal query reads three ScreenCast properties and may activate
the user's portal service. It does not select a source, open media, change
audio links or modify system configuration.

```sh
python3 qa/linux/capture-host-probe.py --query-portal --output /path/to/new-host-report.json
```

Use a new output file in an existing directory. Non-Linux/root execution is
refused. This script has not yet been executed on Linux. Tool presence,
`pidfd_open` API presence or an advertised window bit does not establish
functional capture, ownership, audio isolation or item completion. Subsequent
runtime results must be added to the acceptance record before closing item 53.

Antigravity batch `053a` passed Python syntax parsing and actual non-Linux
refusal on Windows, with no Linux report created. The frozen probe remained
unchanged. Evidence is in `qa/evidence/sequential/item-053/`; these two checks
do not test the Linux path, portal interaction or media capture.
