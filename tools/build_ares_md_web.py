"""Prepare or explicitly build the pinned Mega Drive software WASM source.

Source preparation archives selected committed inputs, authors the adapted
project and retains its inventory. It never invokes a compiler, core or test.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

from pinned_core_source import ROOT, owned, file_hash, materialize, source_inventory
from build_ares_md import PIN, HEADER_PIN, COMPONENTS, SOURCE_PATHS, retire_inputs, prepare as prepare_selected, replace
from web_core_options_policy import validate_policy
from web_portable_files_policy import FORMAT, admit as admit_portable_policy

VERSION = '0.3.0+ares.9408cb43d494.omweb1'
COMPILE_SOURCES = ('adapter/adapter.cpp', 'adapter/core.cpp', 'ares/ares/memory/fixed-allocator.cpp',
                   'ares/md/md.cpp', 'thirdparty/qon/qon.cpp', 'libco/libco.c',
                   'omniplay-frontend.c', 'omniplay-results.c',
                   *('ares/component/' + name + '/' + name.split('/')[-1] + '.cpp' for name in COMPONENTS))
RECIPES = ('build_ares_md.py', 'build_ares_md_web.py', 'pinned_core_source.py', 'web_core_options_policy.py', 'web_portable_files_policy.py')
DEFINES = ('PROFILE_ACCURACY', 'NALL_HEADER_ONLY', 'BUILD_RELEASE',
           'OMNIPLAY_FILE_SAVES', 'OMNIPLAY_WEB_FILES', 'RETRO_EXPORT_SYMBOLS')
ADAPTER_INPUTS = {'adapter.cpp': 'engines/ares-md/adapter.cpp', 'web-files.hpp': 'engines/ares-md/web-files.hpp',
                  'save-memory.inc': 'engines/ares-md/save-memory.inc',
                  'bsnes-fibers.c': 'engines/web/adapters/bsnes-fibers.c',
                  'libretro-results.c': 'engines/web/adapters/libretro-results.c',
                  'windows-libco.c': 'engines/native/adapters/windows-libco.c',
                  'md-save-publish.c': 'engines/native/adapters/md-save-publish.c'}


def wasm_intrinsics(source):
    path = source / 'nall/nall/intrinsics.hpp'
    replace(path, '#if defined(_WIN32)\n  #define PLATFORM_WINDOWS', '''#if defined(__EMSCRIPTEN__)
  #define PLATFORM_WASM
  struct Platform {
    static constexpr bool Windows = 0, MacOS = 0, Android = 0, Linux = 0, BSD = 0;
  };
#elif defined(_WIN32)
  #define PLATFORM_WINDOWS''')
    replace(path, '#if defined(_WIN32)\n  #define ABI_WINDOWS', '''#if defined(__EMSCRIPTEN__)
  #define ABI_WASM
  struct ABI { static constexpr bool Windows = 0, SystemV = 0; };
#elif defined(_WIN32)
  #define ABI_WINDOWS''')
    replace(path, '#if defined(_WIN32)\n  #define DISPLAY_WINDOWS', '''#if defined(__EMSCRIPTEN__)
  struct DisplayServer { static constexpr bool Windows = 0, Quartz = 0, Xorg = 0; };
#elif defined(_WIN32)
  #define DISPLAY_WINDOWS''')
    replace(path, '#if defined(__i386__) || defined(_M_IX86)', '''#if defined(__wasm32__)
  #define ARCHITECTURE_WASM32
  struct Architecture {
    static constexpr bool x86 = 0, amd64 = 0, arm64 = 0, arm32 = 0;
    static constexpr bool ppc64 = 0, ppc32 = 0, rv64 = 0, rv32 = 0;
  };
#elif defined(__i386__) || defined(_M_IX86)''')
    replace(path, '#if (defined(__BYTE_ORDER) && defined(__LITTLE_ENDIAN)', '#if defined(__wasm32__) || (defined(__BYTE_ORDER) && defined(__LITTLE_ENDIAN)')


def prepare(work, git):
    if work.exists():
        raise ValueError('Preserve existing MD corresponding source before preparing a new assembly')
    owned(work).mkdir(parents=True)
    original, archive = materialize('ares', PIN, work / 'inputs', git, SOURCE_PATHS)
    archive_hash = file_hash(archive)
    sameboy = ROOT / 'third_party/sources/sameboy'
    if subprocess.check_output([git, '-C', str(sameboy), 'rev-parse', 'HEAD'], text=True).strip() != HEADER_PIN:
        raise ValueError('The libretro interface checkout differs from its pinned revision')
    header = subprocess.check_output([git, '-C', str(sameboy), 'show', HEADER_PIN + ':libretro/libretro.h'])
    if not 1024 <= len(header) <= 2 * 1024 * 1024:
        raise ValueError('The committed libretro interface exceeds its source bound')
    header_path = work / 'libretro-input.h'
    header_path.write_bytes(header)
    source = work / 'source'
    changes = prepare_selected(original, source, libretro_header=header_path)
    # Keep the original analyzer/list sources for review of the selected
    # transformations, without copying the resource aggregator or BIOS assets.
    (source / 'adapter/upstream').mkdir()
    shutil.copy2(original / 'mia/medium/mega-drive.cpp', source / 'adapter/upstream/cartridge-analysis.cpp')
    shutil.copy2(original / 'desktop-ui/emulator/mega-drive.cpp', source / 'adapter/upstream/controller-list.cpp')
    (source / 'CMakeLists.txt').unlink()  # The explicit SDK recipe supplies every translation unit.
    wasm_intrinsics(source)
    shutil.copy2(ROOT / 'engines/web/adapters/bsnes-fibers.c', source / 'libco/omniplay-fibers.c')
    shutil.copy2(ROOT / 'engines/web/adapters/LICENSE.libco', source / 'libco/LICENSE.wasm-fibers')
    (source / 'libco/libco.c').write_text('#include "omniplay-fibers.c"\n', encoding='utf-8')
    replace(source / 'adapter/adapter.cpp', 'constexpr char kVersion[]="0.3.0+ares.9408cb43d494";', 'constexpr char kVersion[]="' + VERSION + '";')
    shutil.copy2(ROOT / 'engines/web/frontend.c', source / 'omniplay-frontend.c')
    shutil.copy2(ROOT / 'engines/web/adapters/libretro-results.c', source / 'omniplay-results.c')
    if any(not (source / name).is_file() for name in COMPILE_SOURCES):
        raise ValueError('The selected MD Web translation unit is absent')
    changes = [item for item in changes if item not in {'windows_fibers_stable_handles_and_rederive','native_memory_file_table_without_host_write_fallback'}] + [
        'selected_committed_source_only', 'explicit_wasm32_platform_abi',
        'single_worker_asyncify_fibers', 'void_bridge_completion_slot',
        'checked_private_canonical_ram_eeprom_files', 'no_native_publisher_translation_unit',
        'portable_md_canonical_file_profile_v1']
    policy = ROOT / 'engines/policies/ares-md-options.json'
    validate_policy(json.loads(policy.read_text(encoding='utf-8')))
    shutil.copy2(policy, work / 'core-options-policy.json')
    exports = json.loads((ROOT / 'engines/web/exports.json').read_text(encoding='utf-8')) + ['_om_web_flush_save_io', '_om_web_libretro_call', '_om_md_describe_save']
    (work / 'exports.json').write_text(json.dumps(exports, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(ROOT / 'engines/web/runtime-methods.json', work / 'runtime-methods.json')
    shutil.copytree(ROOT / 'engines/web', work / 'omniplay-web-runtime')
    for name in RECIPES:
        shutil.copy2(ROOT / 'tools' / name, work / name)
    source_inputs = json.loads((source / 'SOURCE_INPUTS.json').read_text(encoding='utf-8'))
    source_inputs.update({'core_version': VERSION, 'target': 'wasm32',
                          'adapter_sha256': file_hash(source / 'adapter/adapter.cpp'),
                          'upstream_source_sha256': archive_hash})
    (source / 'SOURCE_INPUTS.json').write_text(json.dumps(source_inputs, indent=2) + '\n', encoding='utf-8')
    (source / 'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version': 1, 'upstream_revision': PIN,
        'changes': changes, 'initial_platform': 'wasm32', 'compiled': False, 'verification': 'not_run'}, indent=2) + '\n', encoding='utf-8')
    provenance = {'schema_version': 1, 'core': 'ares-md-web', 'revision': PIN, 'core_version': VERSION,
        'libretro_header_revision': HEADER_PIN, 'libretro_header_sha256': file_hash(header_path),
        'upstream_source_sha256': archive_hash, 'source_selection': list(SOURCE_PATHS),
        'upstream_input_tree_retained': False, 'adaptations': changes, 'compile_sources': list(COMPILE_SOURCES),
        'prepared_source_sha256': source_inventory(source),
        'adapter_sha256': {name: file_hash(ROOT / path) for name, path in ADAPTER_INPUTS.items()},
        'recipe_sha256': {name: file_hash(work / name) for name in RECIPES},
        'frontend_source_sha256': file_hash(source / 'omniplay-frontend.c'),
        'web_runtime_source_sha256': source_inventory(work / 'omniplay-web-runtime'),
        'exports_sha256': file_hash(work / 'exports.json'), 'runtime_methods_sha256': file_hash(work / 'runtime-methods.json'),
        'core_options_policy_sha256': file_hash(work / 'core-options-policy.json'),
        'source_only': True, 'verification': 'not_run', 'distribution_approved': False,
        'remaining': ['exact SDK/compiler/dependency inventory', 'fiber/exception/result ABI qualification',
                      'state semantic and recovery qualification', 'actual mapper/EEPROM/SVP/controller compatibility',
                      'device memory/audio/input/video/performance', 'security/license/release admission']}
    retire_inputs(work / 'inputs', work)
    header_path.unlink()
    (work / 'corresponding-source.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    return source, provenance


def build(args, work, source, provenance):
    policy = validate_policy(json.loads((work / 'core-options-policy.json').read_text(encoding='utf-8')))
    common = ['-O3', '-fexceptions', '-fno-strict-aliasing', '-sENVIRONMENT=worker',
              '-sSTACK_SIZE=2097152', '-sINITIAL_MEMORY=67108864', '-sALLOW_MEMORY_GROWTH=1',
              '-sMAXIMUM_MEMORY=1073741824', '-sALLOW_TABLE_GROWTH=1', '-sFILESYSTEM=1',
              '-sASYNCIFY=1', '-sASYNCIFY_IGNORE_INDIRECT=0', '-sSTACK_OVERFLOW_CHECK=2', '-sASSERTIONS=1']
    includes = [flag for directory in ('adapter', 'ares', 'nall', '.', 'thirdparty') for flag in ('-I', str(source / directory))]
    objects = work / 'objects'
    objects.mkdir(exist_ok=False)
    compiled = []
    for index, name in enumerate(COMPILE_SOURCES):
        obj = objects / (str(index) + '.o')
        compiler = args.emxx if name.endswith('.cpp') else args.emcc
        standard = '-std=c++20' if name.endswith('.cpp') else '-std=gnu11'
        subprocess.run([str(compiler), standard, *common, *includes, *('-D'+value for value in DEFINES),
                        '-c', str(source / name), '-o', str(obj)], check=True)
        compiled.append(str(obj))
    output = owned(ROOT / 'apps/game-hub/cores/ares-md')
    if output.exists():
        raise ValueError('Preserve previous core artifacts before making a new MD Web candidate')
    output.mkdir(parents=True)
    subprocess.run([str(args.emxx), *common, *compiled, '--no-entry', '-sMODULARIZE=1', '-sEXPORT_ES6=1',
                    '-sEXPORTED_FUNCTIONS=@' + str(work / 'exports.json'),
                    '-sEXPORTED_RUNTIME_METHODS=@' + str(work / 'runtime-methods.json'),
                    '-o', str(output / 'ares-md.js')], check=True)
    wasm = output / 'ares-md.wasm'
    manifest = {'schema_version': 1, 'id': 'ares-md-web', 'core_version': VERSION, 'runtime_version_string': VERSION,
        'source_revision': PIN, 'systems': ['mega_drive'], 'module': 'cores/ares-md/ares-md.js',
        'wasm': 'cores/ares-md/ares-md.wasm', 'wasm_bytes': wasm.stat().st_size, 'wasm_sha256': file_hash(wasm),
        'license_expression': 'ISC AND MIT', 'state_format_version': 'ares-v146-omweb1-sync',
        'in_game_format_version': FORMAT, 'portable_in_game_format': FORMAT, 'portable_files_protocol':1, 'minimum_memory_mb': 512,
        'capabilities': ['pause', 'gamepad', 'keyboard_mouse', 'touch_controls', 'save_ram', 'save_state', 'save_files'],
        'content_formats': ['raw_rom'], 'media_filesystem_protocol': 1, 'libretro_result_protocol': 1,
        'saved_files_protocol': 1, 'saved_files_io': 'single_threaded_barrier_v1', 'wasm_threads': False,
        'core_options_protocol': 1, 'core_option_definitions': policy['definitions'],
        'core_options_policy_sha256': provenance['core_options_policy_sha256'],
        'frontend_source_sha256': provenance['frontend_source_sha256'], 'adapter_sha256': provenance['adapter_sha256'],
        'firmware_protocol': 1, 'firmware': [{'role': 'mega_drive_svp', 'filename': 'svp.rom', 'allowed_bytes': [2048], 'optional': True}],
        'distribution_approved': False, 'verification': 'not_run'}
    admit_portable_policy(manifest)
    shutil.copy2(source / 'LICENSE', output / 'LICENSE.ares.txt')
    shutil.copy2(source / 'libco/LICENSE.wasm-fibers', output / 'LICENSE.omniplay-fibers.txt')
    header = (source / 'adapter/libretro.h').read_text(encoding='utf-8')
    license_end = header.find('*/')
    if not 0 < license_end < 16384:
        raise ValueError('The retained libretro interface notice boundary changed')
    (output / 'LICENSE.libretro-interface.txt').write_text(header[:license_end+2] + '\n', encoding='utf-8')
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    (work / 'build-receipt.json').write_text(json.dumps(dict(provenance, source_only=False,
        module_sha256=file_hash(output / 'ares-md.js'), wasm_sha256=file_hash(wasm)), indent=2) + '\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare-only', action='store_true')
    mode.add_argument('--build-prepared', action='store_true')
    parser.add_argument('--git', default='git')
    parser.add_argument('--emcc', type=Path)
    parser.add_argument('--emxx', type=Path)
    args = parser.parse_args()
    work = owned(ROOT / 'out/web-cores/ares-md')
    if args.prepare_only:
        prepare(work, args.git)
        print('Authored pinned MD Web corresponding source. No compiler, core or test was started.')
        return
    if not args.emcc or not args.emxx or not args.emcc.is_file() or not args.emxx.is_file():
        raise ValueError('A prepared-source MD build needs explicit Emscripten C/C++ compiler paths')
    source = work / 'source'
    provenance = json.loads((work / 'corresponding-source.json').read_text(encoding='utf-8'))
    if provenance.get('revision') != PIN or provenance.get('source_only') is not True or provenance.get('core_version') != VERSION or provenance.get('compile_sources') != list(COMPILE_SOURCES) or source_inventory(source) != provenance.get('prepared_source_sha256'):
        raise ValueError('Prepared MD Web source differs from its retained assembly')
    for name, field in [('exports.json', 'exports_sha256'), ('runtime-methods.json', 'runtime_methods_sha256'), ('core-options-policy.json', 'core_options_policy_sha256')]:
        if file_hash(work / name) != provenance.get(field):
            raise ValueError('Retained MD frontend or policy differs from its source receipt')
    if any(file_hash(work / name) != provenance.get('recipe_sha256', {}).get(name) or file_hash(ROOT / 'tools' / name) != provenance.get('recipe_sha256', {}).get(name) for name in RECIPES):
        raise ValueError('The MD build recipe differs from its retained source assembly')
    if source_inventory(work / 'omniplay-web-runtime') != provenance.get('web_runtime_source_sha256') or source_inventory(ROOT / 'engines/web') != provenance.get('web_runtime_source_sha256'):
        raise ValueError('The shared runtime differs from the retained MD source assembly')
    build(args, work, source, provenance)
    print('Built an unapproved MD Web candidate. No browser, game, compatibility or test was run.')


if __name__ == '__main__':
    main()
