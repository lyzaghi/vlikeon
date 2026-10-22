"""Materialize native PSP source from committed archives, without running a build.

Working-tree bytes, ignored/untracked files, submodule checkout contents and
filesystem links never become implicit product inputs. Prebuilt executable and
game-image omissions are explicit inventory entries at the selected revision.
"""
from pathlib import PurePosixPath
import re
import shutil
import subprocess
import tarfile
from pinned_core_source import ROOT, owned, file_hash


POLICY = 'ppsspp_native_committed_source_v1'
EXCLUDED = re.compile(r'\.(?:o|obj|a|lib|dll|exe|dylib|iso|cso|pbp|chd)$|\.so(?:\.[0-9]+)*$', re.IGNORECASE)
ARCHIVE_LIMIT = 512 * 1024 * 1024
OMITTED_GITLINKS = {
    'SDL/macOS': 'macOS application dependency outside the Windows libretro target',
    'assets/debugger': 'browser debugger not part of the private runtime',
    'ext/OpenXR-SDK': 'OpenXR disabled in the fixed product configuration',
    'ext/discord-rpc': 'Discord disabled in the fixed product configuration',
    'ext/freetype': 'atlas tool uses its separately pinned font-tool source',
    'ext/libadrenotools': 'Android application dependency outside the Windows libretro target',
    'ext/miniupnp': 'UPnP disabled in the fixed product configuration',
    'ext/oboe': 'Android application dependency outside the Windows libretro target',
    'frametests': 'upstream frame-test project outside product source selection',
    'ios/MoltenVK': 'iOS application dependency outside the Windows libretro target',
    'pspautotests': 'upstream guest-test project outside product source selection',
}


def safe_name(raw):
    name = PurePosixPath(raw)
    if name.is_absolute() or not name.parts or len(name.parts) > 32 or '\\' in raw:
        raise ValueError('The committed source archive has an unsafe path')
    for part in name.parts:
        if part in {'.', '..'} or len(part.encode('utf-8')) > 255 or part.endswith(('.', ' ')) or any(ord(char) < 32 or char in ':<>"|?*' for char in part):
            raise ValueError('The committed source archive has an unsafe component')
        if part.split('.')[0].upper() in {'CON', 'PRN', 'AUX', 'NUL', *('COM' + str(i) for i in range(1, 10)), *('LPT' + str(i) for i in range(1, 10))}:
            raise ValueError('The committed source archive has a reserved component')
    return name


def materialize(repository, revision, work, git='git', selected_paths=(), exclude_assets=False):
    original = ROOT / 'third_party/sources' / repository
    if subprocess.check_output([git, '-C', str(original), 'rev-parse', 'HEAD'], text=True).strip() != revision:
        raise ValueError('The source cache differs from its exact selected revision')
    owned(work)
    if work.exists():
        raise ValueError('Preserve prior source; use a fresh materialization directory')
    work.mkdir(parents=True)
    archive = work / 'upstream-source.tar'
    if not isinstance(selected_paths, (tuple, list)) or len(selected_paths) > 128 or any(not re.fullmatch(r'[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*', name) or any(part in {'.', '..'} for part in name.split('/')) for name in selected_paths):
        raise ValueError('The native source selection must use explicit committed paths')
    subprocess.run([git, '-C', str(original), 'archive', '--format=tar', '--output=' + str(archive), revision, *selected_paths], check=True)
    if archive.stat().st_size > ARCHIVE_LIMIT:
        raise ValueError('The committed native-source archive exceeds its bound')
    archive_hash = file_hash(archive)
    source = work / 'source'
    source.mkdir()
    count = total = 0
    aliases = {}
    omitted = []
    source_root = source.resolve()
    with tarfile.open(archive, 'r:') as contents:
        for member in contents:
            name = safe_name(member.name)
            count += 1
            total += member.size
            if count > 100000 or member.size < 0 or member.size > 256 * 1024 * 1024 or total > ARCHIVE_LIMIT:
                raise ValueError('The committed native source exceeds its file budget')
            for index in range(1, len(name.parts) + 1):
                prefix = '/'.join(name.parts[:index])
                if aliases.setdefault(prefix.upper(), prefix) != prefix:
                    raise ValueError('The committed source aliases a Windows path')
            path = source.joinpath(*name.parts)
            if not path.resolve().is_relative_to(source_root):
                raise ValueError('The committed source escapes its output root')
            if exclude_assets and name.parts[0] == 'assets':
                if not member.isdir():
                    omitted.append({'path': name.as_posix(), 'bytes': member.size, 'reason': 'atlas_tool_excludes_product_assets'})
                continue
            if not member.isdir() and EXCLUDED.search(name.name):
                omitted.append({'path': name.as_posix(), 'bytes': member.size,
                    'kind': 'symbolic_link' if member.issym() else 'hard_link' if member.islnk() else 'file',
                    'reason': 'prebuilt_executable_or_game_image'})
                continue
            if member.issym() or member.islnk():
                raise ValueError('Native source cannot materialize filesystem links')
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                path.parent.mkdir(parents=True, exist_ok=True)
                with contents.extractfile(member) as incoming, path.open('xb') as outgoing:
                    shutil.copyfileobj(incoming, outgoing, 65536)
            else:
                raise ValueError('Native source cannot materialize special files')
    # Keep immutable identity and omission evidence, without redistributing an
    # additional archive containing deliberately omitted binary/game objects.
    archive.unlink()
    return source, {'repository': repository, 'revision': revision,
        'archive_sha256': archive_hash, 'archive_members': count,
        'archive_member_bytes': total, 'selected_committed_paths': selected_paths,
        'exclude_product_assets': exclude_assets, 'omitted': omitted}


def prepare(work, lock, git='git', exclude_assets=False):
    owned(work)
    if work.exists():
        raise ValueError('Use a fresh committed PSP source project')
    parent, parent_record = materialize('ppsspp', lock['core']['revision'], work, git, exclude_assets=exclude_assets)
    # Verify the selected dependency revisions against committed gitlinks rather
    # than trusting a separately editable lock to redefine the parent's source.
    original = ROOT / 'third_party/sources/ppsspp'
    tree = subprocess.check_output([git, '-C', str(original), 'ls-tree', '-rz', lock['core']['revision']])
    if len(tree) > 16 * 1024 * 1024:
        raise ValueError('The committed PSP tree inventory exceeds its bound')
    gitlinks = {}
    for entry in tree.split(b'\0'):
        if not entry:
            continue
        metadata, name = entry.split(b'\t', 1)
        mode, kind, object_id = metadata.split(b' ')
        if mode == b'160000':
            gitlinks[name.decode('utf-8')] = object_id.decode('ascii')
    selected = {entry['path']: entry['revision'] for entry in lock['dependencies']}
    if len(selected) != len(lock['dependencies']) or any(gitlinks.get(path) != revision for path, revision in selected.items()) or set(gitlinks) - set(selected) != set(OMITTED_GITLINKS):
        raise ValueError('The PSP dependency lock differs from committed gitlinks')
    inputs = work / 'inputs'
    inputs.mkdir()
    records = {'ppsspp': parent_record}
    for entry in lock['dependencies']:
        if entry['name'] == 'ffmpeg':
            continue  # The separate modern decoder adapter records this override.
        name = entry['name']
        if not re.fullmatch(r'[A-Za-z0-9_.-]{1,96}', name):
            raise ValueError('The dependency cache name is not a literal component')
        relative = safe_name(entry['path'])
        selection = ('LICENSE', 'COPYING', 'lib') if name == 'ext-zstd' else ()
        dependency, record = materialize('ppsspp-deps/' + name, entry['revision'], inputs / name, git, selection)
        target = owned(parent.joinpath(*relative.parts))
        if target.exists():
            if target.is_symlink() or any(target.iterdir()):
                raise ValueError('The selected gitlink destination contains unrelated source')
            target.rmdir()
        target.parent.mkdir(parents=True, exist_ok=True)
        dependency.rename(target)
        records[name] = record
    return parent, {'schema_version': 1, 'policy': POLICY,
        'source_only': True, 'verification': 'not_run', 'inputs': records,
        'omitted_gitlinks': {path: {'revision': gitlinks[path], 'reason': reason} for path, reason in OMITTED_GITLINKS.items()},
        'overridden_parent_decoder_revision': next(entry['revision'] for entry in lock['dependencies'] if entry['name'] == 'ffmpeg')}
