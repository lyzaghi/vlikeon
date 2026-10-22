"""Prepare or explicitly build the pinned single-Worker Nintendo DS candidate.

Preparation materializes committed source, applies bounded adapters, and writes
corresponding-source inventories. It never configures, compiles or runs a core.
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
from pinned_core_source import ROOT, file_hash, materialize, owned, source_inventory
from web_core_options_policy import validate_policy

PIN = 'f394adbacb5722ee97c1b37c8064da9a25818310'
VERSION = '1.4.0+omniplay.web1'
ADAPTERS = ROOT / 'engines/web/adapters'
RECIPES = ('build_melonds_web.py', 'pinned_core_source.py', 'web_core_options_policy.py')
ADAPTED = ('melonds-info.hpp', 'melonds-info.cpp', 'melonds-threads.hpp', 'melonds-semaphore.cpp', 'libretro-results.c')
FIXED = {'melonds_console_mode': 'ds', 'melonds_boot_mode': 'direct',
    'melonds_sysfile_mode': 'native', 'melonds_firmware_nds_path': 'firmware.bin',
    'melonds_touch_mode': 'touch', 'melonds_number_of_screen_layouts': '1',
    'melonds_jit_enable': 'disabled', 'melonds_threaded_renderer': 'disabled',
    'melonds_render_mode': 'software', 'melonds_mic_input': 'silence',
    'melonds_network_mode': 'disabled', 'melonds_homebrew_sdcard': 'disabled',
    'melonds_dsi_sdcard': 'disabled', 'melonds_slot2_device': 'auto'}
FLAGS = ['-O3', '-fexceptions', '-sENVIRONMENT=worker', '-sSTACK_SIZE=4194304',
    '-sINITIAL_MEMORY=134217728', '-sALLOW_MEMORY_GROWTH=1', '-sMAXIMUM_MEMORY=2147483648',
    '-sALLOW_TABLE_GROWTH=1', '-sFILESYSTEM=1', '-sSTACK_OVERFLOW_CHECK=2', '-sASSERTIONS=1']


def replace(path, before, after):
    value = path.read_text(encoding='utf-8')
    if value.count(before) != 1:
        raise ValueError('Pinned NDS Web adaptation anchor changed: ' + path.name)
    path.write_text(value.replace(before, after, 1), encoding='utf-8', newline='\n')


def function(path, signature, body):
    """Replace exactly one pinned function, retaining surrounding source notices."""
    value = path.read_text(encoding='utf-8')
    if value.count(signature) != 1:
        raise ValueError('Pinned function adaptation anchor changed: ' + path.name)
    start = value.index(signature)
    opening = value.index('{', start)
    # These selected upstream functions have balanced brace-only lexical bodies;
    # the caller supplies the complete exact replacement and its retained source.
    depth = 1
    end = opening + 1
    while depth and end < len(value):
        depth += (value[end] == '{') - (value[end] == '}')
        end += 1
    if depth:
        raise ValueError('Pinned function body is incomplete')
    path.write_text(value[:opening] + '{\n' + body + '\n}' + value[end:], encoding='utf-8', newline='\n')


def bound_states(melon):
    state = melon / 'src/Savestate.cpp'
    replace(state, '    if (Saving)\n    {\n        WriteSavestateHeader();',
        '    if (!buffer || size < 16 || size > 64u * 1024u * 1024u) { Error = true; return; }\n'
        '    if (Saving)\n    {\n        WriteSavestateHeader();')
    replace(state, '    buffer = static_cast<u8 *>(malloc(buffer_length));',
        '    if (buffer_length < 16 || buffer_length > 64u * 1024u * 1024u) { Error = true; return; }\n'
        '    buffer = static_cast<u8 *>(malloc(buffer_length));')
    replace(state, '    assert(buffer_offset <= buffer_length);',
        '    if (buffer_offset > buffer_length || (!data && len)) { Error = true; return; }')
    value = state.read_text(encoding='utf-8')
    if value.count('buffer_offset + len > buffer_length') != 2:
        raise ValueError('Pinned savestate primitive bounds changed')
    state.write_text(value.replace('buffer_offset + len > buffer_length', 'len > buffer_length - buffer_offset'), encoding='utf-8', newline='\n')
    replace(state, '#include <stdio.h>', '#include <stdio.h>\n#include <algorithm>\n#include <cstdint>')
    replace(state, '            if (!(buffer_owned && Resize(buffer_length * 2 + len)))',
        '            if (len > 64u * 1024u * 1024u - buffer_offset ||\n'
        '                !(buffer_owned && Resize(static_cast<u32>(std::min<uint64_t>(64u * 1024u * 1024u,\n'
        '                    static_cast<uint64_t>(buffer_length) * 2u + len)))))')
    replace(state, '    if (!buffer_owned)\n    {',
        '    if (new_length < buffer_length || new_length > 64u * 1024u * 1024u) return false;\n'
        '    if (!buffer_owned)\n    {')
    replace(state, '        u8 val;\n        Var8(&val);', '        u8 val = 0;\n        Var8(&val);')
    replace(state, '        u32 val;\n        Var32(&val);', '        u32 val = 0;\n        Var32(&val);')
    function(state, 'u32 Savestate::FindSection(const char* magic) const', '''    if (!magic || !buffer || buffer_length < 16 || buffer_length > 64u * 1024u * 1024u) return NO_SECTION;
    u32 found = NO_SECTION;
    u32 offset = 16;
    unsigned count = 0;
    while (offset < buffer_length) {
        if (++count > 4096 || buffer_length - offset < 16) return NO_SECTION;
        u32 length = 0;
        memcpy(&length, buffer + offset + 4, sizeof(length));
        if (length < 16 || length > buffer_length - offset) return NO_SECTION;
        if (memcmp(buffer + offset, magic, 4) == 0) {
            if (found != NO_SECTION) return NO_SECTION;
            found = offset + 16;
        }
        offset += length;
    }
    return offset == buffer_length ? found : NO_SECTION;''')
    replace(state, '        u32 section_length = buffer_offset - CurSection;',
        '        if (CurSection > buffer_length || buffer_length - CurSection < 16 ||\n'
        '            buffer_offset < CurSection + 16 || buffer_offset > buffer_length) { Error = true; return; }\n'
        '        u32 section_length = buffer_offset - CurSection;')
    header = melon / 'src/Savestate.h'
    replace(header, '        memcpy(&major, buffer + 0x04, sizeof(major));',
        '        if (buffer && buffer_length >= 6) memcpy(&major, buffer + 0x04, sizeof(major));')
    replace(header, '        memcpy(&minor, buffer + 0x06, sizeof(minor));',
        '        if (buffer && buffer_length >= 8) memcpy(&minor, buffer + 0x06, sizeof(minor));')


def prepare(work, git):
    lock_path = ROOT / 'security/melonds-source.lock.json'
    lock = json.loads(lock_path.read_text(encoding='utf-8'))
    if lock.get('schema_version') != 1 or lock.get('core', {}).get('revision') != PIN or len(lock.get('dependencies', [])) != 10:
        raise ValueError('The NDS Web source lock is incomplete')
    source, archive = materialize('melonds-ds', PIN, work, git)
    archives = {'melonds-ds': file_hash(archive)}
    archive.unlink()
    local = {}
    for entry in lock['dependencies']:
        name, revision = entry['name'], entry['revision']
        if not re.fullmatch('[A-Za-z0-9_-]{1,64}', name) or not re.fullmatch('[a-f0-9]{40}', revision) or name.lower() in {n.lower() for n in local}:
            raise ValueError('The NDS dependency identity is invalid or aliased')
        dependency, archived = materialize('melonds-deps/' + name, revision, work / 'dependencies' / name, git)
        archives[name] = file_hash(archived)
        archived.unlink()
        local[name] = dependency
    fetch = source / 'cmake/FetchDependencies.cmake'
    replace(fetch, 'fetch_dependency(libslirp "https://github.com/JesseTG/libslirp-mirror" "e61dbd4")', '')
    replace(fetch, 'set(CMAKE_MODULE_PATH "${FETCHCONTENT_BASE_DIR}/melonds-src/cmake" "${FETCHCONTENT_BASE_DIR}/embed-binaries-src/cmake" "${CMAKE_MODULE_PATH}")',
        'set(CMAKE_MODULE_PATH "${FETCHCONTENT_SOURCE_DIR_MELONDS}/cmake" "${FETCHCONTENT_SOURCE_DIR_EMBED-BINARIES}/cmake" "${CMAKE_MODULE_PATH}")')
    replace(fetch, 'FetchContent_MakeAvailable(melonDS libretro-common embed-binaries glm zlib libslirp pntr fmt yamc span-lite date)',
        'FetchContent_MakeAvailable(melonDS libretro-common embed-binaries glm zlib pntr fmt yamc span-lite date)')
    replace(source / 'cmake/ConfigureDependencies.cmake', 'include(cmake/libslirp.cmake)', '')
    replace(source / 'cmake/GenerateAttributions.cmake', 'file(READ "${libslirp_SOURCE_DIR}/COPYRIGHT" SLIRP_LICENSE)', '')
    replace(source / 'cmake/melondsds-LICENSE.txt.in', '# Slirp ################################################################################################################\n\n${SLIRP_LICENSE}\n\n', '')
    replace(source / 'cmake/ConfigureFeatures.cmake',
        'find_package(Threads REQUIRED)\n\nif (Threads_FOUND)\n    set(HAVE_THREADS ON)\nendif ()',
        'set(HAVE_THREADS OFF) # One synchronous browser Worker; no pthread producers.')
    libretro = source / 'src/libretro'
    for name, target in [('melonds-info.hpp', 'retro/info.hpp'), ('melonds-info.cpp', 'retro/info.cpp'),
                         ('melonds-threads.hpp', 'retro/threads.hpp'), ('melonds-semaphore.cpp', 'platform/semaphore.cpp')]:
        shutil.copy2(ADAPTERS / name, libretro / target)
    (libretro / 'retro/threads.cpp').write_text('#include "threads.hpp"\n', encoding='utf-8')
    environment = libretro / 'environment.cpp'
    fixed = '\n'.join('    if (key == ' + json.dumps(key) + ') return ' + json.dumps(value) + ';' for key, value in sorted(FIXED.items()))
    replace(environment, 'string_view retro::get_variable(std::string_view key) noexcept {',
        'string_view retro::get_variable(std::string_view key) noexcept {\n' + fixed)
    retro = libretro / 'libretro.cpp'
    replace(retro, '    info->library_version = MELONDSDS_VERSION;', '    info->library_version = "' + VERSION + '";')
    replace(retro, '    info->need_fullpath = false;', '    info->need_fullpath = true;')
    replace(retro, '    info->valid_extensions = "nds|ids|dsi";', '    info->valid_extensions = "nds";')
    replace(retro, 'PUBLIC_SYMBOL bool retro_load_game(const struct retro_game_info *info) {',
        'PUBLIC_SYMBOL bool retro_load_game(const struct retro_game_info *info) {\n    if (!info || !info->path) return false;')
    core = libretro / 'core/core.cpp'
    replace(core, '                if (game[0].size == 0) {', '                if (false) { // Size is checked by the bounded full-path reader.')
    replace(core, '                if (game[0].data == nullptr) {', '                if (!game[0].path || game[0].data || game[0].size) {')
    replace(core, '    InitContent(type, game);',
        '    if (type != MELONDSDS_GAME_TYPE_NDS || game.size() != 1)\n'
        '        throw content_exception("This adapter requires one DS cartridge");\n    InitContent(type, game);')
    function(core, 'void MelonDsDs::CoreState::InitFlushFirmwareTask() noexcept',
        '    // Selected system files are immutable. Session firmware changes remain\n'
        '    // in memory; cartridge SRAM is captured through RETRO_MEMORY_SAVE_RAM.\n    _flushTaskId = std::nullopt;')
    console = libretro / 'config/console.cpp'
    replace(console, '#include <string_view>', '#include <string_view>\n#include <stdexcept>')
    replace(console, 'retro::warn("Falling back to built-in firmware");', 'throw std::runtime_error("Selected DS firmware could not be loaded");')
    replace(console, 'retro::warn("Falling back to FreeBIOS");', 'throw std::runtime_error("Selected DS BIOS files could not be loaded");')
    replace(console, '    span<const std::byte> rom = ndsInfo.GetData();',
        '    ndsInfo.PrepareNdsData();\n    span<const std::byte> rom = ndsInfo.GetData();')
    replace(console, 'cart = melonDS::NDSCart::ParseROM(reinterpret_cast<const uint8_t*>(rom.data()), rom.size(), nullptr, std::move(sdargs));',
        'cart = melonDS::NDSCart::ParseROM(ndsInfo.TakeNdsData(), rom.size(), nullptr, std::move(sdargs));')
    replace(console, '    const NDSHeader &header = *reinterpret_cast<const NDSHeader*>(rom.data());',
        '    const NDSHeader &header = *reinterpret_cast<const NDSHeader*>(rom.data());\n'
        '    if (header.ARM9ROMOffset < 0x200 || header.ARM9ROMOffset > rom.size() - 0x14 ||\n'
        '        header.ARM9Size > rom.size() - header.ARM9ROMOffset ||\n'
        '        header.ARM7ROMOffset < 0x200 || header.ARM7ROMOffset > rom.size() ||\n'
        '        header.ARM7Size > rom.size() - header.ARM7ROMOffset)\n'
        '        throw invalid_rom_exception("DS cartridge regions exceed the input file");')
    sram = libretro / 'sram.cpp'
    replace(sram, '    _sram(std::make_unique<u8[]>(initialLength)),',
        '    _sram(initialLength <= 32u * 1024u * 1024u ? std::make_unique<u8[]>(initialLength) : throw std::bad_alloc()),')
    function(sram, 'void MelonDsDs::sram::SaveManager::Flush(const u8 *savedata, u32 savelen, u32 writeoffset, u32 writelen)', '''    if (savelen > 32u * 1024u * 1024u || (!savedata && savelen)) std::abort();
    if (_sram_length != savelen) {
        auto replacement = std::make_unique<u8[]>(savelen);
        if (savelen) memcpy(replacement.get(), savedata, savelen);
        _sram = std::move(replacement);
        _sram_length = savelen;
    } else if (savelen) {
        if (writeoffset >= savelen) std::abort();
        const u32 first = std::min(writelen, savelen - writeoffset);
        memcpy(_sram.get() + writeoffset, savedata + writeoffset, first);
        const u32 wrapped = std::min(writelen - first, savelen);
        if (wrapped) memcpy(_sram.get(), savedata, wrapped);
    } else if (writeoffset || writelen) std::abort();''')
    replace(sram, '#include <cstring>', '#include <cstring>\n#include <algorithm>\n#include <cstdlib>')
    bound_states(local['melonDS'])
    shutil.copy2(ROOT / 'engines/web/frontend.c', libretro / 'omniplay-frontend.c')
    shutil.copy2(ADAPTERS / 'libretro-results.c', libretro / 'omniplay-results.c')
    cmake = libretro / 'CMakeLists.txt'
    replace(cmake, 'add_library(melondsds_libretro ${LIBRARY_TYPE}',
        'add_executable(melondsds_libretro\n    omniplay-frontend.c\n    omniplay-results.c')
    with cmake.open('a', encoding='utf-8', newline='\n') as stream:
        stream.write('''
if(NOT EMSCRIPTEN)
    message(FATAL_ERROR "The adapted NDS Web source requires Emscripten")
endif()
set_target_properties(melondsds_libretro PROPERTIES OUTPUT_NAME "melonds-ds" SUFFIX ".js"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/candidate")
target_link_options(melondsds_libretro PRIVATE --no-entry -sMODULARIZE=1 -sEXPORT_ES6=1
    "-sEXPORTED_FUNCTIONS=@${CMAKE_SOURCE_DIR}/../exports.json"
    "-sEXPORTED_RUNTIME_METHODS=@${CMAKE_SOURCE_DIR}/../runtime-methods.json")
''')
    exports = json.loads((ROOT / 'engines/web/exports.json').read_text(encoding='utf-8')) + ['_om_web_libretro_call']
    (work / 'exports.json').write_text(json.dumps(exports, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(ROOT / 'engines/web/runtime-methods.json', work / 'runtime-methods.json')
    policy = ROOT / 'engines/policies/melonds-ds-options.json'
    validate_policy(json.loads(policy.read_text(encoding='utf-8')))
    shutil.copy2(policy, work / 'core-options-policy.json')
    shutil.copy2(lock_path, work / 'source-lock.json')
    shutil.copytree(ROOT / 'engines/web', work / 'omniplay-web-runtime')
    for name in RECIPES:
        shutil.copy2(ROOT / 'tools' / name, work / name)
    changes = ['committed_offline_dependency_trees', 'no_jit_gpu_pthreads_or_network_backend',
        'synchronous_renderer_and_cooperative_task_queue', 'owned_full_path_512_mib_cartridge',
        'retained_header_after_rom_ownership_transfer', 'fixed_ds_direct_boot_and_no_microphone_capture',
        'mandatory_user_bios_and_firmware_no_silent_fallback', 'immutable_system_firmware',
        'subtractive_cartridge_region_checks', 'bounded_sram_updates_and_snapshot_sections',
        'void_call_result_protocol', 'post_load_four_mib_memory_admission']
    (source / 'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version': 1, 'revision': PIN, 'changes': changes}, indent=2) + '\n', encoding='utf-8')
    provenance = {'schema_version': 1, 'core': 'melonds-ds-web', 'revision': PIN,
        'version': VERSION, 'source_only': True, 'verification': 'not_run', 'distribution_approved': False,
        'source_lock': lock, 'source_archives_sha256': archives, 'fixed_options': FIXED,
        'adaptations': changes, 'prepared_source_sha256': source_inventory(source),
        'dependency_source_sha256': {name: source_inventory(path) for name, path in local.items()},
        'adapter_sha256': {name: file_hash(ADAPTERS / name) for name in ADAPTED},
        'web_runtime_source_sha256': source_inventory(work / 'omniplay-web-runtime'),
        'recipe_sha256': {name: file_hash(work / name) for name in RECIPES},
        'auxiliary_sha256': {name: file_hash(work / name) for name in ('exports.json', 'runtime-methods.json', 'core-options-policy.json', 'source-lock.json')},
        'remaining': ['exact Emscripten SDK/toolchain and compiled dependencies',
            'untrusted ROM/state fault and shutdown qualification', 'homebrew storage and DSi/slot-2/peripheral adapters',
            'portable native/Web progress profile', 'actual memory, timing, compatibility and touch geometry',
            'complete artifact notices/content/license/store/security/release approval']}
    (work / 'corresponding-source.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    return source, local, provenance


def build(args, work):
    provenance = json.loads((work / 'corresponding-source.json').read_text(encoding='utf-8'))
    source = work / 'source'
    if provenance.get('revision') != PIN or provenance.get('source_only') is not True or provenance.get('version') != VERSION or provenance.get('fixed_options') != FIXED or source_inventory(source) != provenance.get('prepared_source_sha256'):
        raise ValueError('Prepared NDS source differs from its retained source assembly')
    local = {entry['name']: work / 'dependencies' / entry['name'] / 'source' for entry in provenance['source_lock']['dependencies']}
    if any(source_inventory(path) != provenance.get('dependency_source_sha256', {}).get(name) for name, path in local.items()):
        raise ValueError('Prepared NDS dependency source differs from its source assembly')
    if any(file_hash(work / name) != value for name, value in provenance['auxiliary_sha256'].items()) or any(file_hash(work / name) != provenance.get('recipe_sha256', {}).get(name) or file_hash(ROOT / 'tools' / name) != provenance.get('recipe_sha256', {}).get(name) for name in RECIPES):
        raise ValueError('Prepared NDS auxiliary inputs or build recipe changed')
    if source_inventory(ROOT / 'engines/web') != provenance.get('web_runtime_source_sha256') or source_inventory(work / 'omniplay-web-runtime') != provenance.get('web_runtime_source_sha256'):
        raise ValueError('The shared Web runtime changed after NDS source preparation')
    build_dir = work / 'build'
    if build_dir.exists():
        raise ValueError('An explicit NDS build requires a fresh build directory')
    flags = ' '.join(FLAGS)
    command = [str(args.cmake), '-S', str(source), '-B', str(build_dir), '-G', args.generator,
        '-DCMAKE_TOOLCHAIN_FILE=' + str(args.toolchain_file), '-DCMAKE_C_COMPILER=' + str(args.emcc),
        '-DCMAKE_CXX_COMPILER=' + str(args.emxx), '-DCMAKE_BUILD_TYPE=Release',
        '-DCMAKE_C_FLAGS=' + flags, '-DCMAKE_CXX_FLAGS=' + flags, '-DCMAKE_EXE_LINKER_FLAGS=' + flags,
        '-DFETCHCONTENT_FULLY_DISCONNECTED=ON', '-DFETCHCONTENT_UPDATES_DISCONNECTED=ON',
        '-DENABLE_JIT=OFF', '-DENABLE_OPENGL=OFF', '-DENABLE_COMPUTE_RENDERER=OFF',
        '-DENABLE_OGLRENDERER=OFF', '-DENABLE_NETWORKING=OFF', '-DENABLE_DYNAMIC=OFF',
        '-DENABLE_GDBSTUB=OFF', '-DENABLE_THREADED_RENDERER=OFF', '-DENABLE_LTO=OFF',
        '-DENABLE_LTO_RELEASE=OFF', '-DBUILD_TESTING=OFF', '-DENABLE_TESTING=OFF',
        '-DTRACY_ENABLE=OFF', '-DBUILD_QT_SDL=OFF']
    for entry in provenance['source_lock']['dependencies']:
        name = entry['name']
        command += ['-DFETCHCONTENT_SOURCE_DIR_' + name.upper() + '=' + str(local[name]),
            '-D' + re.sub('[^A-Z0-9_]', '_', name.upper()) + '_REPOSITORY_TAG=' + entry['revision']]
    subprocess.run(command, check=True)
    subprocess.run([str(args.cmake), '--build', str(build_dir), '--target', 'melondsds_libretro', '--parallel', str(args.parallel)], check=True)
    candidate = build_dir / 'candidate'
    module, wasm = candidate / 'melonds-ds.js', candidate / 'melonds-ds.wasm'
    if not module.is_file() or not wasm.is_file():
        raise ValueError('The NDS build did not produce both executable artifacts')
    output = owned(ROOT / 'apps/game-hub/cores/melonds-ds')
    if output.exists():
        raise ValueError('Preserve prior NDS artifacts before preparing a candidate package')
    output.mkdir(parents=True)
    for path in (module, wasm):
        shutil.copy2(path, output / path.name)
    policy = validate_policy(json.loads((work / 'core-options-policy.json').read_text(encoding='utf-8')))
    manifest = {'schema_version': 1, 'id': 'melonds-ds-web', 'core_version': VERSION,
        'runtime_version_string': VERSION, 'source_revision': PIN, 'systems': ['nds'],
        'module': 'cores/melonds-ds/melonds-ds.js', 'wasm': 'cores/melonds-ds/melonds-ds.wasm',
        'wasm_bytes': wasm.stat().st_size, 'wasm_sha256': file_hash(wasm),
        'license_expression': 'GPL-3.0-or-later', 'state_format_version': 'melonds-ds-web1-ds-state',
        'in_game_format_version': 'melonds-ds-web1-nds-sram', 'minimum_memory_mb': 1024,
        'minimum_system_ram_bytes': 4194304, 'capabilities': ['pause', 'gamepad', 'keyboard_mouse', 'touch_controls', 'save_ram', 'save_state'],
        'content_formats': ['raw_rom'], 'media_filesystem_protocol': 1, 'file_backed_content_required': True, 'wasm_threads': False,
        'libretro_result_protocol': 1, 'core_options_protocol': 1, 'core_option_definitions': policy['definitions'],
        'core_options_policy_sha256': provenance['auxiliary_sha256']['core-options-policy.json'],
        'adapter_sha256': provenance['adapter_sha256'], 'firmware_protocol': 1,
        'firmware': [{'role': role, 'filename': filename, 'allowed_bytes': sizes} for role, filename, sizes in
            [('nds_bios7', 'bios7.bin', [16384]), ('nds_bios9', 'bios9.bin', [4096]), ('nds_firmware', 'firmware.bin', [131072, 262144, 524288])]],
        'distribution_approved': False, 'verification': 'not_run'}
    notices = list(build_dir.glob('melondsds-LICENSE.txt'))
    if len(notices) != 1:
        raise ValueError('The generated NDS dependency notice is missing')
    shutil.copy2(notices[0], output / 'LICENSE.melonds-ds.txt')
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    (work / 'build-receipt.json').write_text(json.dumps(dict(provenance, source_only=False,
        module_sha256=file_hash(module), wasm_sha256=file_hash(wasm)), indent=2) + '\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare-only', action='store_true')
    mode.add_argument('--build-prepared', action='store_true')
    parser.add_argument('--git', default='git')
    for name in ('cmake', 'toolchain-file', 'emcc', 'emxx'):
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--generator', default='Ninja')
    parser.add_argument('--parallel', type=int, default=8)
    args = parser.parse_args()
    if not 1 <= args.parallel <= 32:
        raise ValueError('Build parallelism exceeds its bound')
    work = owned(ROOT / 'out/web-cores/melonds-ds')
    if args.prepare_only:
        prepare(work, args.git)
        print('Authored pinned NDS Web source and dependency inventories. No configure, compiler, core or test was started.')
        return
    if any(path is None or not path.is_file() for path in (args.cmake, args.toolchain_file, args.emcc, args.emxx)):
        raise ValueError('An explicit prepared-source build requires actual CMake, Emscripten toolchain, C and C++ compiler paths')
    build(args, work)
    print('Built an unapproved NDS Web candidate. No browser, game, device or test was run.')


if __name__ == '__main__':
    main()
