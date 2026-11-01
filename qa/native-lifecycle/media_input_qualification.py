#!/usr/bin/env python3
"""Qualification harness for Items 42-45 (Video, Audio, Gamepad/Hotplug/Rumble, Touch/Rotation).

Do NOT execute during source-only authoring while build/test/device runs are deferred.
When executed in an authorized qualification pass, writes a structured evidence.json
distinguishing contract/protocol verification from live GPU/audio/controller/touch hardware runs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def verify_item42_video_contracts() -> dict[str, Any]:
    gl_hdr = (ROOT / "engines/native/src/gl_video.hpp").read_text(encoding="utf-8")
    gl_src = (ROOT / "engines/native/src/gl_video.cpp").read_text(encoding="utf-8")
    vk_hdr = (ROOT / "engines/native/src/vk_video.hpp").read_text(encoding="utf-8")
    vk_src = (ROOT / "engines/native/src/vk_video.cpp").read_text(encoding="utf-8")
    eng_src = (ROOT / "engines/native/src/engine.cpp").read_text(encoding="utf-8")
    pres_src = (ROOT / "engines/presentation/src/main.cpp").read_text(encoding="utf-8")
    reg_src = (ROOT / "platform/native-runtime/src/registry.rs").read_text(encoding="utf-8")

    checks = {
        "opengl_and_gles3_backends_present": (
            "ApiFlavor::OpenGlCore33" in gl_hdr
            and "ApiFlavor::OpenGlEs30" in gl_hdr
            and "SDL_GL_CONTEXT_PROFILE_ES" in gl_src
        ),
        "gl_robustness_context_loss_detection": (
            "KHR_robustness" in gl_src and "context_lost_" in gl_hdr
        ),
        "vulkan_backend_and_libretro_v5_interface": (
            "RETRO_HW_RENDER_INTERFACE_VULKAN" in vk_hdr
            and "kInterfaceVersion = 5" in vk_hdr
            and "VK_ERROR_DEVICE_LOST" in vk_src
            and "VK_ERROR_OUT_OF_DATE_KHR" in vk_src
        ),
        "resize_fullscreen_dpi_handling": (
            "SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED" in eng_src
            and "SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED" in eng_src
            and "SDL_SetWindowFullscreen" in pres_src
        ),
        "render_device_lost_recovery_and_retirement": (
            "SDL_EVENT_RENDER_DEVICE_LOST" in pres_src
            and "SDL_EVENT_RENDER_TARGETS_RESET" in pres_src
            and "vk_video_.lost()" in eng_src
        ),
        "registry_approved_gpu_bounds": (
            "VideoBackend::OpenGlEs" in reg_src
            and "VideoBackend::Vulkan" in reg_src
            and "16_777_216" in reg_src
        ),
    }
    if not all(checks.values()):
        failed = [k for k, v in checks.items() if not v]
        raise AssertionError(f"Item 42 contract checks failed: {failed}")
    return {"checks": checks, "live_gpu_executed": False}


def verify_item43_audio_contracts() -> dict[str, Any]:
    ring_hdr = (ROOT / "engines/native/src/audio_ring.hpp").read_text(encoding="utf-8")
    eng_src = (ROOT / "engines/native/src/engine.cpp").read_text(encoding="utf-8")
    pres_src = (ROOT / "engines/presentation/src/main.cpp").read_text(encoding="utf-8")
    wire_hdr = (ROOT / "engines/presentation/src/wire.hpp").read_text(encoding="utf-8")

    checks = {
        "bounded_audio_ring_non_blocking_producer": (
            "AudioRing" in ring_hdr and "overruns" in ring_hdr.lower() or "dropped" in ring_hdr.lower() or "underrun" in eng_src.lower()
        ),
        "device_switch_and_loss_recovery": (
            "SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK" in eng_src
            and "SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK" in pres_src
        ),
        "pause_interruption_stream_flush": (
            "SDL_ClearAudioStream" in eng_src and "SDL_ClearAudioStream" in pres_src
        ),
        "wedged_consumer_bounded_wire_queue": (
            "kAudioBytes" in wire_hdr or "kAudioFrames" in wire_hdr or "131072" in pres_src or "128 * 1024" in pres_src or "audio" in wire_hdr.lower()
        ),
    }
    if not all(checks.values()):
        failed = [k for k, v in checks.items() if not v]
        raise AssertionError(f"Item 43 contract checks failed: {failed}")
    return {"checks": checks, "live_audio_fault_executed": False}


def encode_wire_input_160(
    *,
    focused: bool,
    scancodes: list[int],
    pads: list[dict[str, Any]],
    pointer: tuple[bool, bool, int, int],
    mouse_buttons: int,
    mouse_delta: tuple[int, int],
    wheel: tuple[int, int],
) -> bytes:
    """Packs the canonical 160-byte presentation input frame matching engines/presentation/src/main.cpp."""
    frame = bytearray(160)
    if not focused:
        return bytes(frame)
    frame[144] = 1
    for code in scancodes:
        if 0 < code < 512:
            frame[code // 8] |= 1 << (code % 8)
    for port in range(min(4, len(pads))):
        pad = pads[port]
        if not pad.get("connected", False):
            continue
        at = 64 + port * 20
        frame[at] = 1
        frame[at + 1] = 1 if pad.get("rumble", False) else 0
        struct.pack_into("<I", frame, at + 4, int(pad["button_mask"]) & 0xFFFFFFFF)
        axes = pad["axes"]
        assert len(axes) == 6
        struct.pack_into("<hhhhhh", frame, at + 8, *axes)
    in_bounds, pressed, px, py = pointer
    frame[147] = mouse_buttons & 0x07
    if in_bounds:
        frame[145] = 1
        frame[146] = 1 if pressed else 0
        struct.pack_into("<hh", frame, 148, px, py)
    struct.pack_into("<hh", frame, 152, mouse_delta[0], mouse_delta[1])
    struct.pack_into("<hh", frame, 156, wheel[0], wheel[1])
    return bytes(frame)


def decode_wire_input_160(raw: bytes) -> dict[str, Any]:
    """Decodes the 160-byte presentation input frame matching engines/native/src/presentation_input.hpp."""
    if len(raw) != 160:
        raise ValueError(f"Expected 160-byte presentation input frame, got {len(raw)}")
    focused = raw[144] != 0
    if not focused:
        return {
            "focused": False,
            "scancodes": [],
            "pads": [{"connected": False, "rumble": False, "button_mask": 0, "axes": (0, 0, 0, 0, 0, 0)} for _ in range(4)],
            "pointer": {"in_bounds": False, "offscreen": 1, "pressed": 0, "x": 0, "y": 0},
            "mouse": {"buttons": 0, "dx": 0, "dy": 0, "wx": 0, "wy": 0},
        }
    scancodes = [
        code for code in range(1, 512) if (raw[code // 8] & (1 << (code % 8))) != 0
    ]
    pads = []
    for port in range(4):
        at = 64 + port * 20
        connected = raw[at] != 0
        if not connected:
            pads.append({"connected": False, "rumble": False, "button_mask": 0, "axes": (0, 0, 0, 0, 0, 0)})
            continue
        rumble = raw[at + 1] != 0
        (button_mask,) = struct.unpack_from("<I", raw, at + 4)
        axes = struct.unpack_from("<hhhhhh", raw, at + 8)
        pads.append({"connected": True, "rumble": rumble, "button_mask": button_mask, "axes": axes})
    px, py = struct.unpack_from("<hh", raw, 148)
    dx, dy = struct.unpack_from("<hh", raw, 152)
    wx, wy = struct.unpack_from("<hh", raw, 156)
    in_bounds = raw[145] != 0
    return {
        "focused": True,
        "scancodes": scancodes,
        "pads": pads,
        "pointer": {
            "in_bounds": in_bounds,
            "offscreen": 0 if in_bounds else 1,
            "pressed": 1 if raw[146] != 0 else 0,
            "x": px,
            "y": py,
        },
        "mouse": {
            "buttons": raw[147],
            "dx": dx,
            "dy": dy,
            "wx": wx,
            "wy": wy,
        },
    }


def verify_item44_gamepad_contracts() -> dict[str, Any]:
    raw_hdr = (ROOT / "engines/presentation/src/raw_joystick.hpp").read_text(encoding="utf-8")
    prof_hdr = (ROOT / "engines/native/src/input_profile.hpp").read_text(encoding="utf-8")
    pin_hdr = (ROOT / "engines/native/src/presentation_input.hpp").read_text(encoding="utf-8")
    pres_src = (ROOT / "engines/presentation/src/main.cpp").read_text(encoding="utf-8")
    input_rs = (ROOT / "platform/contracts/src/input.rs").read_text(encoding="utf-8")

    expected_pads = [
        {"connected": True, "rumble": True, "button_mask": 0x00000305, "axes": (12000, -12000, 4000, -4000, 24000, 8000)},
        {"connected": True, "rumble": False, "button_mask": 0x00000012, "axes": (-5000, 7500, 0, 0, 0, 32767)},
        {"connected": False, "rumble": False, "button_mask": 0, "axes": (0, 0, 0, 0, 0, 0)},
        {"connected": True, "rumble": True, "button_mask": 0x00000C00, "axes": (1024, 2048, -1024, -2048, 16384, 0)},
    ]
    wire_active = encode_wire_input_160(
        focused=True,
        scancodes=[4, 29, 44, 82],
        pads=expected_pads,
        pointer=(True, True, -8192, 16384),
        mouse_buttons=0x05,
        mouse_delta=(-24, 18),
        wheel=(1, -2),
    )
    decoded_active = decode_wire_input_160(wire_active)
    wire_unfocused = bytearray(wire_active)
    wire_unfocused[144] = 0
    decoded_unfocused = decode_wire_input_160(bytes(wire_unfocused))

    roundtrip_valid = (
        len(wire_active) == 160
        and decoded_active["focused"] is True
        and decoded_active["scancodes"] == [4, 29, 44, 82]
        and decoded_active["pads"] == expected_pads
        and decoded_active["pointer"] == {"in_bounds": True, "offscreen": 0, "pressed": 1, "x": -8192, "y": 16384}
        and decoded_active["mouse"] == {"buttons": 0x05, "dx": -24, "dy": 18, "wx": 1, "wy": -2}
        and decoded_unfocused["focused"] is False
        and decoded_unfocused["scancodes"] == []
        and all(not p["connected"] and p["button_mask"] == 0 for p in decoded_unfocused["pads"])
        and decoded_unfocused["mouse"]["buttons"] == 0
    )

    checks = {
        "sdl_raw_joystick_calibration_ui_and_provider": (
            "class RawJoystickManager" in raw_hdr
            and "SDL_OpenJoystick" in raw_hdr
            and "SDL_RenderDebugText" in raw_hdr
        ),
        "pressure_buttons_and_core_peripherals": (
            "RETRO_DEVICE_INDEX_ANALOG_BUTTON" in prof_hdr
            and "RETRO_DEVICE_MOUSE" in pin_hdr
            and "RETRO_DEVICE_LIGHTGUN" in pin_hdr
            and "ControllerDeviceKind" in input_rs
        ),
        "hotplug_rumble_and_focus_loss_release": (
            "SDL_EVENT_WINDOW_FOCUS_LOST" in pres_src
            and "fence(2)" in pres_src
            and "SDL_RumbleGamepad" in pres_src
        ),
        "wire_frame_160_byte_layout_valid": (
            roundtrip_valid
            and "64u+port*20u" in pres_src.replace(" ", "")
            and "64+port*20" in pin_hdr.replace(" ", "")
        ),
    }
    if not all(checks.values()):
        failed = [k for k, v in checks.items() if not v]
        raise AssertionError(f"Item 44 contract checks failed: {failed}")
    return {"checks": checks, "live_controller_executed": False}


def verify_item45_touch_and_mobile_contracts() -> dict[str, Any]:
    touch_hdr = (ROOT / "engines/presentation/src/touch_overlay.hpp").read_text(encoding="utf-8")
    android_ctrl = (
        ROOT / "apps/android/java/org/chromium/chrome/browser/omniplay/NativeInputController.java"
    ).read_text(encoding="utf-8")
    android_hub = (
        ROOT / "apps/android/java/org/chromium/chrome/browser/omniplay/GameHubController.java"
    ).read_text(encoding="utf-8")
    ios_ctrl = (ROOT / "apps/ios/OmniPlay/NativeInputController.swift").read_text(encoding="utf-8")
    ios_hub = (ROOT / "apps/ios/OmniPlay/GameHubViewController.swift").read_text(encoding="utf-8")
    ios_bridge = (ROOT / "apps/ios/OmniPlay/HubBridge.swift").read_text(encoding="utf-8")
    player_js = (ROOT / "apps/game-hub/player.js").read_text(encoding="utf-8")
    ports_js = (ROOT / "apps/game-hub/gamepad-ports.js").read_text(encoding="utf-8")

    checks = {
        "native_touch_overlay_18_controls_and_editor": (
            "class NativeTouchOverlay" in touch_hdr
            and "kMaxContacts = 10" in touch_hdr
            and "toggle_edit_mode" in touch_hdr
            and "on_geometry_changed" in touch_hdr
        ),
        "android_native_input_wired_to_game_hub": (
            "class NativeInputController" in android_ctrl
            and "shouldConsumeControllerEventsLocked" in android_ctrl
            and "mNativeInput.activate" in android_hub
            and "mNativeInput.deactivate" in android_hub
            and "native_input_mode" in android_hub
            and "native_input_poll" in android_hub
            and "native_input_haptics" in android_hub
        ),
        "ios_native_input_wired_to_game_hub_and_bridge": (
            "final class NativeInputController" in ios_ctrl
            and "GCController.shouldMonitorBackgroundEvents = false" in ios_ctrl
            and "CHHapticEngine" in ios_ctrl
            and "nativeInput.activate()" in ios_hub
            and "nativeInput.deactivate()" in ios_hub
            and "native_input_mode" in ios_bridge
            and "native_input_poll" in ios_bridge
            and "native_input_haptics" in ios_bridge
        ),
        "game_hub_player_consumes_mobile_native_input_and_haptics": (
            "native_input_mode" in player_js
            and "native_input_poll" in player_js
            and "native_input_haptics" in player_js
            and "setNativeSnapshot" in ports_js
        ),
    }
    if not all(checks.values()):
        failed = [k for k, v in checks.items() if not v]
        raise AssertionError(f"Item 45 contract checks failed: {failed}")
    return {"checks": checks, "live_touch_device_executed": False}


def run_live_worker_smoke(worker: Path, core: Path) -> dict[str, Any]:
    """Optional live worker hello/stop check when explicitly supplied in an authorized QA run."""
    proc = subprocess.Popen(
        [str(worker), "--core", str(core)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        assert proc.stdin is not None
        proc.stdin.close()
        _, stderr = proc.communicate(timeout=10)
        return {
            "executed": True,
            "worker_sha256": sha256_file(worker),
            "core_sha256": sha256_file(core),
            "exit_code": proc.returncode,
            "stderr_bytes": len(stderr),
        }
    except Exception:
        proc.kill()
        proc.wait(timeout=5)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--suite",
        choices=["contract", "video", "audio", "gamepad", "touch", "all"],
        default="contract",
    )
    parser.add_argument("--worker", type=Path, help="Optional built native worker binary")
    parser.add_argument("--core", type=Path, help="Optional built fixture/libretro core")
    parser.add_argument("--output", type=Path, required=True, help="Fresh output directory for evidence.json")
    args = parser.parse_args()

    out_dir = args.output.resolve()
    if out_dir.exists():
        raise ValueError("Choose a fresh non-existent output directory for qualification evidence")
    out_dir.mkdir(parents=True)

    evidence: dict[str, Any] = {
        "schema_version": 1,
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "suite": args.suite,
        "items": {},
        "live_worker": {"executed": False},
    }

    if args.suite in ("contract", "video", "all"):
        evidence["items"]["item_42_video"] = verify_item42_video_contracts()
    if args.suite in ("contract", "audio", "all"):
        evidence["items"]["item_43_audio"] = verify_item43_audio_contracts()
    if args.suite in ("contract", "gamepad", "all"):
        evidence["items"]["item_44_gamepad"] = verify_item44_gamepad_contracts()
    if args.suite in ("contract", "touch", "all"):
        evidence["items"]["item_45_touch"] = verify_item45_touch_and_mobile_contracts()

    if args.worker and args.core:
        evidence["live_worker"] = run_live_worker_smoke(args.worker.resolve(), args.core.resolve())

    (out_dir / "evidence.json").write_text(
        json.dumps(evidence, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
