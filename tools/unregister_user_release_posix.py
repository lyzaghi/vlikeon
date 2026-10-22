"""Safely remove OmniPlay GNU/Linux per-user launch registrations.

Program versions, profiles, games, firmware and saves are retained.  The socket
is disabled first to prevent new native-host activation.  A running service is
never stopped: it must reach its own cgroup-aware quiescent exit before the
owned unit and native-messaging files can be removed.
"""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import time

SERVICE = "omniplay-native-host.service"
SOCKET = "omniplay-native-host.socket"
MANIFEST = "com.omniplay.broker.json"
ORIGIN = "chrome-extension://knanejoacaeomhkfeciafoclmmnogdpg/"


def fail(message):
    raise SystemExit(message)


def xdg(name, fallback):
    value = os.environ.get(name)
    path = Path(value) if value else Path.home() / fallback
    if not path.is_absolute():
        fail(f"{name} must be an absolute path")
    return path


def systemctl(*arguments, accepted=(0,)):
    result = subprocess.run(["systemctl", "--user", *arguments], stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                            encoding="utf-8", timeout=60, check=False)
    if result.returncode not in accepted:
        fail(result.stderr.strip() or "The user service manager rejected maintenance")
    return result


def active(unit):
    return systemctl("is-active", "--quiet", unit, accepted=(0, 3, 4)).returncode == 0


def owned_regular(path):
    try:
        shape = path.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(shape.st_mode) or shape.st_uid != os.geteuid() or shape.st_nlink != 1:
        fail(f"Refusing to remove an unsafe registration path: {path}")
    return path.read_bytes()


def remove_unit(path, identity):
    raw = owned_regular(path)
    if raw is None:
        return
    if len(raw) > 65536 or identity.encode("ascii") not in raw or b"OmniPlay" not in raw:
        fail(f"Refusing to remove a unit whose identity is not OmniPlay: {path}")
    path.unlink()


def remove_manifest(path, launcher):
    raw = owned_regular(path)
    if raw is None:
        return
    if len(raw) > 65536:
        fail("The native-messaging registration exceeds its bound")
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError):
        fail("The native-messaging registration is invalid")
    expected = {"name": "com.omniplay.broker",
                "description": "OmniPlay local game library and progress service",
                "path": str(launcher), "type": "stdio", "allowed_origins": [ORIGIN]}
    if value != expected:
        fail("Refusing to remove a native-messaging registration owned by another configuration")
    path.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser-data-directory", type=Path)
    parser.add_argument("--wait-seconds", type=int, default=420)
    args = parser.parse_args()
    if not sys.platform.startswith("linux") or os.geteuid() == 0:
        fail("This maintenance command requires an unprivileged GNU/Linux user")
    if not 300 <= args.wait_seconds <= 3600:
        fail("The quiescent wait must be between 300 and 3600 seconds")
    units = xdg("XDG_CONFIG_HOME", ".config") / "systemd" / "user"
    install = xdg("XDG_DATA_HOME", ".local/share") / "OmniPlay" / "install"
    launcher = install / "bootstrap" / "omniplay-launcher"
    browser = args.browser_data_directory.expanduser().resolve() if args.browser_data_directory else xdg("XDG_CONFIG_HOME", ".config") / "omniplay"

    service_running = active(SERVICE)
    if service_running:
        relations = systemctl("show", "--property=Requires", "--property=BindsTo", "--value", SERVICE)
        if SOCKET in relations.stdout.split():
            fail("The loaded service still binds its lifetime to the socket; update it before safe unregistration")
    # Stop only the activation socket.  The service unit has no Requires/BindsTo
    # propagation from the socket and keeps its inherited fd until it exits.
    systemctl("disable", "--now", SOCKET, accepted=(0, 1))
    deadline = time.monotonic() + args.wait_seconds
    while active(SERVICE) and time.monotonic() < deadline:
        time.sleep(5)
    if active(SERVICE):
        # Restore admission when an active/dying delegated tree prevents safe
        # completion.  No registration file has been touched at this point.
        systemctl("enable", "--now", SOCKET)
        fail("Active native work deferred unregistration; try again after sessions finish")
    if active(SOCKET):
        fail("The native-host socket remained active")

    remove_unit(units / SERVICE, "--linux-native-host-service")
    remove_unit(units / SOCKET, "ListenStream=%t/omniplay/native-host.sock")
    remove_manifest(browser / "NativeMessagingHosts" / MANIFEST, launcher)
    systemctl("daemon-reload")
    systemctl("reset-failed", SERVICE, SOCKET, accepted=(0, 1))
    print("OmniPlay launch registrations were removed; program versions and user data were retained.")


if __name__ == "__main__":
    main()
