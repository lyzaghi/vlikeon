#!/usr/bin/env python3
"""
OmniPlay Corresponding Source Archival Utility
Addresses Work Item 97 (GPL / LGPL / MPL Compliance).

Packages exact source trees, adaptation patches, build scripts, and licenses
for all copyleft-licensed binaries distributed with OmniPlay.
"""

import os
import sys
import tarfile
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

COMPONENTS = [
    {
        "name": "omniplay-native-engines",
        "version": "1.0.0-omniplay",
        "license": "GPL-2.0-or-later AND MPL-2.0",
        "source_dir": "engines/native",
        "patches": ["browser/patches/0001-omniplay-browser.patch", "browser/patches/0002-omniplay-android.patch"],
        "build_script": "tools/build_desktop_core.py",
    },
    {
        "name": "bsnes",
        "version": "115-omniplay",
        "license": "GPL-3.0-only",
        "source_dir": "engines/native/bsnes",
        "patches": ["engines/native/patches/bsnes-remove-ipl-array.patch"],
        "build_script": "engines/native/build-bsnes.sh",
    },
    {
        "name": "beetle-psx",
        "version": "0.9.44.1-omniplay",
        "license": "GPL-2.0-only",
        "source_dir": "engines/native/beetle-psx",
        "patches": [],
        "build_script": "engines/native/build-beetle-psx.sh",
    },
    {
        "name": "mgba",
        "version": "0.10.3-omniplay",
        "license": "MPL-2.0",
        "source_dir": "engines/native/mgba",
        "patches": [],
        "build_script": "engines/native/build-mgba.sh",
    },
]

def file_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

def create_corresponding_source_archive(output_dir: Path) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    archive_path = output_dir / "omniplay-corresponding-source-v1.tar.gz"
    manifest_path = output_dir / "omniplay-corresponding-source-v1.json"

    print(f"[+] Creating corresponding source archive at {archive_path}...")
    manifest = {
        "schema_version": 1,
        "platform": "omniplay",
        "purpose": "GPL/LGPL/MPL compliance for distributed binaries",
        "components": [],
    }

    with tarfile.open(archive_path, "w:gz") as tar:
        # Add root license and build instructions
        readme_content = b"""# OmniPlay Corresponding Source Distribution
This archive contains the complete corresponding source code, build scripts,
and adaptation patches for all copyleft components included in OmniPlay.

Build instructions:
Each component directory contains its respective build script and upstream README.
To compile, follow the instructions in BUILD.md.
"""
        readme_info = tarfile.TarInfo(name="README.md")
        readme_info.size = len(readme_content)
        tar.addfile(readme_info, io_bytes(readme_content))

        for comp in COMPONENTS:
            src_root = ROOT / comp["source_dir"]
            archived_files = []
            missing_paths = []

            if src_root.exists() and src_root.is_dir():
                for item in sorted(src_root.rglob("*")):
                    if item.is_file() and not item.is_symlink():
                        rel = item.relative_to(ROOT).as_posix()
                        tar.add(item, arcname=rel, recursive=False)
                        archived_files.append({
                            "path": rel,
                            "bytes": item.stat().st_size,
                            "sha256": file_sha256(item),
                        })
            else:
                missing_paths.append(comp["source_dir"])

            for patch_rel in comp.get("patches", []):
                patch_path = ROOT / patch_rel
                if patch_path.is_file() and not patch_path.is_symlink():
                    tar.add(patch_path, arcname=patch_rel, recursive=False)
                    archived_files.append({
                        "path": patch_rel,
                        "bytes": patch_path.stat().st_size,
                        "sha256": file_sha256(patch_path),
                    })
                else:
                    missing_paths.append(patch_rel)

            build_rel = comp.get("build_script")
            if build_rel:
                build_path = ROOT / build_rel
                if build_path.is_file() and not build_path.is_symlink():
                    tar.add(build_path, arcname=build_rel, recursive=False)
                    archived_files.append({
                        "path": build_rel,
                        "bytes": build_path.stat().st_size,
                        "sha256": file_sha256(build_path),
                    })
                else:
                    missing_paths.append(build_rel)

            source_included = len(archived_files) > 0 and len(missing_paths) == 0
            comp_record = {
                "name": comp["name"],
                "version": comp["version"],
                "license": comp["license"],
                "source_included": source_included,
                "status": "archived" if source_included else "missing_upstream_checkout",
                "archived_file_count": len(archived_files),
                "files": archived_files,
                "missing_paths": missing_paths,
            }
            manifest["components"].append(comp_record)
            print(f"    Component: {comp['name']} ({comp['version']}) -> source_included={source_included} ({len(archived_files)} files)")

    archive_digest = file_sha256(archive_path)
    manifest["archive_sha256"] = archive_digest
    manifest["archive_bytes"] = archive_path.stat().st_size

    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)

    print(f"[+] Archive created successfully: {archive_digest}")
    print(f"[+] Manifest written to {manifest_path}")
    return archive_path

def io_bytes(b: bytes):
    import io
    return io.BytesIO(b)

def main():
    out_dir = Path("dist/source")
    create_corresponding_source_archive(out_dir)

if __name__ == "__main__":
    main()
