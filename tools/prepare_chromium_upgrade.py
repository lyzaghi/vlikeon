"""Author a three-sided Chromium upgrade review from already-present Git objects.

Copies current originals, OmniPlay replacements and candidate upstream bytes.
Does not fetch, switch/reset a checkout, apply patches, configure, build or test.
"""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
from prepare_desktop_browser import ROOT, digest, encoded, ordinary, relative

REPOSITORY = 'https://chromium.googlesource.com/chromium/src.git'
MAX_FILE = 8 * 1024 * 1024
MAX_TOTAL = 128 * 1024 * 1024


def git_bytes(git, checkout, environment, *arguments):
    return subprocess.check_output([str(git), '-C', str(checkout), *arguments], env=environment)


def blob(git, checkout, environment, revision, name):
    relative(name)
    tree = git_bytes(git, checkout, environment, 'ls-tree', '-z', revision, '--', name)
    records = tree.split(b'\0')
    if len(records) != 2 or records[1] or b'\t' not in records[0]:
        raise ValueError('The candidate integration path is not one exact Git tree entry')
    shape, path = records[0].split(b'\t', 1)
    parts = shape.split(b' ')
    if len(parts) != 3 or parts[0] not in {b'100644', b'100755'} or parts[1] != b'blob' or path != name.encode('utf-8'):
        raise ValueError('The integration source must be an ordinary non-linked Git file')
    specification = revision + ':' + name
    size = int(git_bytes(git, checkout, environment, 'cat-file', '-s', specification).strip())
    if not 0 <= size <= MAX_FILE:
        raise ValueError('A candidate integration source exceeds its byte bound')
    identity = git_bytes(git, checkout, environment, 'rev-parse', specification).decode('ascii').strip()
    if not re.fullmatch('[0-9a-f]{40}', identity):
        raise ValueError('The candidate source has no exact ordinary Git blob identity')
    raw = git_bytes(git, checkout, environment, 'cat-file', 'blob', specification)
    if len(raw) != size or hashlib.sha1(b'blob ' + str(size).encode('ascii') + b'\0' + raw).hexdigest() != identity:
        raise ValueError('A candidate integration source differs from its Git blob identity')
    return raw, identity


def version(raw):
    pairs = {}
    for line in raw.decode('ascii').splitlines():
        key, separator, value = line.partition('=')
        if not separator or key in pairs or key not in {'MAJOR', 'MINOR', 'BUILD', 'PATCH'} or not re.fullmatch('[0-9]{1,8}', value):
            raise ValueError('The candidate Chromium VERSION document is invalid')
        pairs[key] = int(value)
    if set(pairs) != {'MAJOR', 'MINOR', 'BUILD', 'PATCH'}:
        raise ValueError('The candidate Chromium version is incomplete')
    return tuple(pairs[key] for key in ['MAJOR', 'MINOR', 'BUILD', 'PATCH'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--candidate-checkout', type=Path, required=True)
    parser.add_argument('--candidate-revision', required=True)
    parser.add_argument('--git', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch('[0-9a-f]{40}', args.candidate_revision):
        raise ValueError('Select an explicit candidate commit; branch names are not upgrade pins')
    project = args.project.absolute()
    ordinary(project, ROOT, False)
    if project.exists() or not project.resolve().is_relative_to((ROOT / 'out').resolve()):
        raise ValueError('Choose a fresh workspace output for the Chromium upgrade review')
    git = args.git.resolve(strict=True)
    checkout = args.candidate_checkout.resolve(strict=True)
    environment = os.environ.copy()
    # A promisor checkout may otherwise fetch behind a nominally read-only
    # object request. This authoring command operates only on local objects.
    environment['GIT_NO_LAZY_FETCH'] = '1'
    environment['GIT_OPTIONAL_LOCKS'] = '0'
    remote = git_bytes(git, checkout, environment, 'remote', 'get-url', 'origin').decode('utf-8').strip()
    if remote != REPOSITORY:
        raise ValueError('Use an explicit checkout of the fixed Chromium upstream repository')
    resolved = git_bytes(git, checkout, environment, 'rev-parse', '--verify', args.candidate_revision + '^{commit}').decode('ascii').strip()
    if resolved != args.candidate_revision:
        raise ValueError('The requested candidate commit is not present locally')
    lock = json.loads((ROOT / 'browser/upstream.lock.json').read_text(encoding='utf-8'))
    series = json.loads((ROOT / 'browser/patches/series.json').read_text(encoding='utf-8'))
    if series['upstream_revision'] != lock['revision'] or args.candidate_revision == lock['revision']:
        raise ValueError('Select a different candidate after authoring the current exact desktop overlay')
    current_version = tuple(int(part) for part in lock['version'].split('.'))
    raw_version, version_blob = blob(git, checkout, environment, args.candidate_revision, 'chrome/VERSION')
    candidate_version = version(raw_version)
    if len(current_version) != 4 or candidate_version <= current_version:
        raise ValueError('An upstream upgrade review requires an explicitly newer Chromium version')
    snapshots = ROOT / 'browser/upstream-snapshots'
    staged = ROOT / 'out/browser-integration'
    paths = set(series['modified_files'])
    for path in snapshots.rglob('*'):
        ordinary(path, snapshots, False)
        if path.is_file():
            paths.add(path.relative_to(snapshots).as_posix())
    if not 1 <= len(paths) <= 256:
        raise ValueError('The integration snapshot inventory exceeds its source-review bound')
    prepared = []
    total = 0
    for name in sorted(paths):
        relative(name)
        original = ordinary(snapshots / name, snapshots).read_bytes()
        baseline, baseline_blob = blob(git, checkout, environment, lock['revision'], name)
        if baseline != original:
            raise ValueError('A baseline source snapshot differs from its active upstream revision')
        candidate, candidate_blob = blob(git, checkout, environment, args.candidate_revision, name)
        modified = name in series['modified_files']
        product = ordinary(staged / name, staged).read_bytes() if modified else None
        total += len(original) + len(candidate) + (len(product) if product is not None else 0)
        if len(original) > MAX_FILE or product is not None and len(product) > MAX_FILE or total > MAX_TOTAL:
            raise ValueError('The upstream upgrade review exceeds its source byte bound')
        prepared.append((name, original, baseline_blob, candidate, candidate_blob, product))
    # Collect all candidate objects before publishing any output directory.
    project.mkdir(parents=True)
    inventory = []
    changes = []
    delta = []
    for name, original, baseline_blob, candidate, candidate_blob, product in prepared:
        variants = [('baseline', original), ('candidate', candidate)]
        if product is not None:
            variants.append(('omniplay', product))
        for group, raw in variants:
            path = project / 'source' / group / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)
            inventory.append({'path': path.relative_to(project).as_posix(), 'bytes': len(raw),
                              'sha256': hashlib.sha256(raw).hexdigest()})
        changed = original.replace(b'\r\n', b'\n') != candidate.replace(b'\r\n', b'\n')
        changes.append({'path': name, 'baseline_blob': baseline_blob, 'candidate_blob': candidate_blob,
                        'omniplay_modifies': product is not None, 'upstream_changed': changed,
                        'migration': 'review_required' if changed else 'unchanged_source_still_requires_candidate_build'})
        if changed:
            delta.extend(difflib.unified_diff(original.decode('utf-8').splitlines(keepends=True),
                candidate.decode('utf-8').splitlines(keepends=True),
                fromfile='baseline/' + name, tofile='candidate/' + name))
    (project / 'upstream-delta.patch').write_text(''.join(delta), encoding='utf-8', newline='\n')
    (project / 'candidate-VERSION').write_bytes(raw_version)
    shutil.copy2(ROOT / 'browser/upstream.lock.json', project / 'baseline-lock.json')
    shutil.copy2(ROOT / 'browser/patches/series.json', project / 'baseline-series.json')
    receipt = {'schema_version': 1, 'source_only': True, 'baseline_revision': lock['revision'],
        'candidate_revision': args.candidate_revision, 'candidate_version': '.'.join(map(str, candidate_version)),
        'candidate_version_blob': version_blob, 'source_inventory': inventory, 'integration_sources': changes,
        'fetch': 'not_started', 'patch_application': 'not_started', 'configuration': 'not_started',
        'compiler': 'not_started', 'verification': 'not_run', 'lock_changed': False,
        'distribution_approved': False, 'authoring_recipe_sha256': digest(Path(__file__).resolve())}
    (project / 'upgrade-review.json').write_bytes(encoded(receipt))
    print('Authored the three-sided Chromium upgrade review. No fetch, checkout change, patch check, compiler, browser or test ran.')


if __name__ == '__main__':
    main()
