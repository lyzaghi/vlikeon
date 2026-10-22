"""Prepare/build a pinned software PS1 WASM candidate, never run it or tests.

--prepare-only materializes corresponding source and recipes without invoking
Make, a compiler, linker, application or core. Build mode is explicit and keeps
distribution_approved false until the separate release admission process.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
from web_core_options_policy import validate_policy
from psx_source_adaptations import include_all_card_data

ROOT = Path(__file__).resolve().parents[1]
PIN = 'df2b53b306ba52d2116f8e8cc93b9ee33d239be7'
VERSION = '0.9.44.1+omniplay.web1'
PRODUCT_VERSION = '0.9.44+1.omniplay.web1'
ADAPTERS = ROOT / 'engines/web/adapters'


def file_hash(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(65536), b''):
            digest.update(block)
    return digest.hexdigest()


def source_inventory(source):
    result = {}
    for path in sorted(source.rglob('*')):
        if path.is_symlink() or getattr(path, 'is_junction', lambda: False)():
            raise ValueError('Prepared source cannot contain filesystem links')
        if path.is_file():
            result[path.relative_to(source).as_posix()] = file_hash(path)
        elif not path.is_dir():
            raise ValueError('Prepared source contains a special file')
    return result


def replace(path, old, new):
    source = path.read_text(encoding='utf-8')
    if source.count(old) != 1:
        raise ValueError('The pinned PS1 Web adaptation anchor changed: ' + path.name)
    path.write_text(source.replace(old, new), encoding='utf-8', newline='\n')


def materialize(work, git):
    original = ROOT / 'third_party/sources/beetle-psx'
    revision = subprocess.check_output([git, '-C', str(original), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != PIN:
        raise ValueError('The PS1 source checkout differs from the pinned revision')
    if work.exists() or work.resolve() != work.absolute() or not work.resolve().is_relative_to(ROOT):
        raise ValueError('PS1 Web source needs a fresh unlinked output directory in the workspace')
    work.mkdir(parents=True)
    archive = work / 'upstream-source.tar'
    subprocess.run([git, '-C', str(original), 'archive', '--format=tar', '--output=' + str(archive), PIN], check=True)
    if archive.stat().st_size > 512 * 1024 * 1024:
        raise ValueError('The pinned PS1 source archive exceeds its bound')
    source = work / 'source'
    source.mkdir()
    total = count = 0
    with tarfile.open(archive, 'r:') as contents:
        for member in contents:
            name = PurePosixPath(member.name)
            count += 1
            if count > 100000 or name.is_absolute() or not name.parts or any(part in {'.', '..'} or '\\' in part or ':' in part for part in name.parts):
                raise ValueError('The source archive has an unsafe entry')
            path = source.joinpath(*name.parts)
            if not path.resolve().is_relative_to(source.resolve()) or member.issym() or member.islnk():
                raise ValueError('The source archive cannot contain links or escape its owned tree')
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                total += member.size
                if member.size < 0 or member.size > 256 * 1024 * 1024 or total > 512 * 1024 * 1024:
                    raise ValueError('The source archive exceeds its file budget')
                path.parent.mkdir(parents=True, exist_ok=True)
                with contents.extractfile(member) as incoming, path.open('xb') as outgoing:
                    shutil.copyfileobj(incoming, outgoing, 65536)
            else:
                raise ValueError('The source archive contains a special file')
    return source, archive


def prepare(work, git):
    source, archive = materialize(work, git)
    include_all_card_data(source)
    # Emscripten's upstream branch otherwise enables GPU and whole-disc precache.
    # Make command-line settings keep software/no-thread flags authoritative.
    replace(source / 'libretro.c', 'static bool cdimagecache = true;', 'static bool cdimagecache = false; /* OmniPlay: bounded positional source reads. */')
    replace(source / 'Makefile', '   FLAGS   += -msimd128 -ftree-vectorize', '   # OmniPlay baseline does not require WASM SIMD or executable-code backends.')
    # This is a standalone WASM factory; no external RetroArch frontend supplies
    # the libretro-common VFS/string/stream functions omitted by upstream archives.
    replace(source / 'Makefile.common', '   ifneq ($(STATIC_LINKING), 1)\n      SOURCES_C +=', '   ifeq (1, 1) # OmniPlay standalone WASM frontend\n      SOURCES_C +=')
    replace(source / 'libretro_options.h', '#define MEDNAFEN_CORE_SETVERSION "0.9.44.1"', '#define MEDNAFEN_CORE_SETVERSION "' + VERSION + '"')
    shutil.copy2(ADAPTERS / 'beetle-card-writer.c', source / 'mc_async.c')
    with (source / 'libretro.c').open('a', encoding='utf-8', newline='\n') as core:
        core.write((ADAPTERS / 'beetle-saved-files.c.inc').read_text(encoding='utf-8'))
    frontend = source / 'omniplay-frontend.c'
    shutil.copy2(ROOT / 'engines/web/frontend.c', frontend)
    exports = json.loads((ROOT / 'engines/web/exports.json').read_text(encoding='utf-8')) + ['_om_web_flush_save_io']
    (work / 'exports.json').write_text(json.dumps(exports, indent=2) + '\n', encoding='utf-8')
    shutil.copytree(ROOT / 'engines/web', work / 'omniplay-web-runtime')
    option_policy=ROOT / 'engines/policies/beetle-options.json'
    validate_policy(json.loads(option_policy.read_text(encoding='utf-8')))
    shutil.copy2(option_policy,work / 'core-options-policy.json')
    shutil.copy2(ROOT / 'tools/web_core_options_policy.py',work / 'web_core_options_policy.py')
    shutil.copy2(ROOT / 'tools/psx_source_adaptations.py',work / 'psx_source_adaptations.py')
    shutil.copy2(Path(__file__), work / Path(__file__).name)
    provenance = {
        'schema_version': 1, 'core': 'beetle-psx-web', 'revision': PIN,
        'upstream_source_sha256': file_hash(archive),
        'adaptations': ['software_interpreter', 'no_threads_jit_gpu_simd', 'no_direct_cdrom',
                        'bounded_file_backed_disc_reads_no_precache', 'retained_standalone_vfs',
                        'synchronous_periodic_card_writer', 'checked_all_card_save_barrier',
                        'snapshots_always_include_memory_card_bytes',
                        'shared_disc_and_persistent_file_envelopes'],
        'frontend_source_sha256': hashlib.sha256(frontend.read_bytes()).hexdigest(),
        'adapter_sha256': {name: hashlib.sha256((ADAPTERS / name).read_bytes()).hexdigest()
                          for name in ['beetle-card-writer.c', 'beetle-saved-files.c.inc']},
        'prepared_source_sha256': source_inventory(source),
        'core_options_policy_sha256': file_hash(work / 'core-options-policy.json'),
        'source_only': True, 'verification': 'not_run', 'distribution_approved': False,
        'upstream_openbios': 'Retained original MIT PCSX-Redux implementation; not manufacturer firmware',
        'remaining': ['exact SDK/build provenance', 'compiled dependency licenses', 'firmware/region qualification',
                      'real core I/O/restart/rollback qualification', 'device compatibility/memory/performance',
                      'security and release admission'],
    }
    (work / 'corresponding-source.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    return source, provenance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare-only', action='store_true')
    parser.add_argument('--build-prepared', action='store_true', help='Build the existing exact recorded source instead of recreating it')
    parser.add_argument('--git', default='git')
    parser.add_argument('--make', type=Path)
    parser.add_argument('--emcc', type=Path)
    parser.add_argument('--emxx', type=Path)
    parser.add_argument('--emar', type=Path)
    parser.add_argument('--parallel', type=int, default=4)
    args = parser.parse_args()
    if not 1 <= args.parallel <= 32:
        raise ValueError('Build parallelism exceeds its bound')
    if args.prepare_only and args.build_prepared:
        raise ValueError('Choose source preparation or an explicit prepared-source build')
    if not args.prepare_only and any(value is None for value in [args.make, args.emcc, args.emxx, args.emar]):
        raise ValueError('Build mode requires explicit Make and Emscripten compiler/archive paths')
    work = ROOT / 'out/web-cores/beetle-psx'
    if args.build_prepared:
        if work.resolve() != work.absolute() or not work.resolve().is_relative_to(ROOT):
            raise ValueError('Prepared source must stay in its owned workspace directory')
        source = work / 'source'
        provenance = json.loads((work / 'corresponding-source.json').read_text(encoding='utf-8'))
        if provenance.get('revision') != PIN or provenance.get('source_only') is not True or source_inventory(source) != provenance.get('prepared_source_sha256'):
            raise ValueError('The prepared source differs from its retained assembly receipt')
        expected_exports = json.loads((ROOT / 'engines/web/exports.json').read_text(encoding='utf-8')) + ['_om_web_flush_save_io']
        if json.loads((work / 'exports.json').read_text(encoding='utf-8')) != expected_exports:
            raise ValueError('The prepared frontend export contract changed')
    else:
        source, provenance = prepare(work, args.git)
    if args.prepare_only:
        print('Authored pinned PS1 Web corresponding source. No compiler, build, core or test was started.')
        return
    policy_file=work / 'core-options-policy.json'
    if file_hash(policy_file)!=provenance.get('core_options_policy_sha256'):
        raise ValueError('The retained core option policy differs from its assembly receipt')
    option_policy=validate_policy(json.loads(policy_file.read_text(encoding='utf-8')))
    archive = work / 'beetle-psx.a'
    command = [args.make, '-f', 'Makefile', 'platform=emscripten', 'CC=' + str(args.emcc),
               'CXX=' + str(args.emxx), 'AR=' + str(args.emar), 'TARGET=' + str(archive),
               'HAVE_HW=0', 'HAVE_OPENGL=0', 'HAVE_VULKAN=0', 'HAVE_LIGHTREC=0', 'GLES=0', 'GLES3=0',
               'THREADED_RECOMPILER=0', 'NEED_THREADING=0', 'HAVE_CDROM=0', 'HAVE_CHD=1',
               'PTHREAD_FLAGS=', 'GIT_VERSION=" unknown"', 'EXTRA_FLAGS=-DHAVE_NO_LANGEXTRA', '-j' + str(args.parallel)]
    subprocess.run([str(value) for value in command], cwd=source, check=True)
    output = ROOT / 'apps/game-hub/cores/beetle-psx'
    if output.exists() or output.resolve() != output.absolute() or not output.resolve().is_relative_to(ROOT):
        raise ValueError('Preserve existing core artifacts before making a fresh Web candidate')
    output.mkdir(parents=True)
    common = ['-O3', '-sENVIRONMENT=worker', '-sSTACK_SIZE=2097152', '-sINITIAL_MEMORY=67108864', '-sALLOW_MEMORY_GROWTH=1', '-sMAXIMUM_MEMORY=1073741824',
              '-sALLOW_TABLE_GROWTH=1', '-sFILESYSTEM=1']
    frontend_object = work / 'omniplay-frontend.o'
    subprocess.run([str(args.emcc), '-std=c11', *common, '-c', str(source / 'omniplay-frontend.c'), '-o', str(frontend_object)], check=True)
    module = output / 'beetle-psx.js'
    subprocess.run([str(args.emxx), *common, '-Wl,--whole-archive', str(archive), '-Wl,--no-whole-archive', str(frontend_object),
                    '--no-entry', '-sMODULARIZE=1', '-sEXPORT_ES6=1', '-sEXPORTED_FUNCTIONS=@' + str(work / 'exports.json'),
                    '-sEXPORTED_RUNTIME_METHODS=@' + str(ROOT / 'engines/web/runtime-methods.json'), '-o', str(module)], check=True)
    wasm = output / 'beetle-psx.wasm'
    manifest = {
        'schema_version': 1, 'id': 'beetle-psx-web', 'core_version': PRODUCT_VERSION, 'runtime_version_string': VERSION,
        'source_revision': PIN, 'systems': ['ps1'], 'module': 'cores/beetle-psx/beetle-psx.js',
        'wasm': 'cores/beetle-psx/beetle-psx.wasm', 'wasm_bytes': wasm.stat().st_size,
        'wasm_sha256': file_hash(wasm), 'license_expression': 'GPL-2.0-or-later',
        'state_format_version': 'beetle-psx-' + PIN[:16] + '-omweb1',
        'in_game_format_version': 'beetle-psx-memory-and-cards-omweb1', 'minimum_memory_mb': 512,
        'capabilities': ['pause', 'gamepad', 'haptics', 'keyboard_mouse', 'touch_controls', 'save_ram', 'save_state', 'save_files', 'disc_control'],
        'content_formats': ['raw_rom', 'cue', 'm3u', 'chd', 'iso', 'pbp'], 'media_filesystem_protocol': 1,
        'disc_state_protocol': 1, 'saved_files_protocol': 1, 'saved_files_io': 'single_threaded_barrier_v1', 'wasm_threads': False,
        'core_options_protocol':1,'core_option_definitions':option_policy['definitions'],
        'core_options_policy_sha256':provenance['core_options_policy_sha256'],
        'frontend_source_sha256': provenance['frontend_source_sha256'], 'adapter_sha256': provenance['adapter_sha256'],
        'firmware_protocol': 1, 'firmware': [{'role': role, 'filename': name, 'allowed_bytes': [524288], 'required_group': 'ps1_bios'}
                                           for role, name in [('ps1_bios_jp', 'scph5500.bin'), ('ps1_bios', 'scph5501.bin'), ('ps1_bios_eu', 'scph5502.bin')]],
        'distribution_approved': False, 'verification': 'not_run',
    }
    shutil.copy2(source / 'COPYING', output / 'LICENSE.beetle-psx.txt')
    shutil.copy2(source / 'deps/openbios/LICENSE.txt', output / 'LICENSE.openbios.txt')
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    provenance['source_only'] = False
    (work / 'build-receipt.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    print('Built an unapproved PS1 Web candidate. No game, browser, compatibility or test was run.')


if __name__ == '__main__':
    main()
