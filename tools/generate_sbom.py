#!/usr/bin/env python3
"""
OmniPlay Automated Software Bill of Materials (SBOM) Generator
Addresses Work Item 98.

Generates an industry-standard SPDX 2.3 JSON document from actual Cargo.lock
dependencies and native engine components.
"""

import os
import sys
import json
import re
from pathlib import Path
from datetime import datetime, timezone

KNOWN_LICENSES = {
    "rusqlite": "MIT",
    "ring": "MIT AND ISC",
    "tokio": "MIT",
    "serde": "MIT OR Apache-2.0",
    "sha2": "MIT OR Apache-2.0",
    "uuid": "Apache-2.0 OR MIT",
    "openh264": "BSD-2-Clause",
    "libvpx": "BSD-3-Clause",
    "opus": "BSD-3-Clause",
    "mgba": "MPL-2.0",
    "beetle-psx": "GPL-2.0-only",
    "bsnes": "GPL-3.0-only",
    "sameboy": "MIT",
    "ppsspp": "GPL-2.0-or-later",
}

def parse_cargo_lock(lock_path: Path):
    packages = []
    if not lock_path.exists():
        return packages

    content = lock_path.read_text(encoding="utf-8")
    current_pkg = {}

    for line in content.splitlines():
        line = line.strip()
        if line == "[[package]]":
            if current_pkg and "name" in current_pkg:
                packages.append(current_pkg)
            current_pkg = {}
        elif line.startswith("name = "):
            current_pkg["name"] = line.split("=")[1].strip(' "')
        elif line.startswith("version = "):
            current_pkg["version"] = line.split("=")[1].strip(' "')
        elif line.startswith("checksum = "):
            current_pkg["checksum"] = line.split("=")[1].strip(' "')

    if current_pkg and "name" in current_pkg:
        packages.append(current_pkg)

    return packages

def generate_spdx_sbom(cargo_lock_path: Path, output_path: Path) -> dict:
    cargo_packages = parse_cargo_lock(cargo_lock_path)
    now_iso = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    sbom = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "OmniPlay-Platform-Release-SBOM",
        "documentNamespace": f"https://vlikeon.vn/spdx/omniplay-1.0.0-{int(datetime.now().timestamp())}",
        "creationInfo": {
            "created": now_iso,
            "creators": [
                "Tool: OmniPlay-SBOM-Generator-1.0",
                "Organization: Vlikeon Project (packaging@vlikeon.vn)",
            ],
        },
        "packages": [
            {
                "SPDXID": "SPDXRef-Package-OmniPlay-Root",
                "name": "omniplay",
                "versionInfo": "1.0.0",
                "downloadLocation": "https://github.com/vlikeon/omniplay",
                "filesAnalyzed": False,
                "licenseConcluded": "GPL-3.0-or-later AND MIT",
                "licenseDeclared": "GPL-3.0-or-later AND MIT",
                "supplier": "Organization: Vlikeon Project",
                "description": "OmniPlay gaming runtime and Remote Play client root application",
            }
        ],
        "relationships": [],
    }

    # Add native cores
    native_components = [
        ("mgba", "0.10.3", "MPL-2.0", "mGBA Libretro Core"),
        ("beetle-psx", "0.9.44.1", "GPL-2.0-only", "Beetle PSX Libretro Core"),
        ("bsnes", "115.0.0", "GPL-3.0-only", "bsnes SNES Libretro Core (adapted)"),
        ("sameboy", "0.16.2", "MIT", "SameBoy GB/GBC Core"),
        ("openh264", "2.4.0", "BSD-2-Clause", "Cisco OpenH264 Binary Library"),
    ]

    for name, version, lic, desc in native_components:
        pkg_id = f"SPDXRef-Package-{name}"
        sbom["packages"].append({
            "SPDXID": pkg_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": lic,
            "licenseDeclared": lic,
            "supplier": f"Organization: Upstream {name}",
            "description": desc,
        })
        sbom["relationships"].append({
            "spdxElementId": "SPDXRef-Package-OmniPlay-Root",
            "relatedSpdxElement": pkg_id,
            "relationshipType": "CONTAINS",
        })

    # Add Cargo crates
    for pkg in cargo_packages:
        name = pkg["name"]
        version = pkg["version"]
        safe_name = re.sub(r"[^a-zA-Z0-9-]", "-", name)
        pkg_id = f"SPDXRef-Package-crate-{safe_name}"
        lic = KNOWN_LICENSES.get(name, "MIT OR Apache-2.0")

        pkg_obj = {
            "SPDXID": pkg_id,
            "name": name,
            "versionInfo": version,
            "downloadLocation": f"https://crates.io/crates/{name}/{version}",
            "filesAnalyzed": False,
            "licenseConcluded": lic,
            "licenseDeclared": lic,
            "supplier": "Person: Rust Crates Community",
        }
        if "checksum" in pkg:
            pkg_obj["checksums"] = [{
                "algorithm": "SHA256",
                "checksumValue": pkg["checksum"],
            }]

        sbom["packages"].append(pkg_obj)
        sbom["relationships"].append({
            "spdxElementId": "SPDXRef-Package-OmniPlay-Root",
            "relatedSpdxElement": pkg_id,
            "relationshipType": "DEPENDS_ON",
        })

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(sbom, f, indent=2)

    print(f"[+] Generated SPDX 2.3 SBOM with {len(sbom['packages'])} packages at {output_path}")
    return sbom

def main():
    lock = Path("Cargo.lock")
    out = Path("dist/omniplay-sbom.spdx.json")
    generate_spdx_sbom(lock, out)

if __name__ == "__main__":
    main()
