"""Inventory a Linux desktop for item 53; never start capture or change routing.

Run as the logged-in desktop user, without sudo. This is prerequisite evidence,
not proof that game-window ownership, application-only audio or capture works.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys


def command(name, args):
    executable = shutil.which(name)
    if executable is None:
        return {"status": "missing"}
    try:
        result = subprocess.run(
            [executable, *args], stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            timeout=8, check=False, encoding="utf-8", errors="replace",
            env={**os.environ, "LC_ALL": "C"},
        )
    except subprocess.TimeoutExpired:
        return {"status": "timeout"}
    except OSError as error:
        return {"status": "unavailable", "error_type": type(error).__name__}
    return {"status": "ok" if result.returncode == 0 else "failed",
            "exit_code": result.returncode, "value": result.stdout[:4096].strip()}


def portal_property(name):
    result = command("gdbus", [
        "call", "--session", "--dest", "org.freedesktop.portal.Desktop",
        "--object-path", "/org/freedesktop/portal/desktop",
        "--method", "org.freedesktop.DBus.Properties.Get",
        "org.freedesktop.portal.ScreenCast", name,
    ])
    raw = result.pop("value", "")
    if result["status"] == "ok":
        match = re.fullmatch(r"\(<uint32 ([0-9]{1,10})>,\)", raw)
        if match and int(match[1]) <= 0xFFFFFFFF:
            result["value"] = int(match[1])
        else:
            result["status"] = "unexpected_response"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path,
                        help="New JSON file in an existing directory; never overwritten")
    parser.add_argument("--query-portal", action="store_true",
                        help="Read ScreenCast properties on the user session bus; may activate the portal service")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.exit(2, "This probe requires a Linux desktop; no report was created.\n")
    if os.geteuid() == 0:
        parser.exit(2, "Run as the desktop user without sudo; no report was created.\n")
    if args.output.exists() or not args.output.parent.is_dir():
        parser.exit(2, "Output must be new and its parent directory must exist.\n")
    session_type = os.environ.get("XDG_SESSION_TYPE", "")
    desktops = {part.lower() for part in os.environ.get("XDG_CURRENT_DESKTOP", "").split(":")}
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    pipewire_socket = False
    if runtime:
        try:
            pipewire_socket = stat.S_ISSOCK((Path(runtime) / "pipewire-0").stat().st_mode)
        except OSError:
            pass
    report = {
        "schema_version": 1, "item": 53, "kind": "prerequisite_inventory",
        "observed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "kernel": platform.release(), "architecture": platform.machine(),
        "session": {"type": session_type if session_type in {"x11", "wayland", "tty"} else "unknown",
                    "known_desktops": sorted(desktops & {"gnome", "kde", "sway", "xfce", "cinnamon", "mate", "lxqt"}),
                    "display_present": bool(os.environ.get("DISPLAY")),
                    "wayland_display_present": bool(os.environ.get("WAYLAND_DISPLAY")),
                    "session_bus_present": bool(os.environ.get("DBUS_SESSION_BUS_ADDRESS"))},
        "python_pidfd_api_present": hasattr(os, "pidfd_open"),
        "default_pipewire_socket_present": pipewire_socket,
        "tools_present": {name: shutil.which(name) is not None for name in
                          ("gdbus", "pipewire", "pw-cli", "wpctl", "pactl", "pkg-config", "cmake", "cc")},
        "versions": {}, "user_services": {},
        "portal": {"status": "not_queried"},
        "capture_executed": False, "audio_routing_changed": False,
        "acceptance": "not_established",
    }
    for name in ("pipewire", "pactl"):
        result = command(name, ["--version"])
        if "value" in result:
            result["value"] = result["value"].splitlines()[:3]
        report["versions"][name] = result
    for service in ("pipewire.service", "wireplumber.service", "xdg-desktop-portal.service"):
        result = command("systemctl", ["--user", "is-active", service])
        value = result.pop("value", "")
        result["state"] = value if value in {"active", "inactive", "failed", "activating", "deactivating", "unknown"} else "unknown"
        report["user_services"][service] = result
    if args.query_portal:
        report["portal"] = {name: portal_property(name) for name in
                            ("version", "AvailableSourceTypes", "AvailableCursorModes")}
    # No usernames, hostnames, socket addresses, windows, process lists, audio
    # node properties or environment contents are included in the report.
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(report, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print("Linux prerequisite inventory written. Capture acceptance is not established.")


if __name__ == "__main__":
    main()
