"""Committed PSP/font source authoring and a separate explicit atlas build."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from build_ppsspp import PIN, prepare, replace, ATLAS_FONT_PINS as EXPECTED, RECIPES
from pinned_core_source import ROOT, owned, file_hash, source_inventory
from ppsspp_native_source import materialize, safe_name
from ppsspp_atlas_receipt import POLICY

FONT_INPUTS = [('noto-fonts', 'hinted/ttf/NotoSans/NotoSans-Regular.ttf', 'UWEeGRVS'),
    ('noto-cjk', 'Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf', 'khcJK'),
    ('noto-fonts', 'hinted/ttf/NotoSansHebrew/NotoSansHebrew-Regular.ttf', 'H'),
    ('noto-fonts', 'hinted/ttf/NotoSansThai/NotoSansThai-Regular.ttf', 'T')]
RECIPE_FILES = (*RECIPES, 'ppsspp_atlas_committed.py', 'build_ppsspp_atlas.py')
LOCKS = ('ppsspp-atlas-source.lock.json', 'ppsspp-source.lock.json', 'ffmpeg-source.lock.json', 'committed-source.json')


def inventory(path):
    return {'source': str(path.resolve()), 'bytes': path.stat().st_size, 'sha256': file_hash(path)}


def fonts_from_commit(entry, output):
    if entry.get('inventory_encoding') != 'raw_committed_git_blob':
        raise ValueError('The atlas requires raw committed font/notice bytes, without checkout line-ending conversion')
    original = ROOT / 'third_party/sources/ppsspp-atlas-deps' / entry['name']
    if subprocess.check_output(['git', '-C', str(original), 'rev-parse', 'HEAD'], text=True).strip() != entry['revision']:
        raise ValueError('The font cache differs from its selected revision')
    expected = set(entry['files'])
    if expected != {item['path'] for item in entry['inventory']} or len(expected) != len(entry['inventory']) or len(expected) > 16:
        raise ValueError('The font inventory differs from its finite selection')
    for item in entry['inventory']:
        relative = safe_name(item['path']); spec = entry['revision'] + ':' + relative.as_posix()
        object_id = subprocess.check_output(['git', '-C', str(original), 'rev-parse', spec], text=True).strip()
        size = int(subprocess.check_output(['git', '-C', str(original), 'cat-file', '-s', spec], text=True).strip())
        if object_id != item['blob'] or not 0 < size <= 32 * 1024 * 1024 or size != item['bytes']:
            raise ValueError('A font/notice object differs from its locked identity')
        value = subprocess.check_output(['git', '-C', str(original), 'show', spec])
        if len(value) != size or hashlib.sha256(value).hexdigest() != item['sha256']:
            raise ValueError('Materialized font/notice bytes differ from their lock')
        target = output.joinpath(*relative.parts); target.parent.mkdir(parents=True, exist_ok=True)
        with target.open('xb') as stream: stream.write(value)
    return output


def prepare_project(work):
    font_lock = json.loads((ROOT / 'security/ppsspp-atlas-source.lock.json').read_text(encoding='utf-8'))
    core_lock = json.loads((ROOT / 'security/ppsspp-source.lock.json').read_text(encoding='utf-8'))
    if font_lock.get('schema_version') != 2 or font_lock['core_revision'] != PIN or core_lock['core']['revision'] != PIN or {entry['name']: entry['revision'] for entry in font_lock['sources']} != EXPECTED or len(font_lock['sources']) != len(EXPECTED):
        raise ValueError('The atlas source identities differ from the selected pins')
    source = work / 'source'
    prepare(ROOT / 'third_party/sources/ppsspp', source, core_lock, exclude_assets=True)
    sources = {}; records = {}
    for entry in font_lock['sources']:
        if entry['name'] == 'freetype':
            tree, record = materialize('ppsspp-atlas-deps/freetype', EXPECTED['freetype'], work / 'font-inputs/freetype')
            nested = subprocess.check_output(['git', '-C', str(ROOT / 'third_party/sources/ppsspp-atlas-deps/freetype'), 'ls-tree', EXPECTED['freetype'], '--', 'subprojects/dlg'], text=True).strip()
            if nested != '160000 commit 395ccad2c1e0daae535c4d20bb0a3f2424648e17\tsubprojects/dlg':
                raise ValueError('The FreeType logging gitlink identity changed')
            replace(tree / 'include/freetype/config/ftoption.h', '/* #define FT_DEBUG_LOGGING */', '#undef FT_DEBUG_LOGGING')
            record['omitted_gitlinks'] = {'subprojects/dlg': {'revision': '395ccad2c1e0daae535c4d20bb0a3f2424648e17', 'reason': 'FT_DEBUG_LOGGING explicitly undefined for the fixed atlas tool'}}
            target = owned(source / 'ext/freetype')
            if target.exists():
                if target.is_symlink() or any(target.iterdir()): raise ValueError('The FreeType destination contains unrelated source')
                target.rmdir()
            tree.rename(target); sources['freetype'] = target; records['freetype'] = record
        else:
            sources[entry['name']] = fonts_from_commit(entry, owned(source / 'omniplay-font-sources' / entry['name']))
            records[entry['name']] = {'revision': entry['revision'], 'committed_inventory': entry['inventory']}
    replace(source / 'CMakeLists.txt', 'target_link_libraries(AtlasTool ${CMAKE_THREAD_LIBS_INIT} freetype)', 'target_link_libraries(AtlasTool ${CMAKE_THREAD_LIBS_INIT} freetype Common zstd)')
    fonts = source / 'omniplay-fonts'; fonts.mkdir(); inputs = []; script = ['2048']
    for name, relative, ranges in FONT_INPUTS:
        target = fonts / Path(relative).name; shutil.copy2(sources[name] / relative, target)
        inputs.append({'repository': name, 'revision': EXPECTED[name], 'path': relative, **inventory(target)})
        script.append(f'font UBUNTU24 omniplay-fonts/{target.name} {ranges} 24 0')
    old = (source / 'ppge_atlasscript.txt').read_text(encoding='utf-8').splitlines()
    if old[:3] != ['2048', 'font UBUNTU24 C:/Windows/Fonts/KozGoPro-Medium.otf UWkhcJGRS 24', 'font UBUNTU24 C:/Windows/Fonts/ARIALUNI.ttf UWEhkcRGHKVTeS 24 0']:
        raise ValueError('The committed atlas script anchor changed')
    script.extend(old[3:]); script_path = source / 'omniplay_ppge_atlasscript.txt'; script_path.write_text('\n'.join(script) + '\n', encoding='utf-8')
    notices = source / 'omniplay-font-notices'; notices.mkdir()
    for name, relative, target in [('noto-cjk', 'Sans/LICENSE', 'LICENSE.NotoCJK.txt'), ('noto-fonts', 'LICENSE', 'LICENSE.NotoFonts.txt'), ('freetype', 'docs/FTL.TXT', 'LICENSE.FreeTypeTool.txt')]:
        shutil.copy2(sources[name] / relative, notices / target)
    for name in RECIPE_FILES: shutil.copy2(ROOT / 'tools' / name, work / name)
    for name in LOCKS[:-1]: shutil.copy2(ROOT / 'security' / name, work / name)
    metadata = {'schema_version': 1, 'kind': 'ppsspp_open_font_atlas', 'source_policy': POLICY, 'core_revision': PIN,
        'font_inputs': inputs, 'font_sources': EXPECTED, 'committed_font_sources': records, 'corresponding_source': str(source.resolve()), 'font_script': script_path.name,
        'additional_notice_paths': [path.relative_to(source).as_posix() for path in sorted(notices.iterdir())],
        'source_only': True, 'generated': False, 'distribution_approved': False, 'verification': 'not_run', 'configure': 'not_started', 'build': 'not_started', 'generator': 'not_started',
        'prepared_source_sha256': source_inventory(source), 'recipe_sha256': {name: file_hash(work / name) for name in RECIPE_FILES},
        'auxiliary_sha256': {name: file_hash(work / name) for name in LOCKS}}
    (work / 'source-preparation.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')


def build_project(args, work, output):
    record_path = work / 'source-preparation.json'; metadata = json.loads(record_path.read_text(encoding='utf-8'))
    if metadata.get('kind') != 'ppsspp_open_font_atlas' or metadata.get('source_policy') != POLICY or metadata.get('core_revision') != PIN or metadata.get('font_sources') != EXPECTED or metadata.get('source_only') is not True or metadata.get('generated') is not False:
        raise ValueError('The prepared atlas source identity changed')
    if source_inventory(work / 'source') != metadata['prepared_source_sha256'] or set(metadata['recipe_sha256']) != set(RECIPE_FILES) or set(metadata['auxiliary_sha256']) != set(LOCKS):
        raise ValueError('Prepared atlas source or required producer inputs changed')
    for name, expected in metadata['recipe_sha256'].items():
        if file_hash(work / name) != expected or file_hash(ROOT / 'tools' / name) != expected: raise ValueError('The atlas producer changed after preparation')
    for name, expected in metadata['auxiliary_sha256'].items():
        if file_hash(work / name) != expected: raise ValueError('The atlas provenance/lock changed after preparation')
    if output.exists(): raise ValueError('Preserve prior atlas build source; use a fresh output')
    output.mkdir(parents=True); source = output / 'source'; shutil.copytree(work / 'source', source)
    for name in (*RECIPE_FILES, *LOCKS, 'source-preparation.json'): shutil.copy2(work / name, output / name)
    build = output / 'build'
    command = [args.cmake, '-S', source, '-B', build, '-G', args.generator, '-DCMAKE_BUILD_TYPE=Release', '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded', '-DLIBRETRO=ON', '-DATLAS_TOOL=ON', '-DUNITTEST=OFF', '-DHEADLESS=OFF',
        '-DUSE_FFMPEG=OFF', '-DUSE_DISCORD=OFF', '-DUSE_MINIUPNPC=OFF', '-DOPENXR=OFF', '-DUSE_CCACHE=OFF', '-DUSE_SYSTEM_FREETYPE=OFF', '-DFT_DISABLE_ZLIB=ON', '-DFT_DISABLE_BZIP2=ON', '-DFT_DISABLE_PNG=ON',
        '-DFT_DISABLE_HARFBUZZ=ON', '-DFT_DISABLE_BROTLI=ON', '-DUSE_SYSTEM_LIBPNG=OFF', '-DUSE_SYSTEM_ZSTD=OFF', '-DUSE_SYSTEM_RAPIDJSON=OFF', '-DUSE_SYSTEM_LIBCHDR=OFF', '-DHTTPS_NOT_AVAILABLE=ON', '-DCMAKE_DISABLE_FIND_PACKAGE_ZLIB=ON', '-DBUILD_TESTING=OFF']
    subprocess.run([str(value) for value in command], check=True)
    subprocess.run([str(args.cmake), '--build', str(build), '--config', 'Release', '--target', 'AtlasTool', '--parallel', str(args.parallel)], check=True)
    binaries = list(build.rglob('AtlasTool.exe'))
    if len(binaries) != 1: raise ValueError('Expected one Windows atlas generator executable')
    subprocess.run([str(binaries[0]), metadata['font_script'], 'ppge', '8888'], cwd=source, check=True)
    assets = []
    for name in ('ppge_atlas.zim', 'ppge_atlas.meta'):
        path = source / name
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 32 * 1024 * 1024: raise ValueError('Generated atlas output is missing, linked or oversized')
        assets.append({'name': name, **inventory(path)})
    evidence = {'schema_version': 1, 'source_policy': POLICY, 'core_revision': PIN, 'font_sources': EXPECTED,
        'source_preparation_sha256': file_hash(output / 'source-preparation.json'), 'compiled_source_sha256': source_inventory(source)}
    evidence_path = output / 'source-evidence.json'
    evidence_path.write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
    compact = {key: value for key, value in metadata.items() if key != 'prepared_source_sha256'}
    result = dict(compact, assets=assets, generator_binary=inventory(binaries[0]), generated=True, source_only=False,
        corresponding_source=str(source.resolve()), prepared_source_receipt_sha256=file_hash(record_path),
        font_inputs=[{**item, **inventory(source / 'omniplay-fonts' / Path(item['source']).name)} for item in metadata['font_inputs']],
        configure='completed', build='completed', generator='completed',
        additional_notices=[str((source / relative).resolve()) for relative in metadata['additional_notice_paths']],
        source_evidence={'bytes': evidence_path.stat().st_size, 'sha256': file_hash(evidence_path)},
        cmake_sha256=file_hash(args.cmake), configure_command=[str(value) for value in command])
    (output / 'build-receipt.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print('Generated an unapproved PSP atlas from committed inputs. Visual, glyph, licensing and runtime verification remain pending.')


def main():
    parser = argparse.ArgumentParser(description=__doc__); mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare-only', action='store_true'); mode.add_argument('--build-prepared', action='store_true')
    parser.add_argument('--cmake', type=Path); parser.add_argument('--source-project', type=Path); parser.add_argument('--generator', default='Ninja'); parser.add_argument('--parallel', type=int, default=8)
    args = parser.parse_args()
    if not 1 <= args.parallel <= 32: raise ValueError('Choose bounded atlas build parallelism')
    work = owned(args.source_project.absolute() if args.source_project else ROOT / 'out/native-sources/ppsspp-atlas-committed')
    if args.prepare_only:
        prepare_project(work); print('Authored committed PSP, FreeType and selected open-font inputs and a separate recipe. No compiler, generator, runtime or test was started.'); return
    if args.cmake is None or not args.cmake.is_file(): raise ValueError('Supply an explicit actual CMake executable')
    args.cmake = args.cmake.resolve(strict=True); build_project(args, work, owned(ROOT / 'out/native-assets/ppsspp-atlas-committed'))


if __name__ == '__main__': main()
