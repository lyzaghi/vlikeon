"""Prepare or explicitly compile the pinned N64 WebGL2/interpreter candidate.

Source preparation never invokes make, a compiler, emulator, browser or test.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from pinned_core_source import ROOT, file_hash, materialize, owned, source_inventory
from web_core_options_policy import validate_policy

PIN = '12edd2c74a517ff86dfa8cfc71ad75e4c10486d5'
VERSION = '3.0.0+omniplay.webgl1.12edd2c74a51'
SELECTED = ('LICENSE', 'Makefile', 'Makefile.common', 'README.md', 'GLideN64', 'custom',
    'libretro-common', 'libretro', 'mupen64plus-core', 'mupen64plus-rsp-hle', 'xxHash')
RECIPES = ('build_mupen64plus_web.py', 'pinned_core_source.py', 'web_core_options_policy.py')
ADAPTERS = ROOT / 'engines/web/adapters'
FIXED = {'mupen64plus-rdp-plugin': 'gliden64', 'mupen64plus-rsp-plugin': 'hle',
    'mupen64plus-cpucore': 'cached_interpreter', 'mupen64plus-ThreadedRenderer': 'False',
    'mupen64plus-43screensize': '640x480', 'mupen64plus-aspect': '4:3',
    'mupen64plus-EnableNativeResFactor': '0', 'mupen64plus-MultiSampling': '0',
    'mupen64plus-FXAA': 'False', 'mupen64plus-txHiresEnable': 'False',
    'mupen64plus-EnableShadersStorage': 'False',
    'mupen64plus-MaxTxCacheSize': '1500', 'mupen64plus-FrameDuping': 'False',
    **{'mupen64plus-pak' + str(port): 'memory' for port in range(1, 5)}}
FLAGS = ['-O3', '-fexceptions', '-fno-fast-math', '-fno-unsafe-math-optimizations', '-ffp-contract=off',
    '-sENVIRONMENT=worker', '-sSTACK_SIZE=2097152', '-sINITIAL_MEMORY=134217728',
    '-sALLOW_MEMORY_GROWTH=1', '-sMAXIMUM_MEMORY=1073741824', '-sALLOW_TABLE_GROWTH=1',
    '-sFILESYSTEM=1', '-sASYNCIFY=1', '-sASYNCIFY_IGNORE_INDIRECT=0',
    '-sMIN_WEBGL_VERSION=2', '-sMAX_WEBGL_VERSION=2', '-sOFFSCREENCANVAS_SUPPORT=1',
    '-sOFFSCREEN_FRAMEBUFFER=1', '-sGL_ENABLE_GET_PROC_ADDRESS=1', '-sSTACK_OVERFLOW_CHECK=2', '-sASSERTIONS=1']


def replace(path, before, after, count=1):
    value = path.read_text(encoding='utf-8')
    if value.count(before) != count:
        raise ValueError('Pinned N64 Web adaptation anchor changed: ' + path.name)
    path.write_text(value.replace(before, after), encoding='utf-8', newline='\n')


def prepare(work, git):
    source, archive = materialize('mupen64plus-next', PIN, work, git, SELECTED)
    archive_hash = file_hash(archive)
    archive.unlink()
    libretro = source / 'libretro/libretro.c'
    replace(libretro, 'info->library_version = "3.0" FLAVOUR_VERSION GIT_VERSION;',
        'info->library_version = "' + VERSION + '";')
    replace(libretro, 'CoreStartup(FRONTEND_API_VERSION, ".", ".", NULL, n64DebugCallback, 0, n64StateCallback)',
        'CoreStartup(FRONTEND_API_VERSION, "/scratch", "/scratch", NULL, n64DebugCallback, 0, n64StateCallback)')
    replace(libretro, 'copy_file(inifile, "mupen64plus.ini");', 'copy_file(inifile, "/scratch/mupen64plus.ini");')
    replace(libretro, '    if(ret && log_cb)', '    if (ret) environ_cb(RETRO_ENVIRONMENT_SHUTDOWN, NULL);\n    if(ret && log_cb)')
    bindings = '\n    plugin_connect_rdp_api(RDP_PLUGIN_GLIDEN64);\n    plugin_connect_rsp_api(RSP_PLUGIN_HLE);\n    EnableThreadedRenderer = 0;'
    replace(libretro, '    environ_cb(RETRO_ENVIRONMENT_GET_RUMBLE_INTERFACE, &rumble);',
        '    environ_cb(RETRO_ENVIRONMENT_GET_RUMBLE_INTERFACE, &rumble);' + bindings)
    replace(libretro, '    update_variables(true);', '    update_variables(true);' + bindings)
    replace(libretro, 'bool retro_load_game(const struct retro_game_info *game)\n{',
        'bool retro_load_game(const struct retro_game_info *game)\n{\n'
        '    if (!game || !game->data || game->size < 0x1000 || game->size > 256u * 1024u * 1024u) return false;')
    replace(libretro, '    game_data = malloc(game->size);',
        '    game_data = malloc(game->size);\n    if (!game_data) return false;')
    replace(libretro, 'static void* EmuThreadFunction(void* param)',
        'static bool om_cpu_finished = false;\nstatic void* EmuThreadFunction(void* param)')
    replace(libretro, 'void retro_init(void)\n{', 'void retro_init(void)\n{\n    om_cpu_finished = false;')
    fixed = '\n'.join('        if (!strcmp(var->key, ' + json.dumps(key) + ')) { var->value = ' + json.dumps(value) + '; return true; }' for key, value in sorted(FIXED.items()))
    replace(libretro, 'void retro_set_environment(retro_environment_t cb)\n{\n    environ_cb = cb;', '''static retro_environment_t om_frontend_environment;
static bool om_environment(unsigned command, void* data) {
    if (command == RETRO_ENVIRONMENT_GET_VARIABLE && data) {
        struct retro_variable* var = (struct retro_variable*)data;
        if (!var->key) return false;
''' + fixed + '''
    }
    return om_frontend_environment && om_frontend_environment(command, data);
}
void retro_set_environment(retro_environment_t cb)
{
    om_frontend_environment = cb;
    environ_cb = om_environment;''')
    # The adapter's coroutine returns to its owner on normal CPU shutdown.
    # It must not return through a foreign stack or delete its active fiber.
    value = libretro.read_text(encoding='utf-8')
    start = value.index('    CoreDoCommand(M64CMD_EXECUTE, 0, NULL);')
    end = value.index('static void reinit_gfx_plugin(void)', start)
    segment = value[start:end]
    if segment.count('    return NULL;') != 1:
        raise ValueError('The N64 CPU coroutine shutdown anchor changed')
    value = value[:start] + segment.replace('    return NULL;', '    om_cpu_finished = true;\n    co_switch(retro_thread);\n    abort();\n    return NULL;') + value[end:]
    libretro.write_text(value, encoding='utf-8', newline='\n')
    replace(libretro, '    deinit_audio_libretro();',
        '    deinit_audio_libretro();\n    if (game_thread) { co_delete(game_thread); game_thread = NULL; }')
    replace(libretro, '           CoreDoCommand(M64CMD_STOP, 0, NULL);\n           co_switch(game_thread); /* Let the core thread finish */',
        '           if (!initializing && !om_cpu_finished) {\n'
        '               if (CoreDoCommand(M64CMD_STOP, 0, NULL) != M64ERR_SUCCESS) abort();\n'
        '               for (unsigned attempts = 0; !om_cpu_finished && attempts < 4; ++attempts) co_switch(game_thread);\n'
        '               if (!om_cpu_finished) abort();\n'
        '           }')
    replace(libretro, 'void retro_run (void)\n{',
        'void retro_run (void)\n{\n    if (om_cpu_finished) { environ_cb(RETRO_ENVIRONMENT_SHUTDOWN, NULL); return; }')
    replace(libretro, '    CoreDoCommand(M64CMD_RESET, 0, (void*)0);', '    if (!initializing) CoreDoCommand(M64CMD_RESET, 0, (void*)0);')
    before = '   if (initializing)\n      return false;\n\n   retro_savestate_complete = false;'
    after = '   if (!load_game_successful || !data || size != retro_serialize_size())\n      return false;\n\n   retro_savestate_complete = false;'
    replace(libretro, before, after, count=2)
    replace(libretro, 'bool retro_unserialize(const void *data, size_t size)\n{',
        'extern int om_web_n64_state_header(const void*, size_t);\n'
        'bool retro_unserialize(const void *data, size_t size)\n{\n'
        '   if (!om_web_n64_state_header(data, size)) return false;')
    value = libretro.read_text(encoding='utf-8')
    start = value.index('bool retro_unserialize(const void *data, size_t size)')
    end = value.index('//Needed to be able to detach controllers', start)
    segment = value[start:end]
    if segment.count('   return true;') != 1:
        raise ValueError('The N64 snapshot completion anchor changed')
    libretro.write_text(value[:start] + segment.replace('   return true;', '   return !!retro_savestate_result;') + value[end:], encoding='utf-8', newline='\n')
    state = source / 'mupen64plus-core/src/main/savestates.c'
    with state.open('a', encoding='utf-8', newline='\n') as stream:
        stream.write('''
/* Exact candidate header admission precedes state jobs and their locks. */
int om_web_n64_state_header(const void* input, size_t length) {
    if (!input || length != 16788288u + 1024u + 4u + 4096u) return 0;
    const unsigned char* bytes = (const unsigned char*)input;
    const unsigned version = ((unsigned)bytes[8] << 24) | ((unsigned)bytes[9] << 16) |
        ((unsigned)bytes[10] << 8) | bytes[11];
    return memcmp(bytes, savestate_magic, 8) == 0 && version == savestate_latest_version &&
        memcmp(bytes + 12, ROM_SETTINGS.MD5, 32) == 0;
}
''')
    recomp = source / 'mupen64plus-core/src/device/r4300/recomp.c'
    replace(recomp, 'MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE', 'MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE')
    replace(recomp, 'PROT_READ | PROT_WRITE | PROT_EXEC', 'PROT_READ | PROT_WRITE')
    fiber = source / 'libretro-common/libco/omniplay-fibers.c'
    shutil.copy2(ADAPTERS / 'bsnes-fibers.c', fiber)
    replace(fiber, 'OM_CO_C_MAX = 2 * 1024 * 1024', 'OM_CO_C_MAX = 8 * 1024 * 1024')
    (source / 'libretro-common/libco/libco.c').write_text('#include "omniplay-fibers.c"\n', encoding='utf-8')
    shutil.copy2(ADAPTERS / 'LICENSE.libco', source / 'libretro-common/libco/LICENSE.omniplay')
    for original, target in [(ROOT / 'engines/web/frontend.c', 'omniplay-frontend.c'),
                             (ADAPTERS / 'libretro-results.c', 'omniplay-results.c'),
                             (ADAPTERS / 'webgl2-frontend.c', 'omniplay-webgl2.c')]:
        shutil.copy2(original, source / target)
    shutil.copy2(ADAPTERS / 'LICENSE.webgl2', source / 'LICENSE.omniplay-webgl2.txt')
    # The upstream emscripten branch unconditionally asks its SDK for zlib.
    # Keep all zlib/minizip/libpng/xxHash sources local and pinned instead.
    replace(source / 'Makefile', '   CPUFLAGS += -DEMSCRIPTEN -DNO_ASM -s USE_ZLIB=1',
        '   CPUFLAGS += -DEMSCRIPTEN -DNO_ASM')
    with (source / 'Makefile').open('a', encoding='utf-8', newline='\n') as stream:
        stream.write('\n# OmniPlay: deterministic interpreter math overrides upstream fast-math.\n'
            'override CFLAGS += -fexceptions -fno-fast-math -fno-unsafe-math-optimizations -ffp-contract=off\n'
            'override CXXFLAGS += -fexceptions -fno-fast-math -fno-unsafe-math-optimizations -ffp-contract=off\n')
    exports = json.loads((ROOT / 'engines/web/exports.json').read_text(encoding='utf-8')) + ['_om_web_libretro_call', '_om_web_log_interface'] + [
        '_om_web_hw_' + name for name in ('prepare', 'register', 'call', 'alive', 'callback_bytes')]
    (work / 'exports.json').write_text(json.dumps(exports, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(ROOT / 'engines/web/runtime-methods.json', work / 'runtime-methods.json')
    policy = ROOT / 'engines/policies/mupen64plus-web-options.json'
    validate_policy(json.loads(policy.read_text(encoding='utf-8')))
    shutil.copy2(policy, work / 'core-options-policy.json')
    shutil.copytree(ROOT / 'engines/web', work / 'omniplay-web-runtime')
    for name in RECIPES:
        shutil.copy2(ROOT / 'tools' / name, work / name)
    adaptations = ['selected_committed_gliden64_hle_sources', 'angrylion_cxd4_parallel_not_materialized',
        'no_cpu_dynarec_or_os_threads', 'cached_interpreter_no_executable_instruction_heap',
        'single_worker_asyncify_fibers_with_owned_shutdown', 'webgl2_private_bounded_target',
        'void_reset_destroy_and_snapshot_completion', 'fixed_execution_resolution_and_memory_paks',
        'exact_snapshot_size_version_and_rom_header', 'truthful_load_result_and_prestart_jobs',
        'offline_dependencies_and_private_scratch', 'cpu_fast_math_disabled']
    (source / 'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version': 1, 'revision': PIN, 'version': VERSION,
        'adaptations': adaptations, 'verification': 'not_run'}, indent=2) + '\n', encoding='utf-8')
    provenance = {'schema_version': 1, 'core': 'mupen64plus-web', 'revision': PIN, 'version': VERSION,
        'selected_committed_paths': SELECTED, 'source_archive_sha256': archive_hash,
        'adaptations': adaptations, 'fixed_options': FIXED, 'source_only': True,
        'verification': 'not_run', 'distribution_approved': False,
        'prepared_source_sha256': source_inventory(source),
        'web_runtime_source_sha256': source_inventory(work / 'omniplay-web-runtime'),
        'recipe_sha256': {name: file_hash(work / name) for name in RECIPES},
        'auxiliary_sha256': {name: file_hash(work / name) for name in ('exports.json', 'runtime-methods.json', 'core-options-policy.json')},
        'remaining': ['exact Emscripten/toolchain and actual compiled inventory', 'GLideN64 GLES/WebGL shader and extension qualification',
            'coroutine/state/ROM fault and failure recovery', 'all required LLE/peripheral/companion adapters',
            'portable progress conversion', 'GPU allocation and browser context-loss qualification',
            'actual platform/device timing/memory/energy and controller behavior', 'licenses/content/SBOM/store/security/release approval']}
    (work / 'corresponding-source.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')


def build(args, work):
    provenance = json.loads((work / 'corresponding-source.json').read_text(encoding='utf-8'))
    source = work / 'source'
    if provenance.get('revision') != PIN or provenance.get('version') != VERSION or provenance.get('source_only') is not True or provenance.get('fixed_options') != FIXED or source_inventory(source) != provenance.get('prepared_source_sha256'):
        raise ValueError('The prepared N64 source differs from its assembly receipt')
    if any(file_hash(work / name) != expected for name, expected in provenance['auxiliary_sha256'].items()) or any(file_hash(work / name) != provenance.get('recipe_sha256', {}).get(name) or file_hash(ROOT / 'tools' / name) != provenance.get('recipe_sha256', {}).get(name) for name in RECIPES):
        raise ValueError('Prepared N64 recipe or auxiliary inputs changed')
    if source_inventory(ROOT / 'engines/web') != provenance.get('web_runtime_source_sha256') or source_inventory(work / 'omniplay-web-runtime') != provenance.get('web_runtime_source_sha256'):
        raise ValueError('The shared runtime changed after N64 source preparation')
    build_dir = owned(work / 'build')
    if build_dir.exists():
        raise ValueError('The N64 build requires a fresh output directory')
    build_source = build_dir / 'source'
    shutil.copytree(source, build_source)
    archive = build_dir / 'mupen64plus-next.a'
    command = [str(args.make), '-f', 'Makefile', 'platform=emscripten', 'ARCH=wasm32', 'STATIC_LINKING=1', 'TARGET=' + str(archive),
        'CC=' + str(args.emcc), 'CXX=' + str(args.emxx), 'AR=' + str(args.emar), 'WITH_DYNAREC=',
        'HAVE_THR_AL=0', 'LLE=0', 'HAVE_PARALLEL_RSP=0', 'HAVE_PARALLEL_RDP=0', 'HAVE_NEON=0', 'HAVE_LTCG=0',
        'GLES3=1', 'GLES=0', 'GLFLAGS=-DHAVE_OPENGLES -DHAVE_OPENGLES3 -DGLES3',
        'SYSTEM_ZLIB=0', 'SYSTEM_MINIZIP=0', 'SYSTEM_LIBPNG=0', 'SYSTEM_XXHASH=0',
        'CPUFLAGS=-DEMSCRIPTEN -DNO_ASM', 'DEBUG=0', 'GIT_VERSION=" unknown"', '-j' + str(args.parallel)]
    subprocess.run(command, cwd=build_source, check=True)
    compiled = []
    for name in ('omniplay-frontend.c', 'omniplay-results.c', 'omniplay-webgl2.c'):
        obj = build_dir / (name + '.o')
        subprocess.run([str(args.emcc), '-std=gnu11', *FLAGS, '-I', str(build_source / 'libretro-common/include'), '-c', str(build_source / name), '-o', str(obj)], check=True)
        compiled.append(str(obj))
    output = owned(ROOT / 'apps/game-hub/cores/mupen64plus-web')
    if output.exists():
        raise ValueError('Preserve previous N64 Web artifacts before building another candidate')
    output.mkdir(parents=True)
    module = output / 'mupen64plus.js'
    subprocess.run([str(args.emxx), *FLAGS, *compiled, str(archive), '--no-entry', '-sMODULARIZE=1', '-sEXPORT_ES6=1',
        '-sEXPORTED_FUNCTIONS=@' + str(work / 'exports.json'), '-sEXPORTED_RUNTIME_METHODS=@' + str(work / 'runtime-methods.json'),
        '-o', str(module)], check=True)
    wasm = output / 'mupen64plus.wasm'
    policy = validate_policy(json.loads((work / 'core-options-policy.json').read_text(encoding='utf-8')))
    manifest = {'schema_version': 1, 'id': 'mupen64plus-web', 'core_version': VERSION, 'runtime_version_string': VERSION,
        'source_revision': PIN, 'systems': ['n64'], 'module': 'cores/mupen64plus-web/mupen64plus.js',
        'wasm': 'cores/mupen64plus-web/mupen64plus.wasm', 'wasm_bytes': wasm.stat().st_size, 'wasm_sha256': file_hash(wasm),
        'license_expression': 'GPL-2.0-only', 'state_format_version': 'mupen64plus-webgl1-exact-state',
        'in_game_format_version': 'mupen64plus-webgl1-memory', 'minimum_memory_mb': 1024, 'minimum_system_ram_bytes': 8388608,
        'capabilities': ['pause', 'gamepad', 'keyboard_mouse', 'touch_controls', 'save_ram', 'save_state'],
        'content_formats': ['raw_rom', 'n64_z64', 'n64_v64', 'n64_n64'], 'media_filesystem_protocol': 1,
        'wasm_threads': False, 'libretro_result_protocol': 2, 'hardware_video_protocol': 1,
        'hardware_video': {'schema_version': 1, 'api': 'webgl2', 'maximum_width': 640, 'maximum_height': 480},
        'core_options_protocol': 1, 'core_option_definitions': policy['definitions'],
        'core_options_policy_sha256': provenance['auxiliary_sha256']['core-options-policy.json'],
        'distribution_approved': False, 'verification': 'not_run'}
    notices = ['LICENSE', 'mupen64plus-core/LICENSES', 'mupen64plus-core/doc/gpl-license',
        'mupen64plus-core/doc/lgpl-license', 'mupen64plus-rsp-hle/LICENSES', 'GLideN64/LICENSE',
        'libretro-common/libco/LICENSE.omniplay', 'LICENSE.omniplay-webgl2.txt']
    for index, name in enumerate(notices):
        shutil.copy2(source / name, output / ('LICENSE.n64-' + str(index) + '.txt'))
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    compiled_sources = sorted(path.relative_to(build_source).as_posix() for path in build_source.rglob('*.o'))
    (work / 'build-receipt.json').write_text(json.dumps(dict(provenance, source_only=False,
        compiled_objects=compiled_sources, core_archive_sha256=file_hash(archive),
        module_sha256=file_hash(module), wasm_sha256=file_hash(wasm)), indent=2) + '\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare-only', action='store_true')
    mode.add_argument('--build-prepared', action='store_true')
    parser.add_argument('--git', default='git')
    for name in ('make', 'emcc', 'emxx', 'emar'):
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--parallel', type=int, default=8)
    args = parser.parse_args()
    if not 1 <= args.parallel <= 32:
        raise ValueError('Build parallelism exceeds its bound')
    work = owned(ROOT / 'out/web-cores/mupen64plus-web')
    if args.prepare_only:
        prepare(work, args.git)
        print('Authored pinned N64 WebGL2/interpreter source. No make, compiler, core, browser or test was started.')
        return
    if any(path is None or not path.is_file() for path in (args.make, args.emcc, args.emxx, args.emar)):
        raise ValueError('An explicit N64 build requires actual make, Emscripten C/C++ and archive tool paths')
    build(args, work)
    print('Built an unapproved N64 WebGL2 candidate. No browser, game, device or test was run.')


if __name__ == '__main__':
    main()
