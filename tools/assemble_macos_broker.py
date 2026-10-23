"""Assemble the explicit broker app/XPC layout without running or signing code.

Run before the operator's final app signing and notarization. The release package
must inventory the final signed bytes, including nested seals and both catalogs.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import stat

IDENTIFIER = re.compile(r"[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+){2,}\Z")


def ordinary(path):
    value = path.lstat()
    if (not stat.S_ISREG(value.st_mode) or value.st_nlink != 1
            or value.st_mode & (stat.S_ISUID | stat.S_ISGID)):
        raise ValueError("macOS bundle inputs must be ordinary single-link files")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--broker", type=Path, required=True)
    parser.add_argument("--owner", type=Path, required=True)
    parser.add_argument("--owner-id", required=True)
    parser.add_argument("--broker-id", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    for value in (args.owner_id, args.broker_id):
        if len(value) > 160 or not IDENTIFIER.fullmatch(value):
            raise ValueError("Explicit reverse-DNS signing identifiers are required")
    if args.owner_id == args.broker_id or not re.fullmatch(r"[0-9]+(?:\.[0-9]+){1,2}", args.version):
        raise ValueError("Distinct broker/owner identities and a bundle version are required")
    output = args.output.absolute()
    if output.name != "VlikeonBroker.app" or output.exists():
        raise ValueError("Use a new VlikeonBroker.app output path")
    for path in (args.owner.absolute(), args.broker.absolute(), output.parent):
        for parent in (path, *path.parents):
            if parent.is_symlink() or getattr(parent, "is_junction", lambda: False)():
                raise ValueError("macOS bundle ancestry cannot contain links")
    ordinary(args.broker)
    if not args.owner.is_dir() or output.is_relative_to(args.owner.absolute()):
        raise ValueError("The explicit XPC owner input must be a separate bundle")
    info_path = args.owner / "Contents/Info.plist"
    if ordinary(info_path).st_size > 65536:
        raise ValueError("The XPC owner property list exceeds its bound")
    info = plistlib.loads(info_path.read_bytes())
    if (info.get("CFBundleIdentifier") != args.owner_id
            or info.get("CFBundleExecutable") != "omniplay-macos-owner"
            or info.get("CFBundlePackageType") != "XPC!"
            or not info.get("OmniPlayBrokerRequirement")):
        raise ValueError("The XPC owner bundle identity or broker requirement is absent")
    required = {"Contents/MacOS/omniplay-macos-owner", "Contents/Resources/runtime-catalog.json",
                "Contents/_CodeSignature/CodeResources", "Contents/Info.plist"}
    files = []
    seen = set()
    total = 0
    for directory, dirs, names in os.walk(args.owner, followlinks=False):
        for name in dirs:
            path = Path(directory) / name
            if path.is_symlink() or getattr(path, "is_junction", lambda: False)():
                raise ValueError("Nested XPC bundle directories cannot be links")
        for name in names:
            path = Path(directory) / name
            size = ordinary(path).st_size
            relative = path.relative_to(args.owner).as_posix()
            if relative.lower() in seen or not relative.isascii() or len(relative) > 240:
                raise ValueError("The XPC bundle has a conflicting or oversized path")
            seen.add(relative.lower())
            total += size
            if len(files) >= 8192 or total > 2 * 1024**3:
                raise ValueError("The XPC bundle exceeds its finite inventory budget")
            files.append((path, relative))
    if not required <= {relative for _, relative in files}:
        raise ValueError("The signed XPC bundle is incomplete")
    catalog_path = args.owner / "Contents/Resources/runtime-catalog.json"
    if ordinary(catalog_path).st_size > 1024 * 1024:
        raise ValueError("The owner runtime catalog exceeds its bound")
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    if (set(catalog) != {"schema_version", "runtimes"} or catalog["schema_version"] != 2
            or not isinstance(catalog["runtimes"], list) or not 1 <= len(catalog["runtimes"]) <= 32
            or info.get("LSMinimumSystemVersion") != "13.0"):
        raise ValueError("Only the macOS 13 diskless guest owner layout is supported")
    # Validate every input before creating the reviewable unsigned app output.
    contents = output / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    broker = contents / "MacOS/omniplay-broker"
    shutil.copyfile(args.broker, broker)
    broker.chmod(0o755)
    nested = contents / "XPCServices" / (args.owner_id + ".xpc")
    for source, relative in files:
        target = nested / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o755 if relative.startswith("Contents/MacOS/") else stat.S_IMODE(source.stat().st_mode) & 0o755)
    properties = {"CFBundleIdentifier": args.broker_id, "CFBundleExecutable": "omniplay-broker",
                  "CFBundleName": "Vlikeon Broker", "CFBundlePackageType": "APPL",
                  "CFBundleVersion": args.version, "CFBundleShortVersionString": args.version,
                  "LSMinimumSystemVersion": "13.0", "LSBackgroundOnly": True}
    (contents / "Info.plist").write_bytes(plistlib.dumps(properties, sort_keys=True))
    print("Assembled broker app source layout. Final signing, notarization and package inventory are still required.")


if __name__ == "__main__":
    main()
