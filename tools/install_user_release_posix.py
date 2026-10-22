"""Install one signed OmniPlay release for the current GNU/Linux user.

The two stable bootstrap executables are admitted by explicit SHA-256 pins.
The updater then performs normal signed release/package admission.  Registration
is published only from the resulting active package inventory.  This installer
never elevates privilege, stops a live native host or restarts its delegated
cgroup; socket activation and the host's quiescent idle exit carry upgrades.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import platform

ORIGIN = "chrome-extension://knanejoacaeomhkfeciafoclmmnogdpg/"
SERVICE = "share/systemd/user/omniplay-native-host.service"
SOCKET = "share/systemd/user/omniplay-native-host.socket"
HOST = "libexec/omniplay-native-host"
PIN = re.compile(r"[0-9a-fA-F]{64}\Z")
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z")


def fail(message):
    raise SystemExit(message)


def xdg(name, fallback):
    value = os.environ.get(name)
    path = Path(value) if value else Path.home() / fallback
    if not path.is_absolute():
        fail(f"{name} must be an absolute path")
    return path


def ordinary_file(path, executable=False):
    try:
        shape = path.lstat()
    except OSError as error:
        fail(f"Required file is unavailable: {path}: {error}")
    if not stat.S_ISREG(shape.st_mode) or shape.st_nlink != 1:
        fail(f"Required input must be a single-link ordinary file: {path}")
    if executable and shape.st_mode & 0o111 == 0:
        fail(f"Required bootstrap is not executable: {path}")
    return shape


def digest(path):
    value = hashlib.sha256()
    with path.open("rb", buffering=0) as source:
        for block in iter(lambda: source.read(65536), b""):
            value.update(block)
    return value.hexdigest()


def private_directory(path):
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    shape = path.lstat()
    if not stat.S_ISDIR(shape.st_mode) or shape.st_uid != os.geteuid():
        fail(f"Managed directory is not an owned ordinary directory: {path}")
    os.chmod(path, 0o700)


def copy_pinned(source, destination, expected):
    if not PIN.fullmatch(expected):
        fail("Each bootstrap requires an explicit SHA-256 pin")
    ordinary_file(source, executable=True)
    expected = expected.lower()
    if destination.exists():
        ordinary_file(destination, executable=True)
        if digest(destination) != expected or digest(source) != expected:
            fail("A different or damaged stable bootstrap is already installed")
        return
    if digest(source) != expected:
        fail("A bootstrap input does not match its explicit SHA-256 pin")
    descriptor, temporary = tempfile.mkstemp(prefix=".omniplay-bootstrap-", dir=destination.parent)
    temporary = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb", buffering=0) as target, source.open("rb", buffering=0) as opened:
            os.fchmod(target.fileno(), 0o700)
            shutil.copyfileobj(opened, target, 65536)
            target.flush()
            os.fsync(target.fileno())
        if digest(temporary) != expected:
            fail("Bootstrap input changed while it was copied")
        os.link(temporary, destination)
        os.chmod(destination, 0o700)
    finally:
        temporary.unlink(missing_ok=True)


def invoke(program, *arguments):
    result = subprocess.run([str(program), *map(str, arguments)], stdin=subprocess.DEVNULL,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                            encoding="utf-8", timeout=300, check=False)
    if result.returncode != 0:
        fail(result.stderr.strip() or "The signed release updater rejected the operation")
    try:
        return json.loads(result.stdout)
    except (TypeError, json.JSONDecodeError):
        fail("The signed release updater returned an invalid response")


def active_release(updater):
    status = invoke(updater, "status")
    if status.get("platform") != "linux-x86_64" or not isinstance(status.get("releases"), list):
        fail("The updater is not configured for the GNU/Linux x86-64 release channel")
    active = [item for item in status["releases"] if item.get("status") == "active"]
    if len(active) != 1 or not UUID.fullmatch(str(active[0].get("id", ""))):
        fail("The installation does not have exactly one active signed release")
    return active[0]


def inventory(active):
    manifest = active.get("manifest")
    files = manifest.get("files") if isinstance(manifest, dict) else None
    if not isinstance(manifest, dict) or manifest.get("platform") != "linux-x86_64" or not isinstance(files, list):
        fail("The active release inventory is invalid")
    result = {}
    for item in files:
        name = item.get("path") if isinstance(item, dict) else None
        if not isinstance(name, str) or name in result:
            fail("The active release inventory has duplicate or invalid paths")
        result[name] = item
    for name in (SERVICE, SOCKET, HOST):
        item = result.get(name)
        if not item or not PIN.fullmatch(str(item.get("sha256", ""))):
            fail("The active release lacks its delegated native-host registration")
    if result[HOST].get("executable") is not True:
        fail("The active native-host entry is not executable")
    return result


def installed_file(root, active, item, name):
    path = root / "versions" / active["id"] / name
    shape = ordinary_file(path, executable=item.get("executable") is True)
    if shape.st_size != item.get("bytes") or digest(path) != item["sha256"]:
        fail(f"Installed release file failed inventory verification: {name}")
    return path


def systemd_argument(path):
    value = str(path)
    if any(ord(character) < 32 or ord(character) == 127 for character in value):
        fail("The stable launcher path contains a control character")
    value = value.replace("%", "%%").replace("\\", "\\\\").replace('"', '\\"')
    return f'"{value}"'


def publish(path, data, mode):
    private_directory(path.parent)
    if path.exists():
        shape = path.lstat()
        if not stat.S_ISREG(shape.st_mode) or shape.st_uid != os.geteuid() or shape.st_nlink != 1:
            fail(f"Refusing to replace an unsafe registration file: {path}")
        if path.read_bytes() == data:
            os.chmod(path, mode)
            return False
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}-", dir=path.parent)
    temporary = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb", buffering=0) as target:
            os.fchmod(target.fileno(), mode)
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
        return True
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap-directory", required=True, type=Path)
    parser.add_argument("--launcher-sha256", required=True)
    parser.add_argument("--updater-sha256", required=True)
    parser.add_argument("--release-envelope", required=True, type=Path)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--browser-data-directory", type=Path)
    args = parser.parse_args()
    if not sys.platform.startswith("linux") or platform.machine().lower() not in {"x86_64", "amd64"} or os.geteuid() == 0:
        fail("This installer requires an unprivileged GNU/Linux x86-64 user")
    os.umask(0o077)
    source_shape = args.bootstrap_directory.lstat()
    source = args.bootstrap_directory.resolve(strict=True)
    if not stat.S_ISDIR(source_shape.st_mode) or stat.S_ISLNK(source_shape.st_mode) or not source.is_dir():
        fail("The bootstrap source must be an ordinary directory")
    ordinary_file(args.release_envelope)
    ordinary_file(args.package)
    install_root = xdg("XDG_DATA_HOME", ".local/share") / "OmniPlay" / "install"
    bootstrap = install_root / "bootstrap"
    private_directory(bootstrap)
    launcher = bootstrap / "omniplay-launcher"
    updater = bootstrap / "omniplay-update"
    copy_pinned(source / launcher.name, launcher, args.launcher_sha256)
    copy_pinned(source / updater.name, updater, args.updater_sha256)

    staged = invoke(updater, "stage", args.release_envelope.resolve(strict=True), args.package.resolve(strict=True))
    if staged.get("status") == "staged":
        identity = staged.get("id")
        if not isinstance(identity, str) or not UUID.fullmatch(identity):
            fail("The updater returned an invalid staged release identity")
        invoke(updater, "activate", identity)
    elif staged.get("status") != "active":
        fail("The selected signed release is neither staged nor active")

    active = active_release(updater)
    files = inventory(active)
    service_source = installed_file(install_root, active, files[SERVICE], SERVICE)
    socket_source = installed_file(install_root, active, files[SOCKET], SOCKET)
    host = installed_file(install_root, active, files[HOST], HOST)
    service = service_source.read_text(encoding="utf-8")
    if (service.count("@OMNIPLAY_LAUNCHER@") != 1 or "[Install]" in service
            or service.splitlines().count("RefuseManualStart=yes") != 1
            or service.splitlines().count("RefuseManualStop=yes") != 1
            or any(line.startswith(("Requires=", "BindsTo=")) and "omniplay-native-host.socket" in line
                   for line in service.splitlines())):
        fail("The signed service template has an invalid launcher placeholder")
    service = service.replace("@OMNIPLAY_LAUNCHER@", systemd_argument(launcher)).encode("utf-8")
    socket = socket_source.read_bytes()
    if b"@OMNIPLAY_LAUNCHER@" in socket or len(service) > 65536 or len(socket) > 65536:
        fail("The signed systemd registration templates are invalid")
    units = xdg("XDG_CONFIG_HOME", ".config") / "systemd" / "user"
    publish(units / "omniplay-native-host.service", service, 0o600)
    publish(units / "omniplay-native-host.socket", socket, 0o600)

    browser = args.browser_data_directory.expanduser().resolve() if args.browser_data_directory else xdg("XDG_CONFIG_HOME", ".config") / "omniplay"
    manifest = json.dumps({"name": "com.omniplay.broker",
                           "description": "OmniPlay local game library and progress service",
                           "path": str(launcher), "type": "stdio", "allowed_origins": [ORIGIN]},
                          ensure_ascii=True, indent=2).encode("utf-8") + b"\n"
    publish(browser / "NativeMessagingHosts" / "com.omniplay.broker.json", manifest, 0o600)

    # Reloading definitions does not stop the already running service.  Enabling
    # the socket starts only its listener; the service is activated on demand.
    subprocess.run(["systemctl", "--user", "daemon-reload"], check=True, timeout=60)
    subprocess.run(["systemctl", "--user", "enable", "--now", "omniplay-native-host.socket"], check=True, timeout=60)
    if digest(host) != files[HOST]["sha256"]:
        fail("The active native host changed during registration")
    print("OmniPlay installed for the current GNU/Linux user; active native sessions were not restarted.")


if __name__ == "__main__":
    main()
