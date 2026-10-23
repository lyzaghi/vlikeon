"""Build a frozen desktop source project during the final verification phase only.

This entry point performs downloads, upstream hooks and compilation. It is never
called by source preparation and does not run or install the resulting browser.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
from prepare_desktop_browser import digest, encoded, ordinary, relative
from chromium_dependencies import verify_dependency_inventory

HOSTS = {'windows-x86_64': 'Windows', 'linux-x86_64': 'Linux', 'macos-aarch64': 'Darwin'}
REPOSITORY = 'https://chromium.googlesource.com/chromium/src.git'
RESOURCE_ROOT = 'chrome/browser/resources/omniplay_game_hub'


def git_text(git, directory, *arguments):
    return subprocess.check_output([str(git), '-C', str(directory), *arguments], text=True).strip()


def persist(path, value):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix='.desktop-build-', delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(encoded(value))
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if temporary:
            temporary.unlink(missing_ok=True)


def check_inventory(project, receipt):
    source = ordinary(project / 'source', project, False)
    expected = set()
    inventory = receipt.get('source_inventory')
    if not isinstance(inventory, list) or not 1 <= len(inventory) <= 10000:
        raise ValueError('The prepared desktop source inventory is invalid')
    for item in inventory:
        name = relative(item['path']).as_posix()
        if name in expected:
            raise ValueError('A prepared desktop source input is duplicated')
        expected.add(name)
        path = ordinary(source / name, source)
        if path.stat().st_size != item['bytes'] or digest(path) != item['sha256']:
            raise ValueError('A prepared desktop source input changed: ' + name)
    for path in source.rglob('*'):
        ordinary(path, source, False)
        if path.is_file() and path.relative_to(source).as_posix() not in expected:
            raise ValueError('The prepared desktop source contains an unknown file')
    return source


def command(arguments, cwd, environment, journal, journal_path, phase):
    journal['running'] = phase
    journal['failed'] = None
    persist(journal_path, journal)
    try:
        subprocess.run([str(value) for value in arguments], cwd=cwd, env=environment, check=True)
    except (OSError, subprocess.CalledProcessError):
        journal['failed'] = phase
        persist(journal_path, journal)
        raise
    journal['running'] = None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, required=True)
    parser.add_argument('--git', type=Path, required=True)
    parser.add_argument('--python', type=Path, required=True)
    parser.add_argument('--depot-tools', type=Path, required=True)
    parser.add_argument('--depot-tools-revision', required=True)
    parser.add_argument('--jobs', type=int, required=True)
    parser.add_argument('--final-verification', action='store_true', required=True)
    args = parser.parse_args()
    if not 1 <= args.jobs <= 256 or not re.fullmatch('[0-9a-f]{40}', args.depot_tools_revision):
        raise ValueError('Select a bounded job count and an exact depot_tools revision')
    project = args.project.absolute()
    ordinary(project, project, False)
    receipt_path = ordinary(project / 'preparation-receipt.json', project)
    receipt = json.loads(receipt_path.read_text(encoding='utf-8'))
    target = receipt.get('target')
    if receipt.get('schema_version') != 1 or receipt.get('source_only') is not True or target not in HOSTS:
        raise ValueError('Select an authored desktop source project')
    if HOSTS[target] != platform.system():
        raise ValueError('The desktop target requires its matching native build host')
    if target == 'windows-x86_64' and platform.machine().lower() not in {'amd64', 'x86_64'}:
        raise ValueError('The Windows build recipe requires an x64 host')
    if target == 'linux-x86_64' and platform.machine().lower() not in {'amd64', 'x86_64'}:
        raise ValueError('The Linux build recipe requires an x64 host')
    if receipt.get('upstream_repository') != REPOSITORY or not re.fullmatch('[0-9a-f]{40}', receipt.get('upstream_revision', '')):
        raise ValueError('The Chromium repository/revision must remain fixed')
    source = check_inventory(project, receipt)
    if digest(Path(__file__).resolve()) != digest(source / 'recipe/tools/build_desktop_browser.py') or digest(Path(__file__).resolve().with_name('prepare_desktop_browser.py')) != digest(source / 'recipe/tools/prepare_desktop_browser.py'):
        raise ValueError('Run the build recipe frozen with this desktop source project')
    if digest(Path(__file__).resolve().with_name('chromium_dependencies.py')) != digest(source / 'recipe/tools/chromium_dependencies.py'):
        raise ValueError('Run the dependency admission code frozen with this project')
    git = args.git.resolve(strict=True)
    python = args.python.resolve(strict=True)
    depot = args.depot_tools.resolve(strict=True)
    for tool in [git, python, depot / 'gclient.py', depot / 'autoninja.py']:
        if not tool.is_file():
            raise ValueError('An explicitly selected desktop build tool is missing')
    if git_text(git, depot, 'rev-parse', 'HEAD') != args.depot_tools_revision or git_text(git, depot, 'status', '--porcelain', '--untracked-files=no'):
        raise ValueError('depot_tools must match its explicit clean revision')
    lock = project / 'build.lock'
    descriptor = os.open(lock, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        try:
            os.write(descriptor, str(os.getpid()).encode('ascii'))
        finally:
            os.close(descriptor)
        build(project, source, receipt, args, git, python, depot)
    finally:
        lock.unlink(missing_ok=True)


def build(project, source, receipt, args, git, python, depot):
    journal_path = project / 'build-journal.json'
    receipt_hash = digest(project / 'preparation-receipt.json')
    identity = {'preparation_sha256': receipt_hash, 'depot_tools_revision': args.depot_tools_revision,
                'python_sha256': digest(python), 'git_sha256': digest(git)}
    journal = {'schema_version': 1, 'identity': identity, 'phase': 'new', 'running': None, 'failed': None}
    if journal_path.exists():
        journal = json.loads(ordinary(journal_path, project).read_text(encoding='utf-8'))
        if journal.get('identity') != identity or journal.get('phase') not in {'new', 'synced', 'hooks', 'overlay', 'configured', 'built'}:
            raise ValueError('The existing build journal belongs to different inputs')
    workspace = project / 'workspace'
    if not journal_path.exists() and workspace.exists():
        raise ValueError('A new build requires a fresh owned dependency workspace')
    workspace.mkdir(exist_ok=True)
    ordinary(workspace, project, False)
    gclient = workspace / '.gclient'
    configuration = 'solutions = ' + repr([{'name': 'src', 'url': REPOSITORY, 'managed': False,
        'custom_deps': {}, 'custom_vars': {}}]) + '\n'
    if gclient.exists() and gclient.read_text(encoding='utf-8') != configuration:
        raise ValueError('The owned gclient configuration changed')
    gclient.write_text(configuration, encoding='utf-8')
    environment = os.environ.copy()
    environment['PATH'] = str(depot) + os.pathsep + str(git.parent) + os.pathsep + environment.get('PATH', '')
    environment['DEPOT_TOOLS_UPDATE'] = '0'
    # Keep source bytes and long-path support local to this build's Git children;
    # do not rewrite the user's global Git configuration.
    git_config_count = int(environment.get('GIT_CONFIG_COUNT', '0'))
    for key, value in [('core.autocrlf', 'false'), ('core.longpaths', 'true')]:
        environment['GIT_CONFIG_KEY_' + str(git_config_count)] = key
        environment['GIT_CONFIG_VALUE_' + str(git_config_count)] = value
        git_config_count += 1
    environment['GIT_CONFIG_COUNT'] = str(git_config_count)
    if platform.system() == 'Windows':
        environment['DEPOT_TOOLS_WIN_TOOLCHAIN'] = '0'
    persist(journal_path, journal)
    revision = receipt['upstream_revision']
    checkout = workspace / 'src'
    if journal['phase'] == 'new':
        command([python, depot / 'gclient.py', 'sync', '--nohooks', '--no-history', '--revision', 'src@' + revision],
                workspace, environment, journal, journal_path, 'dependency_sync')
        journal['phase'] = 'synced'
        persist(journal_path, journal)
    if git_text(git, checkout, 'rev-parse', 'HEAD') != revision:
        raise ValueError('The hydrated Chromium checkout differs from its frozen revision')
    # A resumed gclient sync may skip an incomplete checkout because its parent
    # gitlink already carries the requested pin. Admit actual HEAD and bytes
    # before hooks can silently substitute an unknown dependency version.
    dependency_inventory = verify_dependency_inventory(workspace, git)
    persist(project / 'dependency-git-inventory.json', dependency_inventory)
    if journal['phase'] == 'synced':
        command([python, depot / 'gclient.py', 'runhooks'], workspace, environment,
                journal, journal_path, 'upstream_hooks')
        journal['phase'] = 'hooks'
        persist(journal_path, journal)
    series = json.loads((source / 'recipe/browser/patches/series.json').read_text(encoding='utf-8'))
    changed = set(series['modified_files'])
    resources = {RESOURCE_ROOT + '/' + relative(name).as_posix() for name in series['resource_files']}
    if journal['phase'] == 'hooks':
        # Preflight every destination before copying any overlay bytes. A retry
        # accepts exact old/new bytes; unrelated edits always remain untouched.
        for name in sorted(changed):
            destination = ordinary(checkout / relative(name), checkout)
            original = (source / 'upstream' / name).read_bytes().replace(b'\r\n', b'\n')
            updated = (source / 'overlay' / name).read_bytes().replace(b'\r\n', b'\n')
            if destination.read_bytes().replace(b'\r\n', b'\n') not in {original, updated}:
                raise ValueError('The new desktop checkout has an unrelated edit: ' + name)
        for name in sorted(resources):
            destination = ordinary(checkout / relative(name), checkout, False)
            if destination.exists() and digest(destination) != digest(source / 'overlay' / name):
                raise ValueError('An unrelated file occupies a desktop resource: ' + name)
        for name in sorted(changed | resources):
            destination = checkout / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source / 'overlay' / name, destination)
        journal['phase'] = 'overlay'
        persist(journal_path, journal)
    for name in sorted(changed | resources):
        if digest(ordinary(checkout / name, checkout)) != digest(source / 'overlay' / name):
            raise ValueError('The hydrated desktop integration changed: ' + name)
    destination = checkout / 'out/OmniPlay'
    destination.mkdir(parents=True, exist_ok=True)
    gn_arguments = (source / 'recipe/args.gn').read_bytes()
    if (destination / 'args.gn').exists() and (destination / 'args.gn').read_bytes() != gn_arguments:
        raise ValueError('The desktop build contains different GN arguments')
    (destination / 'args.gn').write_bytes(gn_arguments)
    host_directory = {'Windows': 'win', 'Linux': 'linux64', 'Darwin': 'mac'}[platform.system()]
    gn = checkout / 'buildtools' / host_directory / ('gn.exe' if platform.system() == 'Windows' else 'gn')
    if not gn.is_file():
        raise ValueError('The pinned Chromium hooks did not supply the host GN binary')
    if journal['phase'] == 'overlay':
        command([gn, 'gen', 'out/OmniPlay'], checkout, environment, journal, journal_path, 'gn_configuration')
        journal['phase'] = 'configured'
        persist(journal_path, journal)
    if journal['phase'] == 'configured':
        command([python, depot / 'autoninja.py', '-C', 'out/OmniPlay', '-j', args.jobs, 'chrome'],
                checkout, environment, journal, journal_path, 'chromium_compilation')
        journal['phase'] = 'built'
        persist(journal_path, journal)
    if platform.system() == 'Darwin':
        candidates = []
        for bundle in destination.glob('*.app'):
            info_path = bundle / 'Contents/Info.plist'
            if not info_path.is_file():
                continue
            info = plistlib.loads(info_path.read_bytes())
            if info.get('CFBundleIdentifier') != 'app.omniplay.browser':
                continue
            executable = info.get('CFBundleExecutable')
            if not isinstance(executable, str) or not executable or '/' in executable or '\\' in executable or executable in {'.', '..'}:
                raise ValueError('The built macOS browser has an invalid executable identity')
            candidates.append(bundle / 'Contents/MacOS' / executable)
        if len(candidates) != 1:
            raise ValueError('The build must contain exactly one OmniPlay browser bundle')
        binary = candidates[0]
    else:
        binary = destination / ('chrome.exe' if platform.system() == 'Windows' else 'chrome')
    if not binary.is_file():
        raise ValueError('The desktop build did not produce its expected browser entry point')
    dependencies = subprocess.check_output([str(python), str(depot / 'gclient.py'), 'revinfo', '--actual'],
        cwd=workspace, env=environment)
    (project / 'dependency-revisions.txt').write_bytes(dependencies)
    persist(project / 'build-receipt.json', {
        'schema_version': 1, **identity, 'upstream_revision': revision,
        'target': receipt['target'], 'development_only': receipt['development_only'],
        'configuration': 'completed', 'compiler': 'completed',
        'binary_relative_path': binary.relative_to(project).as_posix(),
        'binary_bytes': binary.stat().st_size, 'binary_sha256': digest(binary),
        'dependency_revisions_sha256': hashlib.sha256(dependencies).hexdigest(),
        'dependency_git_inventory_sha256': digest(project / 'dependency-git-inventory.json'),
        'gn_sha256': digest(gn), 'browser': 'not_started', 'verification': 'not_run',
        'reproducibility_verified': False, 'distribution_approved': False,
    })
    print('Chromium compilation completed. Browser behavior, reproducibility and release qualification remain pending.')


if __name__ == '__main__':
    main()
