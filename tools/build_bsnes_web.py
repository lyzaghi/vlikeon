"""Prepare or explicitly build the pinned SNES software WASM candidate.

--prepare-only writes adapted corresponding source, recipes and provenance. It
does not invoke a compiler, configure step, application, emulator or test.
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess

from pinned_core_source import ROOT, file_hash, materialize, owned, source_inventory
from web_core_options_policy import validate_policy

PIN = '63ee22f7af8f5d1ada295c2c98fd7bf261fa62a7'
VERSION = '115.0.0+omniplay.web1'
ADAPTERS = ROOT / 'engines/web/adapters'
DEFINES = ['STDC_HEADERS', '__STDC_LIMIT_MACROS', '__LIBRETRO__', 'SFC_LAGFIX',
           'GB_INTERNAL', 'DISABLE_DEBUGGER', 'FRONTEND_SUPPORTS_RGB565']
# Fixed admission choices keep CPU rates, execution policy, threads, runahead
# and HD scaling outside renderer-controlled per-game configuration.
OPTIONS = {'bsnes_cpu_overclock': '100', 'bsnes_cpu_sa1_overclock': '100',
           'bsnes_cpu_sfx_overclock': '100', 'bsnes_cpu_fastmath': 'OFF',
           'bsnes_ppu_fast': 'OFF', 'bsnes_ppu_no_sprite_limit': 'OFF',
           'bsnes_ppu_no_vram_blocking': 'OFF', 'bsnes_mode7_scale': '1x',
           'bsnes_dsp_fast': 'OFF', 'bsnes_dsp_cubic': 'OFF',
           'bsnes_dsp_echo_shadow': 'OFF', 'bsnes_run_ahead_frames': 'OFF',
           'bsnes_coprocessor_delayed_sync': 'OFF', 'bsnes_coprocessor_prefer_hle': 'ON',
           'bsnes_entropy': 'Low', 'bsnes_touchscreen_lightgun': 'OFF'}


def replace(path, before, after):
    value = path.read_text(encoding='utf-8')
    if value.count(before) != 1:
        raise ValueError('The pinned SNES adaptation anchor changed: ' + path.name)
    path.write_text(value.replace(before, after), encoding='utf-8', newline='\n')


def source_list(source):
    common = (source / 'Makefile.common').read_text(encoding='utf-8')
    roots = {'CORE_DIR': '', 'BSNES_DIR': 'bsnes', 'GB_DIR': 'bsnes/gb', 'SNES_DIR': 'bsnes/sfc'}
    matches = re.findall(r'\$\((CORE_DIR|BSNES_DIR|GB_DIR|SNES_DIR)\)/([A-Za-z0-9_./-]+\.(?:cpp|c)\b)', common)
    names = sorted({(Path(roots[key]) / name).as_posix() for key, name in matches})
    if not 30 <= len(names) <= 256 or any(not (source / name).is_file() for name in names):
        raise ValueError('The committed SNES compile inventory is incomplete')
    return names


def bounded_serializer(source):
    path = source / 'nall/serializer.hpp'
    replace(path, '  explicit operator bool() const {', '''  auto valid() const -> bool { return !_failed; }

  explicit operator bool() const {''')
    replace(path, '    _mode = mode;\n    _size = 0;', '    _mode = mode;\n    _size = 0;\n    _failed = false;')
    # Every primitive and optimized bulk copy checks the allocated buffer.
    # Size mode remains the upstream sizing pass, without a backing buffer.
    replace(path, '    //this is rather dangerous, and not cross-platform safe;',
            '    if(!available(size)) return *this;\n    //this is rather dangerous, and not cross-platform safe;')
    replace(path, '  template<typename T> auto boolean(T& value) -> serializer& {',
            '  template<typename T> auto boolean(T& value) -> serializer& {\n    if(!available(1)) return *this;')
    replace(path, '    enum : uint { size = std::is_same<bool, T>::value ? 1 : sizeof(T) };',
            '    enum : uint { size = std::is_same<bool, T>::value ? 1 : sizeof(T) };\n    if(!available(size)) return *this;')
    replace(path, '  auto array(uint8_t* data, uint size) -> serializer& {',
            '  auto array(uint8_t* data, uint size) -> serializer& {\n    if(!available(size)) return *this;')
    replace(path, '    _mode = s._mode;\n    _data = new uint8_t[s._capacity];',
            '    _mode = s._mode;\n    _failed = s._failed;\n    _data = new uint8_t[s._capacity];')
    replace(path, '    _mode = s._mode;\n    _data = s._data;',
            '    _mode = s._mode;\n    _failed = s._failed;\n    _data = s._data;')
    replace(path, 'private:\n  Mode _mode = Size;', '''private:
  bool _failed = false;
  auto available(uint bytes) -> bool {
    if(_failed) return false;
    if(bytes > UINT32_MAX - _size ||
       (_mode != Size && (_size > _capacity || bytes > _capacity - _size))) {
      _failed = true;
      return false;
    }
    return true;
  }
  Mode _mode = Size;''')
    system = source / 'bsnes/sfc/system/serialization.cpp'
    replace(system, '  if(signature != 0x31545342) return false;',
            '  if(!s.valid() || signature != 0x31545342) return false;')
    replace(system, '  if(string{version} != Emulator::SerializerVersion) return false;', '''  char expectedVersion[16] = {};
  memory::copy(expectedVersion, (const char*)Emulator::SerializerVersion, Emulator::SerializerVersion.size());
  if(memcmp(version, expectedVersion, sizeof(version))) return false;''')
    replace(system, '  serializeAll(s, synchronize);\n  return true;',
            '  serializeAll(s, synchronize);\n  return s.valid();')


def prepare(work, git):
    source, archive = materialize('bsnes', PIN, work, git)
    archive_hash = file_hash(archive)
    resources = source / 'bsnes/target-libretro/resources.hpp'
    value, count = re.subn(r'const unsigned char iplrom\[64\]\s*=\s*\{[^}]*\};', '', resources.read_text(encoding='utf-8'))
    if count != 1:
        raise ValueError('The pinned SNES embedded IPL declaration changed')
    resources.write_text(value, encoding='utf-8', newline='\n')
    # Retain only the adapted corresponding source. The raw upstream archive
    # contains the removed manufacturer array and is not a distribution input.
    archive.unlink()
    shutil.copy2(ADAPTERS / 'bsnes-fibers.c', source / 'libco/omniplay-fibers.c')
    shutil.copy2(ADAPTERS / 'LICENSE.libco', source / 'libco/LICENSE.omniplay')
    (source / 'libco/libco.c').write_text('#include "omniplay-fibers.c"\n', encoding='utf-8')
    shutil.copy2(ADAPTERS / 'bsnes-save-files.cpp.inc', source / 'bsnes/target-libretro/omniplay-save-files.inc')
    program = source / 'bsnes/target-libretro/program.cpp'
    replace(program, '#include "resources.hpp"', '#include "resources.hpp"\n#include "omniplay-save-files.inc"')
    replace(program, 'result = vfs::memory::file::open(iplrom, sizeof(iplrom));',
            '''const char *directory = nullptr;
		if (environ_cb && environ_cb(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY, &directory) &&
		    directory && !std::strcmp(directory, "/system")) {
			result = vfs::fs::file::open("/system/ipl.rom", mode);
			if (result && result->size() != 64) result = {};
		}''')
    replace(program, 'shared_pointer<vfs::file> result;\n',
            '''shared_pointer<vfs::file> result;
	// Every single-cartridge SRAM, coprocessor RAM and RTC filename follows
	// the upstream memory-name contract. No content-controlled path is used.
	if (id == 1 && om_bsnes_save_name(name))
		return OmBsnesSaveFile::open(name, mode);
	if (mode != vfs::file::mode::read) { om_bsnes_io_failed = true; return {}; }
''')
    replace(program, '\treturn result;\n}',
            '\tif (required && !result) om_bsnes_required_failed = true;\n\treturn result;\n}')
    replace(program, '\temulator->load();',
            '''	if (!emulator->load() || om_bsnes_required_failed || om_bsnes_io_failed) return;
	const auto& devices = SuperFamicom::cartridge.has;
	if (devices.ICD || devices.BSMemorySlot || devices.SufamiTurboSlotA || devices.SufamiTurboSlotB) {
		om_bsnes_required_failed = true; return;
	}''')
    replace(program, 'auto Program::loadFile(string location) -> vector<uint8_t>\n{', '''auto Program::loadFile(string location) -> vector<uint8_t>
{
	FILE *bounded = std::fopen((const char *)location, "rb");
	if (!bounded) return {};
	long actual = -1;
	if (!std::fseek(bounded, 0, SEEK_END)) actual = std::ftell(bounded);
	const bool closed = std::fclose(bounded) == 0;
	if (!closed || actual < 0x8000 || actual > 64 * 1024 * 1024) return {};''')
    replace(program, '\treturn { id, options(0) };', '\treturn {}; // Failed or unavailable media must not masquerade as loaded.')
    # The original splits heuristically sized ROM regions without checking that
    # each range still lies in the actually loaded vector.
    for region in ('program', 'data', 'expansion', 'firmware'):
        replace(program, '\t\tsuperFamicom.' + region + '.resize(size);',
                '\t\tif (offset > rom.size() || size > rom.size() - offset) return false;\n\t\tsuperFamicom.' + region + '.resize(size);')
    core = source / 'bsnes/target-libretro/libretro.cpp'
    # Remove the special loader before adapting the single-cartridge call site.
    start = core.read_text(encoding='utf-8')
    first = start.index('RETRO_API bool retro_load_game_special(')
    last = start.index('RETRO_API void retro_unload_game()', first)
    start = start[:first] + '''RETRO_API bool retro_load_game_special(unsigned, const struct retro_game_info *, size_t)
{
	return false;
}

''' + start[last:]
    core.write_text(start, encoding='utf-8', newline='\n')
    # Fixed execution choices live inside the core adapter, rather than in an
    # unused manifest field or a renderer-supplied option map.
    fixed_rows = '\n'.join('\t\t{' + json.dumps(key) + ', ' + json.dumps(value) + '},' for key, value in OPTIONS.items())
    replace(core, 'RETRO_API void retro_set_environment(retro_environment_t cb)', '''static retro_environment_t om_frontend_environment = nullptr;
static bool om_bsnes_environment(unsigned command, void *data) {
	if (command == RETRO_ENVIRONMENT_GET_VARIABLE && data) {
		auto *variable = static_cast<retro_variable *>(data);
		static const struct { const char *key; const char *value; } fixed[] = {
''' + fixed_rows + '''
		};
		if (variable->key) for (const auto& choice : fixed) {
			if (!std::strcmp(variable->key, choice.key)) { variable->value = choice.value; return true; }
		}
	}
	return om_frontend_environment ? om_frontend_environment(command, data) : false;
}

RETRO_API void retro_set_environment(retro_environment_t cb)''')
    replace(core, '\tenviron_cb = cb;', '\tom_frontend_environment = cb;\n\tenviron_cb = om_bsnes_environment;')
    replace(core, 'info->library_version  = Emulator::Version;', 'info->library_version  = "' + VERSION + '";')
    replace(core, 'info->valid_extensions = "smc|sfc|gb|gbc|bs";', 'info->valid_extensions = "smc|sfc";')
    replace(core, '''RETRO_API bool retro_load_game(const retro_game_info *game)
{''', '''RETRO_API bool retro_load_game(const retro_game_info *game)
{
	if (!program || !emulator || !game || !game->path ||
	    !(string(game->path).endsWith(".sfc") || string(game->path).endsWith(".smc"))) return false;
	om_bsnes_io_failed = false;
	om_bsnes_required_failed = false;''')
    replace(core, '\tprogram->load();\n\n\temulator->connect',
            '\tprogram->load();\n\tif (!emulator->loaded() || om_bsnes_required_failed || om_bsnes_io_failed) { emulator->unload(); return false; }\n\n\temulator->connect')
    replace(core, '\tdelete program;\n}', '\tdelete program;\n\tprogram = nullptr;\n\temulator = nullptr;\n\taudio_buffer_index = 0;\n}')
    replace(core, '\treturn emulator->serialize().size();', '\treturn emulator && emulator->loaded() ? emulator->serialize().size() : 0;')
    replace(core, '\tmemcpy(data, emulator->serialize().data(), size);\n\treturn true;',
            '''	if (!data || !emulator || !emulator->loaded()) return false;
	auto snapshot = emulator->serialize();
	if (!snapshot.size() || size != snapshot.size()) return false;
	memcpy(data, snapshot.data(), size);
	return true;''')
    replace(core, '\tserializer s(static_cast<const uint8_t *>(data), size);',
            '''	if (!data || !emulator || !emulator->loaded() || size < 538 ||
	    size != emulator->serialize().size()) return false;
	// This backend cannot restore unsynchronized raw coroutine stack images.
	if (static_cast<const uint8_t *>(data)[536] != 1 ||
	    static_cast<const uint8_t *>(data)[537] > 1) return false;
	serializer s(static_cast<const uint8_t *>(data), size);''')
    with core.open('a', encoding='utf-8', newline='\n') as output:
        output.write('''
extern "C" int om_web_flush_save_io(void) {
	if (!program || !emulator || !emulator->loaded() ||
	    om_bsnes_required_failed || om_bsnes_save_handles) return 0;
	om_bsnes_io_failed = false;
	om_bsnes_save_name_count = 0;
	om_bsnes_barrier = true;
	program->save();
	om_bsnes_barrier = false;
	return !om_bsnes_io_failed && !om_bsnes_save_handles ? 1 : 0;
}

// Void outer exports let the cooperative trampoline finish before JavaScript
// observes the result written below. No Promise/event-loop yield is admitted.
extern "C" void om_web_libretro_call(unsigned operation, uintptr_t first,
                                      uintptr_t second, uint32_t *result) {
	if (!result) return;
	uint32_t value = 0;
	switch (operation) {
	case 1: value = retro_load_game(reinterpret_cast<const retro_game_info *>(first)) ? 1 : 0; break;
	case 2: value = static_cast<uint32_t>(retro_serialize_size()); break;
	case 3: value = retro_serialize(reinterpret_cast<void *>(first), static_cast<size_t>(second)) ? 1 : 0; break;
	case 4: value = retro_unserialize(reinterpret_cast<const void *>(first), static_cast<size_t>(second)) ? 1 : 0; break;
	case 5: value = om_web_flush_save_io() == 1 ? 1 : 0; break;
	default: return;
	}
	result[0] = value;
	result[1] = 1;
}
''')
    bounded_serializer(source)
    shutil.copy2(ROOT / 'engines/web/frontend.c', source / 'omniplay-frontend.c')
    exports = json.loads((ROOT / 'engines/web/exports.json').read_text(encoding='utf-8')) + ['_om_web_flush_save_io', '_om_web_libretro_call']
    (work / 'exports.json').write_text(json.dumps(exports, indent=2) + '\n', encoding='utf-8')
    shutil.copy2(ROOT / 'engines/web/runtime-methods.json', work / 'runtime-methods.json')
    shutil.copytree(ROOT / 'engines/web', work / 'omniplay-web-runtime')
    for name in ('pinned_core_source.py', 'web_core_options_policy.py', 'build_bsnes_web.py'):
        shutil.copy2(ROOT / 'tools' / name, work / name)
    policy = ROOT / 'engines/policies/bsnes-options.json'
    validate_policy(json.loads(policy.read_text(encoding='utf-8')))
    shutil.copy2(policy, work / 'core-options-policy.json')
    provenance = {'schema_version': 1, 'core': 'bsnes-web', 'revision': PIN,
        'upstream_source_sha256': archive_hash, 'upstream_archive_retained': False,
        'adaptations': ['external_user_supplied_spc_ipl', 'single_worker_asyncify_libco_fibers',
                        'bounded_owned_fiber_stacks_and_rederive', 'synchronized_chip_state_only',
                        'checked_bounded_sram_coprocessor_ram_rtc_files', 'explicit_load_failure',
                        'checked_heuristic_rom_regions', 'bounded_state_primitive_reads_and_version',
                        'fixed_core_execution_options', 'void_bridge_completion_slot', 'single_snes_cartridge_candidate'],
        'adapter_sha256': {name: file_hash(ADAPTERS / name) for name in ('bsnes-fibers.c', 'bsnes-save-files.cpp.inc')},
        'compile_sources': source_list(source),
        'frontend_source_sha256': file_hash(source / 'omniplay-frontend.c'),
        'exports_sha256': file_hash(work / 'exports.json'), 'runtime_methods_sha256': file_hash(work / 'runtime-methods.json'),
        'core_options_policy_sha256': file_hash(work / 'core-options-policy.json'),
        'recipe_sha256': {name: file_hash(work / name) for name in ('pinned_core_source.py', 'web_core_options_policy.py', 'build_bsnes_web.py')},
        'web_runtime_source_sha256': source_inventory(work / 'omniplay-web-runtime'),
        'fixed_options': OPTIONS, 'source_only': True, 'verification': 'not_run', 'distribution_approved': False,
        'remaining': ['exact SDK and compiled dependency inventory', 'fiber/serialization fault qualification',
                      'firmware and enhancement-chip compatibility', 'multi-cart/sidecar adapters',
                      'actual device memory/performance', 'security/license/release admission']}
    (source / 'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({key: provenance[key] for key in ('schema_version', 'core', 'revision', 'adaptations')}, indent=2) + '\n', encoding='utf-8')
    provenance['prepared_source_sha256'] = source_inventory(source)
    (work / 'corresponding-source.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
    return source, provenance


def build(args, work, source, provenance):
    policy = validate_policy(json.loads((work / 'core-options-policy.json').read_text(encoding='utf-8')))
    common = ['-O3', '-fno-strict-aliasing', '-sENVIRONMENT=worker', '-sSTACK_SIZE=2097152',
              '-sINITIAL_MEMORY=67108864', '-sALLOW_MEMORY_GROWTH=1', '-sMAXIMUM_MEMORY=1073741824',
              '-sALLOW_TABLE_GROWTH=1', '-sFILESYSTEM=1', '-sASYNCIFY=1', '-sASYNCIFY_IGNORE_INDIRECT=0',
              '-sSTACK_OVERFLOW_CHECK=2', '-sASSERTIONS=1']
    include = [flag for name in ('.', 'bsnes', 'bsnes/gb', 'bsnes/sfc') for flag in ('-I', str(source / name))]
    defines = ['-D' + name for name in DEFINES]
    objects = work / 'objects'
    objects.mkdir(exist_ok=False)
    compiled = []
    for index, name in enumerate([*provenance['compile_sources'], 'omniplay-frontend.c']):
        obj = objects / (str(index) + '.o')
        compiler = args.emxx if name.endswith('.cpp') else args.emcc
        standard = '-std=c++17' if name.endswith('.cpp') else '-std=gnu11'
        subprocess.run([str(compiler), standard, *common, *include, *defines, '-c', str(source / name), '-o', str(obj)], check=True)
        compiled.append(str(obj))
    output = owned(ROOT / 'apps/game-hub/cores/bsnes')
    if output.exists():
        raise ValueError('Preserve prior core artifacts before building a fresh candidate')
    output.mkdir(parents=True)
    subprocess.run([str(args.emxx), *common, *compiled, '--no-entry', '-sMODULARIZE=1', '-sEXPORT_ES6=1',
                    '-sEXPORTED_FUNCTIONS=@' + str(work / 'exports.json'),
                    '-sEXPORTED_RUNTIME_METHODS=@' + str(work / 'runtime-methods.json'),
                    '-o', str(output / 'bsnes.js')], check=True)
    wasm = output / 'bsnes.wasm'
    manifest = {'schema_version': 1, 'id': 'bsnes-web', 'core_version': VERSION, 'runtime_version_string': VERSION,
        'source_revision': PIN, 'systems': ['snes'], 'module': 'cores/bsnes/bsnes.js', 'wasm': 'cores/bsnes/bsnes.wasm',
        'wasm_bytes': wasm.stat().st_size, 'wasm_sha256': file_hash(wasm),
        'license_expression': 'GPL-3.0-only AND ISC AND MIT', 'state_format_version': 'bsnes115-omweb1-sync-fibers',
        'in_game_format_version': 'bsnes115-omweb1-ram-rtc-files', 'minimum_memory_mb': 512,
        'capabilities': ['pause', 'gamepad', 'keyboard_mouse', 'touch_controls', 'save_ram', 'save_state', 'save_files'],
        'content_formats': ['raw_rom'], 'media_filesystem_protocol': 1,
        'saved_files_protocol': 1, 'saved_files_io': 'single_threaded_barrier_v1', 'wasm_threads': False, 'libretro_result_protocol': 1,
        'core_options_protocol': 1, 'core_option_definitions': policy['definitions'],
        'core_options_policy_sha256': provenance['core_options_policy_sha256'],
        'frontend_source_sha256': provenance['frontend_source_sha256'], 'adapter_sha256': provenance['adapter_sha256'],
        'firmware_protocol': 1, 'firmware': [{'role': 'snes_spc_ipl', 'filename': 'ipl.rom', 'allowed_bytes': [64]}],
        'distribution_approved': False, 'verification': 'not_run'}
    shutil.copy2(source / 'LICENSE.txt', output / 'LICENSE.bsnes.txt')
    shutil.copy2(source / 'bsnes/gb/LICENSE', output / 'LICENSE.bundled-sameboy.txt')
    shutil.copy2(source / 'libco/LICENSE.omniplay', output / 'LICENSE.libco.txt')
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    receipt = dict(provenance, source_only=False, module_sha256=file_hash(output / 'bsnes.js'), wasm_sha256=file_hash(wasm))
    (work / 'build-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare-only', action='store_true')
    mode.add_argument('--build-prepared', action='store_true')
    parser.add_argument('--git', default='git')
    parser.add_argument('--emcc', type=Path)
    parser.add_argument('--emxx', type=Path)
    args = parser.parse_args()
    work = owned(ROOT / 'out/web-cores/bsnes')
    if args.prepare_only:
        prepare(work, args.git)
        print('Authored pinned SNES Web corresponding source. No compiler, core or test was started.')
        return
    if not args.emcc or not args.emxx or not args.emcc.is_file() or not args.emxx.is_file():
        raise ValueError('An explicit prepared-source build needs actual Emscripten C and C++ compiler paths')
    source = work / 'source'
    provenance = json.loads((work / 'corresponding-source.json').read_text(encoding='utf-8'))
    if provenance.get('revision') != PIN or provenance.get('source_only') is not True or source_inventory(source) != provenance.get('prepared_source_sha256') or source_list(source) != provenance.get('compile_sources') or provenance.get('fixed_options') != OPTIONS:
        raise ValueError('Prepared SNES source differs from its retained assembly receipt')
    for name, field in [('exports.json', 'exports_sha256'), ('runtime-methods.json', 'runtime_methods_sha256'), ('core-options-policy.json', 'core_options_policy_sha256')]:
        if file_hash(work / name) != provenance.get(field):
            raise ValueError('Prepared SNES frontend/policy differs from its retained receipt')
    recipes = ('pinned_core_source.py', 'web_core_options_policy.py', 'build_bsnes_web.py')
    if any(file_hash(work / name) != provenance.get('recipe_sha256', {}).get(name) or file_hash(ROOT / 'tools' / name) != provenance.get('recipe_sha256', {}).get(name) for name in recipes):
        raise ValueError('The SNES build recipe differs from its retained source assembly')
    if source_inventory(work / 'omniplay-web-runtime') != provenance.get('web_runtime_source_sha256') or source_inventory(ROOT / 'engines/web') != provenance.get('web_runtime_source_sha256'):
        raise ValueError('The shared runtime differs from the retained SNES source assembly')
    build(args, work, source, provenance)
    print('Built an unapproved SNES Web candidate. No browser, game, compatibility or test was run.')


if __name__ == '__main__':
    main()
