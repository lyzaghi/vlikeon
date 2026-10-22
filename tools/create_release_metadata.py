"""Author an unsigned, short-lived release payload for separate offline signatures."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import time
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--identity", type=Path, required=True)
    parser.add_argument("--metadata-version", type=int, required=True)
    parser.add_argument("--root-version", type=int, required=True)
    parser.add_argument("--security-epoch", type=int, required=True)
    parser.add_argument("--artifact-url", required=True)
    parser.add_argument("--valid-hours", type=int, default=48)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not (1 <= args.valid_hours <= 168 and 0 < args.metadata_version < 2**63
            and 0 < args.root_version < 2**63 and 0 <= args.security_epoch < 2**63):
        raise ValueError("Metadata bounds are invalid")
    url = urlsplit(args.artifact_url)
    if (url.scheme != "https" or not url.hostname or url.port or url.username
            or url.password or url.fragment or not url.path or "\\" in args.artifact_url):
        raise ValueError("Use a canonical HTTPS artifact URL on the trusted release host")
    identity = json.loads(args.identity.read_text(encoding="utf-8"))
    for key in ("channel", "platform"):
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", identity[key]):
            raise ValueError("Channel or platform is invalid")
    if not re.fullmatch(r"[0-9a-f]{40}", identity["chromium_revision"]):
        raise ValueError("Chromium revision must be pinned")
    if identity["minimum_data_schema"] > identity["maximum_data_schema"]:
        raise ValueError("Data schema range is reversed")
    digest = hashlib.sha256()
    size = 0
    with args.package.open("rb") as source:
        if source.read(8) != b"OPPK0001":
            raise ValueError("Input is not an OmniPlay package")
        source.seek(0)
        for block in iter(lambda: source.read(65536), b""):
            digest.update(block)
            size += len(block)
    if not 0 < size <= 2 * 1024**3:
        raise ValueError("Artifact size is outside the updater bound")
    now = int(time.time() * 1000)
    payload = {"schema_version": 1, "metadata_version": args.metadata_version,
               "channel": identity["channel"], "platform": identity["platform"],
               "version": identity["version"], "security_epoch": args.security_epoch,
               "root_version": args.root_version, "issued_at_ms": now,
               "expires_at_ms": now + args.valid_hours * 3600000,
               "artifact_url": args.artifact_url, "artifact_sha256": digest.hexdigest(),
               "artifact_bytes": size, "chromium_revision": identity["chromium_revision"],
               "minimum_data_schema": identity["minimum_data_schema"],
               "maximum_data_schema": identity["maximum_data_schema"]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8", newline="\n") as target:
        json.dump(payload, target, ensure_ascii=True, separators=(",", ":"))
        target.write("\n")
    print("Unsigned release metadata authored. Offline signatures are still required.")


if __name__ == "__main__":
    main()
