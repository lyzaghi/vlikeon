"""Independent task-4 build integrity check. Does not start the browser."""
import argparse
import hashlib
import json
import platform
from pathlib import Path, PurePosixPath
import stat
import struct


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', required=True, type=Path)
    parser.add_argument('--batch-result', required=True, type=Path)
    parser.add_argument('--expected-preparation', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.machine().lower() not in {'x86_64', 'amd64'}:
        raise ValueError('Run the independent acceptance on the Linux x64 build host')
    root = args.project.absolute()
    for ancestor in [root, *root.parents]:
        if ancestor.is_symlink():
            raise ValueError('Linked project ancestry')
    root = root.resolve(strict=True)

    def ordinary(name):
        relative = PurePosixPath(name)
        if (not name or relative.is_absolute() or '\\' in name or ':' in name
                or any(part in {'', '.', '..'} for part in name.split('/'))):
            raise ValueError('Invalid artifact path')
        path = root.joinpath(*relative.parts)
        for ancestor in [path, *path.parents]:
            if ancestor.is_symlink():
                raise ValueError('Linked build artifact: ' + name)
            if ancestor == root:
                break
        if not path.resolve(strict=True).is_relative_to(root) or not path.is_file():
            raise ValueError('Missing or escaped artifact: ' + name)
        return path

    def document(name):
        return json.loads(ordinary(name).read_text(encoding='utf-8'))

    preparation_file = ordinary('preparation-receipt.json')
    if digest(preparation_file) != args.expected_preparation:
        raise ValueError('Different frozen preparation')
    preparation = document('preparation-receipt.json')
    receipt = document('build-receipt.json')
    journal = document('build-journal.json')
    batch = json.loads(args.batch_result.read_text(encoding='utf-8-sig'))
    if (batch.get('item') != 4 or batch.get('status') != 'passed'
            or batch.get('preparation_sha256') != args.expected_preparation
            or not any(case.get('id') == 'chromium-build' and case.get('exit_code') == 0
                       and not case.get('error') for case in batch.get('cases', []))):
        raise ValueError('No successful full Linux build batch bound to these inputs')
    if (journal.get('phase') != 'built' or journal.get('running') is not None
            or journal.get('failed') is not None
            or journal.get('identity', {}).get('preparation_sha256') != args.expected_preparation):
        raise ValueError('Build journal is incomplete or belongs to different inputs')
    expected = {
        'schema_version': 1, 'target': 'linux-x86_64', 'development_only': True,
        'configuration': 'completed', 'compiler': 'completed',
        'upstream_revision': '0e66b94549a8bd85f03fbdf189b752d6e7b63c32',
        'preparation_sha256': args.expected_preparation,
        'binary_relative_path': 'workspace/src/out/OmniPlay/chrome',
        'browser': 'not_started', 'verification': 'not_run',
        'reproducibility_verified': False, 'distribution_approved': False,
    }
    for key, value in expected.items():
        if receipt.get(key) != value:
            raise ValueError('Unexpected build receipt field: ' + key)
    if (preparation.get('target') != 'linux-x86_64'
            or preparation.get('source_only') is not True
            or preparation.get('development_only') is not True
            or preparation.get('upstream_revision') != expected['upstream_revision']):
        raise ValueError('Unexpected Linux preparation')
    inventory = preparation.get('source_inventory')
    if not isinstance(inventory, list) or not 1 <= len(inventory) <= 10000:
        raise ValueError('Invalid frozen input inventory')
    seen = set()
    for item in inventory:
        name = item['path']
        if name in seen:
            raise ValueError('Duplicate frozen input')
        seen.add(name)
        file = ordinary('source/' + name)
        if file.stat().st_size != item['bytes'] or digest(file) != item['sha256']:
            raise ValueError('Frozen input changed: ' + name)
    for name, field in [
        ('dependency-revisions.txt', 'dependency_revisions_sha256'),
        ('dependency-git-inventory.json', 'dependency_git_inventory_sha256'),
        ('workspace/src/buildtools/linux64/gn', 'gn_sha256'),
    ]:
        if digest(ordinary(name)) != receipt.get(field):
            raise ValueError('Receipt binding mismatch: ' + name)
    dependency_inventory = document('dependency-git-inventory.json')
    if (dependency_inventory.get('tracked_source_matches') is not True
            or not dependency_inventory.get('git_dependencies')):
        raise ValueError('Missing dependency admission evidence')
    binary = ordinary(receipt['binary_relative_path'])
    if binary.stat().st_size != receipt['binary_bytes'] or digest(binary) != receipt['binary_sha256']:
        raise ValueError('Browser entry point does not match receipt')
    if not stat.S_IMODE(binary.stat().st_mode) & 0o111:
        raise ValueError('Browser entry point has no executable mode bits')
    with binary.open('rb') as stream:
        header = stream.read(64)
    if (len(header) != 64 or header[:7] != b'\x7fELF\x02\x01\x01'
            or struct.unpack_from('<H', header, 16)[0] not in {2, 3}
            or struct.unpack_from('<H', header, 18)[0] != 62
            or struct.unpack_from('<I', header, 20)[0] != 1
            or struct.unpack_from('<H', header, 52)[0] != 64):
        raise ValueError('Browser entry point is not a little-endian ELF64 x86-64 executable')
    artifacts = {}
    for name in ['chrome', 'resources.pak', 'icudtl.dat', 'v8_context_snapshot.bin', 'locales/en-US.pak']:
        file = ordinary('workspace/src/out/OmniPlay/' + name)
        if file.stat().st_size <= 0:
            raise ValueError('Empty selected output: ' + name)
        artifacts[name] = {'bytes': file.stat().st_size, 'sha256': digest(file)}
    result = {
        'schema_version': 1, 'item': 4, 'passed': True,
        'scope': 'Full development chrome target and selected output integrity only; no browser runtime or release acceptance',
        'preparation_sha256': args.expected_preparation,
        'receipt_sha256': digest(ordinary('build-receipt.json')),
        'batch_result_sha256': digest(args.batch_result),
        'frozen_inputs_unchanged': True, 'architecture': 'ELF64 x86-64 little-endian',
        'artifacts': artifacts, 'distribution_approved': False,
    }
    # Exclusive creation keeps earlier acceptance/failure reports intact.
    with args.output.open('x', encoding='utf-8', newline='\n') as stream:
        stream.write(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
