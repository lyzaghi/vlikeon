"""Task-5 development build integrity only; never launches or signs the app."""
import argparse
import hashlib
import json
import platform
from pathlib import Path, PurePosixPath
import plistlib
import stat
import struct


def digest(file):
    result = hashlib.sha256()
    with file.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def macho_arm64(file, expected_type):
    # Apple xnu EXTERNAL_HEADERS/mach-o/loader.h and osfmk/mach/machine.h.
    with file.open('rb') as stream:
        header = stream.read(32)
    if len(header) != 32:
        raise ValueError('Truncated Mach-O header')
    magic, cpu, subtype, kind, commands, command_bytes, flags, reserved = struct.unpack('<8I', header)
    if (magic != 0xFEEDFACF or cpu != 0x0100000C or kind != expected_type
            or not 1 <= commands <= 65536 or command_bytes < 8 * commands
            or 32 + command_bytes > file.stat().st_size):
        raise ValueError('Unexpected thin ARM64 Mach-O executable/library header')
    if not stat.S_IMODE(file.stat().st_mode) & 0o111:
        raise ValueError('Mach-O file has no executable mode bits')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', required=True, type=Path)
    parser.add_argument('--batch-result', required=True, type=Path)
    parser.add_argument('--expected-preparation', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if platform.system() != 'Darwin' or platform.machine().lower() != 'arm64':
        raise ValueError('Run acceptance natively on the Apple Silicon build host')
    root = args.project.absolute()
    for ancestor in [root, *root.parents]:
        if ancestor.is_symlink():
            raise ValueError('Linked project ancestry')
    root = root.resolve(strict=True)

    def ordinary(name):
        path = PurePosixPath(name)
        if (not name or path.is_absolute() or '\\' in name or ':' in name
                or any(part in {'', '.', '..'} for part in name.split('/'))):
            raise ValueError('Invalid artifact path')
        file = root.joinpath(*path.parts)
        for ancestor in [file, *file.parents]:
            if ancestor.is_symlink():
                raise ValueError('Linked source/receipt/entry point: ' + name)
            if ancestor == root:
                break
        if not file.resolve(strict=True).is_relative_to(root) or not file.is_file():
            raise ValueError('Missing or escaped artifact: ' + name)
        return file

    def document(name):
        return json.loads(ordinary(name).read_text(encoding='utf-8'))

    preparation = document('preparation-receipt.json')
    if digest(ordinary('preparation-receipt.json')) != args.expected_preparation:
        raise ValueError('Different frozen preparation')
    receipt = document('build-receipt.json')
    journal = document('build-journal.json')
    batch = json.loads(args.batch_result.read_text(encoding='utf-8-sig'))
    if (batch.get('item') != 5 or batch.get('status') != 'passed'
            or batch.get('preparation_sha256') != args.expected_preparation
            or not any(case.get('id') == 'chromium-build' and case.get('exit_code') == 0
                       and not case.get('error') for case in batch.get('cases', []))):
        raise ValueError('No successful full Mac build batch bound to these inputs')
    if (journal.get('phase') != 'built' or journal.get('running') is not None
            or journal.get('failed') is not None
            or journal.get('identity', {}).get('preparation_sha256') != args.expected_preparation):
        raise ValueError('Incomplete or mismatched journal')
    expected = {
        'schema_version': 1, 'target': 'macos-aarch64', 'development_only': True,
        'configuration': 'completed', 'compiler': 'completed',
        'upstream_revision': '0e66b94549a8bd85f03fbdf189b752d6e7b63c32',
        'preparation_sha256': args.expected_preparation,
        'browser': 'not_started', 'verification': 'not_run',
        'reproducibility_verified': False, 'distribution_approved': False,
    }
    for key, value in expected.items():
        if receipt.get(key) != value:
            raise ValueError('Unexpected receipt field: ' + key)
    if (preparation.get('target') != 'macos-aarch64'
            or preparation.get('source_only') is not True
            or preparation.get('development_only') is not True
            or preparation.get('upstream_revision') != expected['upstream_revision']):
        raise ValueError('Unexpected macOS preparation')
    inventory = preparation.get('source_inventory')
    if not isinstance(inventory, list) or not 1 <= len(inventory) <= 10000:
        raise ValueError('Invalid frozen input inventory')
    seen = set()
    for item in inventory:
        if item['path'] in seen:
            raise ValueError('Duplicate frozen input')
        seen.add(item['path'])
        file = ordinary('source/' + item['path'])
        if file.stat().st_size != item['bytes'] or digest(file) != item['sha256']:
            raise ValueError('Frozen input changed: ' + item['path'])
    for name, field in [
        ('dependency-revisions.txt', 'dependency_revisions_sha256'),
        ('dependency-git-inventory.json', 'dependency_git_inventory_sha256'),
        ('workspace/src/buildtools/mac/gn', 'gn_sha256'),
    ]:
        if digest(ordinary(name)) != receipt.get(field):
            raise ValueError('Receipt binding mismatch: ' + name)
    dependencies = document('dependency-git-inventory.json')
    if dependencies.get('tracked_source_matches') is not True or not dependencies.get('git_dependencies'):
        raise ValueError('Missing actual dependency admission evidence')
    name = receipt.get('binary_relative_path', '')
    parts = PurePosixPath(name).parts
    if (len(parts) != 8 or parts[:4] != ('workspace', 'src', 'out', 'OmniPlay')
            or not parts[4].endswith('.app') or parts[5:7] != ('Contents', 'MacOS')):
        raise ValueError('Unexpected application entry point')
    binary = ordinary(name)
    bundle = binary.parents[2]
    info_file = ordinary((bundle / 'Contents/Info.plist').relative_to(root).as_posix())
    with info_file.open('rb') as stream:
        info = plistlib.load(stream)
    if (info.get('CFBundleIdentifier') != 'app.omniplay.browser'
            or info.get('CFBundleExecutable') != binary.name
            or info.get('CFBundlePackageType') != 'APPL'):
        raise ValueError('Application identity differs from prepared product')
    if binary.stat().st_size != receipt['binary_bytes'] or digest(binary) != receipt['binary_sha256']:
        raise ValueError('Application entry point does not match receipt')
    macho_arm64(binary, 2)  # MH_EXECUTE

    def bundled(file):
        # macOS frameworks normally contain internal Versions/Current links.
        resolved = file.resolve(strict=True)
        if not resolved.is_relative_to(bundle) or not resolved.is_file():
            raise ValueError('Missing or escaped bundle resource')
        return resolved

    framework_name = bundle.stem + ' Framework'
    framework = bundle / 'Contents/Frameworks' / (framework_name + '.framework')
    framework_binary = bundled(framework / framework_name)
    macho_arm64(framework_binary, 6)  # MH_DYLIB
    resources = framework / 'Resources'
    artifacts = {}
    files = [binary, info_file, framework_binary]
    for resource in ['resources.pak', 'icudtl.dat', 'v8_context_snapshot.arm64.bin']:
        files.append(bundled(resources / resource))
    for file in files:
        if file.stat().st_size <= 0:
            raise ValueError('Empty selected bundle artifact')
        artifacts[file.relative_to(root).as_posix()] = {'bytes': file.stat().st_size, 'sha256': digest(file)}
    result = {
        'schema_version': 1, 'item': 5, 'passed': True,
        'scope': 'Full development chrome target and selected app/framework integrity only; no browser runtime, signing, notarization or release acceptance',
        'preparation_sha256': args.expected_preparation,
        'receipt_sha256': digest(ordinary('build-receipt.json')),
        'batch_result_sha256': digest(args.batch_result),
        'frozen_inputs_unchanged': True, 'architecture': 'thin Mach-O ARM64',
        'bundle_identifier': info['CFBundleIdentifier'], 'artifacts': artifacts,
        'distribution_approved': False,
    }
    with args.output.open('x', encoding='utf-8', newline='\n') as stream:
        stream.write(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
