"""Freeze the exact desktop Chromium overlay and build recipe; never build/test."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    'windows-x86_64': 'windows',
    'linux-x86_64': 'linux',
    'macos-aarch64': 'macos',
}


def relative(value):
    if not isinstance(value, str):
        raise ValueError('A desktop source path must be text')
    path = PurePosixPath(value)
    if not value or path.is_absolute() or '\\' in value or ':' in value or any(part in {'', '.', '..'} for part in value.split('/')):
        raise ValueError('A desktop source path is outside its declared scope')
    return path


def ordinary(path, base, file=True):
    if not path.resolve().is_relative_to(base.resolve()):
        raise ValueError('Desktop source must remain inside its declared root')
    for ancestor in [path, *path.parents]:
        if ancestor.is_symlink() or getattr(ancestor, 'is_junction', lambda: False)():
            raise ValueError('Desktop source ancestry cannot contain links')
        if ancestor == base:
            break
    if file and not path.is_file():
        raise ValueError('A required desktop source input is missing')
    return path


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode('utf-8')


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(256 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--target', choices=TARGETS, required=True)
    parser.add_argument('--development', action='store_true')
    args = parser.parse_args()
    project = args.project.absolute()
    ordinary(project, ROOT, False)
    if not project.resolve().is_relative_to((ROOT / 'out').resolve()) or project.exists():
        raise ValueError('Choose a fresh desktop source project below out/')
    lock = json.loads((ROOT / 'browser/upstream.lock.json').read_text(encoding='utf-8'))
    series = json.loads((ROOT / 'browser/patches/series.json').read_text(encoding='utf-8'))
    if series['upstream_revision'] != lock['revision']:
        raise ValueError('The desktop patch queue belongs to another revision')
    if bool(series['development_only']) != args.development:
        raise ValueError('The selected source overlay differs from the requested build mode')
    staged = ROOT / 'out/browser-integration'
    snapshots = ROOT / 'browser/upstream-snapshots'
    resource_root = 'chrome/browser/resources/omniplay_game_hub'
    inputs = []
    for name in series['modified_files']:
        relative(name)
        inputs.append((ordinary(staged / name, staged), 'overlay/' + name))
        inputs.append((ordinary(snapshots / name, snapshots), 'upstream/' + name))
    for name in series['resource_files']:
        relative(name)
        inputs.append((ordinary(staged / resource_root / name, staged), 'overlay/' + resource_root + '/' + name))
    config = TARGETS[args.target] + ('-development.gn' if args.development else '-release.gn')
    for name in ['browser/upstream.lock.json', 'browser/patches/series.json',
                 'security/component_identity.json', 'security/game_hub_origin.txt',
                 'tools/prepare_desktop_browser.py', 'tools/build_desktop_browser.py',
                 'tools/chromium_dependencies.py',
                 'tools/prepare_chromium_upgrade.py',
                 'tools/generate_browser_patch.py', 'tools/stage_browser_integration.py',
                 'tools/game_hub_assets.py', 'tools/desktop_capture_audio_patch.py',
                 'tools/source_patch.py',
                 'docs/DESKTOP_BROWSER_BUILD.md', 'docs/COMPLETION_SEQUENCE.md',
                 'qa/browser/component-context.test.mjs', 'qa/browser/component-ownership.test.mjs',
                 'qa/browser/DESKTOP_ACCEPTANCE.md']:
        inputs.append((ordinary(ROOT / name, ROOT), 'recipe/' + name))
    for patch in series['patches']:
        name = 'browser/patches/' + relative(patch['file']).as_posix()
        inputs.append((ordinary(ROOT / name, ROOT), 'recipe/' + name))
    inputs.append((ordinary(ROOT / 'browser/config' / config, ROOT), 'recipe/args.gn'))
    destinations = set()
    inventory = []
    total = 0
    for original, destination in inputs:
        relative(destination)
        if destination in destinations:
            raise ValueError('Two desktop source inputs claim the same destination')
        destinations.add(destination)
        size = original.stat().st_size
        total += size
        if size > 512 * 1024 * 1024 or total > 2 * 1024 * 1024 * 1024:
            raise ValueError('The desktop source overlay exceeds its byte bound')
        target = project / 'source' / destination
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, target)
        inventory.append({'path': destination, 'bytes': size, 'sha256': digest(target)})
    receipt = {
        'schema_version': 1, 'source_only': True, 'target': args.target,
        'development_only': args.development, 'upstream_revision': lock['revision'],
        'upstream_repository': lock['repository'], 'source_files': len(inventory),
        'source_bytes': total, 'source_inventory': sorted(inventory, key=lambda item: item['path']),
        'configuration': 'not_started', 'compiler': 'not_started',
        'browser': 'not_started', 'verification': 'not_run',
        'distribution_approved': False,
    }
    (project / 'preparation-receipt.json').write_bytes(encoded(receipt))
    print('Froze the desktop Chromium overlay and build recipe. No dependency hook, compiler, browser or test ran.')


if __name__ == '__main__':
    main()
