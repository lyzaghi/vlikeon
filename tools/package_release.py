"""Author an OmniPlay OPPK0001 package; does not execute or test product binaries.

An explicit inventory avoids generic archive extraction, symlinks, executable mode
inheritance and decompression bombs. Signing is a separate offline operator step.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import tempfile
from release_inventory import check_release_tree

MAX_BYTES = 2 * 1024**3
MAX_MANIFEST = 16 * 1024**2


def safe_path(value):
    if not value or len(value) > 240 or not value.isascii():
        raise ValueError("Unsafe package path")
    for part in value.split("/"):
        stem = part.split(".")[0].upper()
        if (not re.fullmatch(r"[A-Za-z0-9_+. -]+", part) or part in (".", "..")
                or part.endswith((".", " ")) or stem in {"CON", "PRN", "AUX", "NUL", "CLOCK$"}
                or re.fullmatch(r"(?:COM|LPT)[0-9]", stem)):
            raise ValueError("Unsafe package path")


def sha256_file(path):
    digest = hashlib.sha256()
    length = 0
    with path.open("rb") as source:
        for block in iter(lambda: source.read(65536), b""):
            digest.update(block)
            length += len(block)
    return digest.hexdigest(), length


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--identity", required=True, type=Path,
                        help="JSON: channel, platform, version, Chromium revision, schema range and entry points")
    parser.add_argument("--executables", required=True, type=Path, help="JSON array of explicitly approved executable paths")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    base = args.directory.resolve(strict=True)
    if not base.is_dir() or args.directory.is_symlink():
        raise ValueError("Package directory must be a real directory")
    output = args.output.resolve()
    if output.is_relative_to(base) or output.exists():
        raise ValueError("Choose a new output path outside the package source tree")
    identity = json.loads(args.identity.read_text(encoding="utf-8"))
    fields = {"channel", "platform", "version", "chromium_revision", "minimum_data_schema",
              "maximum_data_schema", "browser_entry", "broker_entry"}
    if set(identity) != fields:
        raise ValueError("The explicit package identity fields are incomplete or unknown")
    executables = set(json.loads(args.executables.read_text(encoding="utf-8")))
    for name in executables:
        safe_path(name)
    entries = []
    seen = set()
    total = 0
    for directory, dirs, files in os.walk(base, followlinks=False):
        for name in dirs + files:
            path = Path(directory) / name
            if path.is_symlink() or path.is_junction():
                raise ValueError("Links and junctions are not permitted in release packages")
        for name in sorted(files):
            path = Path(directory) / name
            if not path.is_file():
                raise ValueError("Special files cannot be packaged")
            relative = path.relative_to(base).as_posix()
            safe_path(relative)
            if relative.lower() in seen:
                raise ValueError("Case-insensitive package path collision")
            seen.add(relative.lower())
            digest, length = sha256_file(path)
            total += length
            if total > MAX_BYTES:
                raise ValueError("Unpacked package exceeds its bound")
            entries.append({"path": relative, "bytes": length, "sha256": digest,
                            "executable": relative in executables})
    entries.sort(key=lambda entry: entry["path"])
    if not entries or len(entries) > 100000 or executables - {entry["path"] for entry in entries}:
        raise ValueError("File count or executable inventory is invalid")
    for name in (identity["browser_entry"], identity["broker_entry"]):
        if name not in executables:
            raise ValueError("Both entry points must be approved executable files")
    if identity["platform"] == "linux-x86_64":
        required = {"libexec/omniplay-native-host", "share/systemd/user/omniplay-native-host.service",
                    "share/systemd/user/omniplay-native-host.socket"}
        if not required <= {entry["path"] for entry in entries} or "libexec/omniplay-native-host" not in executables:
            raise ValueError("Linux packages must include the delegated native-host executable and user units")
        service = (base / "share/systemd/user/omniplay-native-host.service").read_text(encoding="utf-8")
        socket = (base / "share/systemd/user/omniplay-native-host.socket").read_text(encoding="utf-8")
        route = "ExecStart=@OMNIPLAY_LAUNCHER@ --linux-native-host-service"
        if (len(service.encode()) > 65536 or service.splitlines().count(route) != 1
                or service.splitlines().count("RefuseManualStart=yes") != 1
                or service.splitlines().count("RefuseManualStop=yes") != 1
                or service.count("@OMNIPLAY_LAUNCHER@") != 1 or "[Install]" in service
                or any(line.startswith(("ExecStartPre=", "ExecStartPost=", "ExecStop="))
                       or (line.startswith(("Requires=", "BindsTo=")) and "omniplay-native-host.socket" in line)
                       for line in service.splitlines())):
            raise ValueError("Linux native-host service template has an unsafe launch route")
        if (len(socket.encode()) > 65536 or socket.splitlines().count("ListenStream=%t/omniplay/native-host.sock") != 1
                or socket.splitlines().count("Service=omniplay-native-host.service") != 1
                or "@OMNIPLAY_LAUNCHER@" in socket or any(line.startswith("Exec") for line in socket.splitlines())):
            raise ValueError("Linux native-host socket template is invalid")
    if not {"licenses/third_party_notices.txt", "licenses/source_offer.json"} <= seen:
        raise ValueError("Include third-party notices and corresponding-source declarations")
    offer = json.loads((base / "licenses/source_offer.json").read_text(encoding="utf-8"))
    if offer.get("schema_version") != 1 or not offer.get("components"):
        raise ValueError("Corresponding-source declaration must list the shipped components")
    for component in offer["components"]:
        if not all(component.get(key) for key in ("name", "revision", "license")):
            raise ValueError("Shipped component identity is incomplete")
        if component.get("corresponding_source_required") and not component.get("source_archive_sha256"):
            raise ValueError("Copyleft source must identify an exact accompanying source archive")
    release_findings=check_release_tree(base,entries)
    if release_findings:
        raise ValueError("Release inventory rejected: "+json.dumps(release_findings,ensure_ascii=True))
    manifest = {"schema_version": 1, **identity, "files": entries}
    raw = json.dumps(manifest, ensure_ascii=True, separators=(",", ":")).encode("utf-8")
    if len(raw) > MAX_MANIFEST:
        raise ValueError("Package inventory exceeds its bound")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, prefix=".omniplay-package-", delete=False) as target:
            temporary = Path(target.name)
            target.write(b"OPPK0001" + struct.pack("<I", len(raw)) + raw)
            for entry in entries:
                digest = hashlib.sha256()
                count = 0
                with (base / entry["path"]).open("rb") as source:
                    for block in iter(lambda: source.read(65536), b""):
                        target.write(block)
                        count += len(block)
                        digest.update(block)
                if count != entry["bytes"] or digest.hexdigest() != entry["sha256"]:
                    raise ValueError("Source changed while authoring the package")
            if target.tell() > MAX_BYTES:
                raise ValueError("Package including its inventory exceeds its bound")
            target.flush()
            os.fsync(target.fileno())
        # Publish without replacing an existing artifact, on the same filesystem.
        os.link(temporary, output)
        digest, count = sha256_file(output)
        print(json.dumps({"artifact_path": str(output), "artifact_sha256": digest,
                          "artifact_bytes": count, "identity": identity}, indent=2))
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


if __name__ == "__main__":
    main()
