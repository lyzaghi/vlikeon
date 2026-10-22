"""Assemble pinned official Rust components locally; never build/run product code.

No installer, registry change, global PATH edit or tool execution is performed.
Published SHA-256 values establish download integrity, not independent signing.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
import urllib.request

VERSION = '1.89.0'
TARGET = 'x86_64-pc-windows-msvc'
ORIGIN = 'https://static.rust-lang.org/dist/'
ARCHIVE_LIMIT = 512 * 1024 * 1024
TOTAL_LIMIT = 2 * 1024 * 1024 * 1024


def download(url, destination, maximum):
    digest = hashlib.sha256()
    size = 0
    with urllib.request.urlopen(url, timeout=60) as response:
        if not response.url.startswith(ORIGIN):
            raise ValueError('Unexpected Rust distribution redirect')
        with destination.open('xb') as output:
            while True:
                block = response.read(256 * 1024)
                if not block:
                    break
                size += len(block)
                if size > maximum:
                    raise ValueError('Rust download exceeds its budget')
                digest.update(block)
                output.write(block)
    return size, digest.hexdigest()


def safe_parts(raw):
    name = PurePosixPath(raw)
    if name.is_absolute() or '\\' in raw or len(name.parts) > 32:
        raise ValueError('Invalid component path')
    for part in name.parts:
        if part in {'.', '..'} or part.endswith((' ', '.')) or re.search(r'[\x00-\x1f:<>"|?*]', part):
            raise ValueError('Invalid component path')
        if part.split('.')[0].upper() in {'CON','PRN','AUX','NUL',*(f'COM{i}' for i in range(1,10)),*(f'LPT{i}' for i in range(1,10))}:
            raise ValueError('Reserved component path')
    return name.parts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', required=True)
    args = parser.parse_args()
    workspace = Path(__file__).resolve().parents[1]
    project = Path(args.project).absolute()
    if not project.is_relative_to(workspace) or project == workspace:
        raise ValueError('Use a fresh directory within the project workspace')
    for ancestor in [project, *project.parents]:
        if ancestor.is_symlink() or getattr(ancestor, 'is_junction', lambda: False)():
            raise ValueError('Toolchain ancestry cannot contain links')
        if ancestor == workspace:
            break
    if project.exists():
        raise ValueError('Preserve previous toolchains; select a fresh directory')
    project.mkdir(parents=True)
    downloads = project / 'downloads'
    downloads.mkdir()
    toolchain = project / 'toolchain'
    toolchain.mkdir()
    inventory, receipts = {}, []
    total = entries = 0
    for package in ('rustc', 'cargo', 'rust-std'):
        stem = f'{package}-{VERSION}-{TARGET}'
        archive = downloads / (stem + '.tar.xz')
        checksum = downloads / (archive.name + '.sha256')
        download(ORIGIN + checksum.name, checksum, 4096)
        expected = checksum.read_text('ascii').split()[0]
        if not re.fullmatch('[0-9a-f]{64}', expected):
            raise ValueError('Invalid published checksum')
        size, observed = download(ORIGIN + archive.name, archive, ARCHIVE_LIMIT)
        if observed != expected:
            raise ValueError('Rust archive integrity mismatch')
        component = f'rust-std-{TARGET}' if package == 'rust-std' else package
        with tarfile.open(archive, 'r:xz') as source:
            for member in source:
                entries += 1
                if entries > 100000:
                    raise ValueError('Component entry budget exceeded')
                parts = safe_parts(member.name)
                if not parts or parts[0] != stem:
                    raise ValueError('Unexpected archive root')
                if len(parts) < 4 or parts[1] != component or parts[2] not in {'bin','lib'}:
                    continue
                if member.isdir():
                    continue
                if not member.isfile() or member.size < 0 or member.size > ARCHIVE_LIMIT:
                    raise ValueError('Unexpected component entry type or size')
                relative = PurePosixPath(*parts[2:]).as_posix()
                total += member.size
                if total > TOTAL_LIMIT:
                    raise ValueError('Toolchain expansion exceeds budget')
                destination = toolchain.joinpath(*parts[2:])
                # Refuse merged-file collisions rather than replacing an earlier component.
                if relative in inventory:
                    raise ValueError('Component files collide')
                destination.parent.mkdir(parents=True, exist_ok=True)
                payload = source.extractfile(member)
                if payload is None:
                    raise ValueError('Component payload missing')
                digest = hashlib.sha256()
                written = 0
                with payload, destination.open('xb') as output:
                    while True:
                        block = payload.read(256 * 1024)
                        if not block:
                            break
                        written += len(block)
                        if written > member.size:
                            raise ValueError('Component extent changed')
                        digest.update(block)
                        output.write(block)
                if written != member.size:
                    raise ValueError('Truncated component')
                inventory[relative] = {'bytes': written, 'sha256': digest.hexdigest()}
        receipts.append({'package': package, 'url': ORIGIN + archive.name, 'bytes': size, 'sha256': observed})
    required = ['bin/rustc.exe','bin/cargo.exe',f'lib/rustlib/{TARGET}/lib']
    if any(not toolchain.joinpath(*PurePosixPath(name).parts).exists() for name in required):
        raise ValueError('Required toolchain component missing')
    receipt = {'schema_version':1,'version':VERSION,'target':TARGET,'components':receipts,
               'inventory':inventory,'files':len(inventory),'bytes':total,
               'tool_execution':'not_started','product_build':'not_started','tests':'not_run',
               'global_path_modified':False,'registry_modified':False}
    (project / 'preparation-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({key:receipt[key] for key in ('version','target','files','bytes','tool_execution','product_build','tests')}))


if __name__ == '__main__':
    main()
