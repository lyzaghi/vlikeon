"""Replace the stable GNU/Linux bootstrap with explicit old and new SHA-256 pins.

The operation is per-user, journaled and resumable.  It does not activate a
release, change registration, stop a process or touch profile and save data.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import sys
import tempfile

PIN = re.compile(r"[0-9a-fA-F]{64}\Z")
NAMES = ("omniplay-update", "omniplay-launcher")


def fail(message):
    raise SystemExit(message)


def digest(path):
    value = hashlib.sha256()
    with path.open("rb", buffering=0) as source:
        for block in iter(lambda: source.read(65536), b""):
            value.update(block)
    return value.hexdigest()


def ordinary(path, executable=True, owned=True):
    try:
        shape = path.lstat()
    except OSError as error:
        fail(f"Bootstrap file is unavailable: {path}: {error}")
    if not stat.S_ISREG(shape.st_mode) or (owned and shape.st_uid != os.geteuid()) or shape.st_nlink != 1:
        fail(f"Bootstrap file is not an owned single-link ordinary file: {path}")
    if executable and shape.st_mode & 0o111 == 0:
        fail(f"Bootstrap file is not executable: {path}")


def pinned(value):
    if not PIN.fullmatch(value):
        fail("Every old and new bootstrap hash must be an explicit SHA-256 value")
    return value.lower()


def directory_sync(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic_json(path, value):
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}-", dir=path.parent)
    temporary = Path(temporary)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as target:
            os.fchmod(target.fileno(), 0o600)
            json.dump(value, target, sort_keys=True, separators=(",", ":"))
            target.write("\n")
            target.flush()
            os.fsync(target.fileno())
        os.replace(temporary, path)
        directory_sync(path.parent)
    finally:
        temporary.unlink(missing_ok=True)


def stage(source, directory, expected):
    ordinary(source, owned=False)
    if digest(source) != expected:
        fail("A replacement bootstrap does not match its new SHA-256 pin")
    descriptor, temporary = tempfile.mkstemp(prefix=".omniplay-bootstrap-next-", dir=directory)
    temporary = Path(temporary)
    try:
        with os.fdopen(descriptor, "wb", buffering=0) as target, source.open("rb", buffering=0) as opened:
            os.fchmod(target.fileno(), 0o700)
            shutil.copyfileobj(opened, target, 65536)
            target.flush()
            os.fsync(target.fileno())
        if digest(temporary) != expected:
            fail("A replacement bootstrap changed while it was copied")
        return temporary
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bootstrap-directory", required=True, type=Path,
                        help="Directory containing the replacement omniplay-update and omniplay-launcher")
    for name in ("current-launcher", "current-updater", "new-launcher", "new-updater"):
        parser.add_argument(f"--{name}-sha256", required=True)
    args = parser.parse_args()
    if not sys.platform.startswith("linux") or platform.machine().lower() not in {"x86_64", "amd64"} or os.geteuid() == 0:
        fail("Bootstrap maintenance requires an unprivileged GNU/Linux x86-64 user")
    os.umask(0o077)
    data = Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local/share")))
    if not data.is_absolute():
        fail("XDG_DATA_HOME must be absolute")
    target = data / "OmniPlay" / "install" / "bootstrap"
    shape = target.lstat()
    if not stat.S_ISDIR(shape.st_mode) or shape.st_uid != os.geteuid() or shape.st_mode & 0o7777 != 0o700:
        fail("The installed bootstrap directory is not private")
    source_shape = args.bootstrap_directory.lstat()
    source = args.bootstrap_directory.resolve(strict=True)
    if not stat.S_ISDIR(source_shape.st_mode) or stat.S_ISLNK(source_shape.st_mode):
        fail("The replacement bootstrap source must be an ordinary directory")
    current = {"omniplay-launcher": pinned(args.current_launcher_sha256),
               "omniplay-update": pinned(args.current_updater_sha256)}
    replacement = {"omniplay-launcher": pinned(args.new_launcher_sha256),
                   "omniplay-update": pinned(args.new_updater_sha256)}
    lock_path = target / "bootstrap-maintenance.lock"
    lock = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_CLOEXEC | os.O_NOFOLLOW, 0o600)
    try:
        lock_shape = os.fstat(lock)
        if not stat.S_ISREG(lock_shape.st_mode) or lock_shape.st_uid != os.geteuid() or lock_shape.st_nlink != 1 or lock_shape.st_mode & 0o7777 != 0o600:
            fail("The bootstrap maintenance lock is not private")
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        journal = target / "bootstrap-maintenance.json"
        wanted = {"schema_version": 1, "current": current, "replacement": replacement}
        if journal.exists():
            ordinary(journal, executable=False)
            try:
                existing = json.loads(journal.read_text(encoding="utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError):
                fail("The bootstrap maintenance journal is corrupt")
            if existing != wanted:
                fail("Another bootstrap replacement journal requires explicit recovery")
        else:
            for name in NAMES:
                ordinary(target / name)
                if digest(target / name) != current[name]:
                    fail("The installed bootstrap differs from its current SHA-256 pin")
            atomic_json(journal, wanted)
        for name in NAMES:
            destination = target / name
            ordinary(destination)
            found = digest(destination)
            if found == replacement[name]:
                continue
            if found != current[name]:
                fail("The interrupted bootstrap replacement has an unknown file")
            temporary = stage(source / name, target, replacement[name])
            try:
                os.replace(temporary, destination)
                os.chmod(destination, 0o700)
                directory_sync(target)
            finally:
                temporary.unlink(missing_ok=True)
        for name in NAMES:
            ordinary(target / name)
            if digest(target / name) != replacement[name]:
                fail("The installed bootstrap replacement is incomplete")
        journal.unlink()
        directory_sync(target)
    finally:
        os.close(lock)
    print("The stable OmniPlay bootstrap was replaced without stopping active processes or changing user data.")


if __name__ == "__main__":
    main()
