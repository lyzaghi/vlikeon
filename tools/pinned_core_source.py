"""Bounded materialization of one explicitly pinned committed core source tree.

This is source preparation, with no configure/build/core/test invocation. Tar
members come from git archive at the exact revision, never the working tree.
"""
import hashlib
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def file_hash(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            result.update(block)
    return result.hexdigest()


def source_inventory(source):
    result = {}
    for path in sorted(source.rglob('*')):
        if path.is_symlink() or getattr(path, 'is_junction', lambda: False)():
            raise ValueError('Corresponding source cannot contain filesystem links')
        if path.is_file():
            result[path.relative_to(source).as_posix()] = file_hash(path)
        elif not path.is_dir():
            raise ValueError('Corresponding source cannot contain special files')
    return result


def owned(path):
    if path.resolve() != path.absolute() or not path.resolve().is_relative_to(ROOT):
        raise ValueError('Source assembly must remain in an unlinked workspace subtree')
    return path


def materialize(core, revision, work, git='git', selected_paths=()):
    original = ROOT / 'third_party/sources' / core
    actual = subprocess.check_output([git, '-C', str(original), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != revision:
        raise ValueError('Core checkout differs from the exact pinned revision')
    owned(work)
    if work.exists():
        raise ValueError('Preserve prior corresponding source; choose a fresh output directory')
    work.mkdir(parents=True)
    archive = work / 'upstream-source.tar'
    if not isinstance(selected_paths, (tuple, list)) or len(selected_paths) > 128 or any(
            not isinstance(name, str) or not name or name.startswith(('/', '-')) or
            any(part in {'', '.', '..'} or any(not (char.isascii() and (char.isalnum() or char in '._-')) for char in part) for part in name.split('/'))
            for name in selected_paths):
        raise ValueError('Committed source selection requires explicit bounded repository-relative paths')
    subprocess.run([git, '-C', str(original), 'archive', '--format=tar', '--output=' + str(archive), revision, *selected_paths], check=True)
    if archive.stat().st_size > 512 * 1024 * 1024:
        raise ValueError('The committed source archive exceeds its byte bound')
    source = work / 'source'
    source.mkdir()
    total = count = 0
    aliases = {}
    with tarfile.open(archive, 'r:') as contents:
        for member in contents:
            name = PurePosixPath(member.name)
            count += 1
            if count > 100000 or name.is_absolute() or not name.parts or len(name.parts) > 32:
                raise ValueError('The committed source archive has an unsafe entry')
            for part in name.parts:
                if part in {'.', '..'} or len(part.encode('utf-8')) > 255 or any(
                        ord(char) < 32 or char in '\\:<>"|?*' for char in part) or part.endswith(('.', ' ')):
                    raise ValueError('The committed source archive has an unsafe component')
                if part.split('.')[0].upper() in {'CON', 'PRN', 'AUX', 'NUL', *('COM'+str(i) for i in range(1, 10)), *('LPT'+str(i) for i in range(1, 10))}:
                    raise ValueError('The committed source archive has a reserved component')
            spelling = name.as_posix()
            for index in range(1, len(name.parts) + 1):
                parent = '/'.join(name.parts[:index])
                previous = aliases.setdefault(parent.upper(), parent)
                if previous != parent:
                    raise ValueError('The source archive aliases a Windows path')
            path = source.joinpath(*name.parts)
            if not path.resolve().is_relative_to(source.resolve()) or member.issym() or member.islnk():
                raise ValueError('The source archive cannot contain links or escape its root')
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                total += member.size
                if member.size < 0 or member.size > 256 * 1024 * 1024 or total > 512 * 1024 * 1024:
                    raise ValueError('The committed source exceeds its file budget')
                path.parent.mkdir(parents=True, exist_ok=True)
                with contents.extractfile(member) as incoming, path.open('xb') as outgoing:
                    shutil.copyfileobj(incoming, outgoing, 65536)
            else:
                raise ValueError('The committed source archive contains a special entry')
    return source, archive
