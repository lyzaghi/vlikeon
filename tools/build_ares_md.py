"""Author/build the pinned ares Mega Drive adapter without bundled firmware.

--prepare-only writes source files, manifests and a CMake recipe only. The actual
compiler invocation remains reserved for the final consolidated verification.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
from web_core_options_policy import validate_policy
from pinned_core_source import owned, materialize, file_hash, source_inventory

ROOT=Path(__file__).resolve().parents[1]
PIN='9408cb43d4948fc3ea6e152a307a34348df3fe04'
VERSION='0.3.0+ares.9408cb43d494'
HEADER_PIN='26a56bd6a122a6318a456abd80558408742d5adf'
COMPONENTS=['processor/m68000','processor/z80','processor/sh2','processor/ssp1601','audio/sn76489','audio/ym2612','eeprom/m24c']
SOURCE_PATHS=('LICENSE','ares/ares','ares/md','nall/nall','libco','thirdparty/qon',
              'thirdparty/xxhash.h','thirdparty/sse2neon.h','mia/medium/mega-drive.cpp',
              'desktop-ui/emulator/mega-drive.cpp',*('ares/component/'+name for name in COMPONENTS))

def retire_inputs(directory,work):
    owned(directory)
    if directory!=work/'inputs' or directory.parent.resolve()!=work.resolve():
        raise ValueError('Input retirement must remain inside the exact owned source assembly')
    source_inventory(directory)
    shutil.rmtree(directory)
def replace(path,old,new):
    text=path.read_text(encoding='utf-8')
    if text.count(old)!=1:raise ValueError('Pinned Mega Drive adaptation anchor changed: '+path.as_posix())
    path.write_text(text.replace(old,new,1),encoding='utf-8',newline='\n')

def bounded_serializer(source):
    path=source/'nall/nall/serializer.hpp'
    replace(path,'  explicit operator bool() const {','  auto valid() const -> bool { return !_failed; }\n\n  explicit operator bool() const {')
    replace(path,'    _mode = 0;\n    _size = 0;','    _mode = 0;\n    _size = 0;\n    _failed = false;')
    replace(path,'    _mode = 1;\n    _size = 0;','    _mode = 1;\n    _size = 0;\n    _failed = false;')
    replace(path,'  auto reserve(u32 size) -> void {','''  auto reserve(u32 size) -> void {
    if(_failed || size > 64 * 1024 * 1024 || (reading() && size > _capacity)) {
      _failed = true;
      return;
    }''')
    replace(path,'    if constexpr(has_serialize_v<T>) {','    if(_failed) return *this;\n    if constexpr(has_serialize_v<T>) {')
    replace(path,'    _mode = s._mode;\n    _data = new u8[s._capacity];','    _mode = s._mode;\n    _failed = s._failed;\n    _data = new u8[s._capacity];')
    replace(path,'    _mode = s._mode;\n    _data = s._data;','    _mode = s._mode;\n    _failed = s._failed;\n    _data = s._data;')
    text=path.read_text(encoding='utf-8')
    if text.count('    reserve(_size + size);')!=2:raise ValueError('Pinned ares primitive serializer anchors changed')
    path.write_text(text.replace('    reserve(_size + size);','    if(!available(size)) return *this;'),encoding='utf-8',newline='\n')
    replace(path,'  bool _mode = 0;','''  bool _failed = false;
  auto available(u32 size) -> bool {
    if(_failed || _size > 64 * 1024 * 1024 || size > 64 * 1024 * 1024 - _size ||
       (reading() && (_size > _capacity || size > _capacity - _size))) {
      _failed = true;
      return false;
    }
    if(writing()) reserve(_size + size);
    return !_failed;
  }
  bool _mode = 0;''')
    system=source/'ares/md/system/serialization.cpp'
    replace(system,'auto System::serialize(bool synchronize) -> serializer {','auto System::serialize(bool synchronize) -> serializer {\n  if(!co_serializable()) synchronize = true;')
    replace(system,'  if(signature != SerializerSignature) return false;','  if(!s.valid() || !synchronize || signature != SerializerSignature) return false;')
    replace(system,'  if(string{version} != SerializerVersion) return false;','''  char expectedVersion[16] = {};
  memory::copy(expectedVersion, (const char*)SerializerVersion, SerializerVersion.size());
  if(memcmp(version, expectedVersion, sizeof(version))) return false;''')
    replace(system,'  serialize(s, synchronize);\n  return true;','  serialize(s, synchronize);\n  return s.valid();')
def prepare(original,source,libretro_header=None):
    source.mkdir(parents=True)
    # No resource aggregator, UI resources, embedded firmware or prebuilt binary
    # is copied into the self-contained corresponding-source project.
    for relative in ['ares/ares','ares/md','nall/nall','libco','thirdparty/qon']+['ares/component/'+part for part in COMPONENTS]:
        shutil.copytree(original/relative,source/relative,ignore=shutil.ignore_patterns('.git','resource','*.o','*.a','*.dll','*.exe','*.bin','*.rom','*.png','*.jpg','*.qoi','*.qon','*.wav','*.ogg','*.mp4'))
    for relative in ['LICENSE','thirdparty/xxhash.h','thirdparty/sse2neon.h']:
        target=source/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(original/relative,target)
    (source/'adapter').mkdir()
    shutil.copy2(ROOT/'engines/ares-md/adapter.cpp',source/'adapter/adapter.cpp')
    shutil.copy2(ROOT/'engines/ares-md/web-files.hpp',source/'adapter/web-files.hpp')
    shutil.copy2(ROOT/'engines/native/src/writable_files_v1.h',source/'adapter/writable_files_v1.h')
    shutil.copy2(ROOT/'engines/native/adapters/md-save-publish.c',source/'adapter/native-publish.c')
    shutil.copy2(libretro_header or ROOT/'third_party/sources/sameboy/libretro/libretro.h',source/'adapter/libretro.h')
    # Keep framebuffer delivery on the libretro calling thread. It must not call
    # the host's SDL renderer from ares's optional video worker thread.
    replace(source/'ares/ares/ares.hpp','static constexpr bool Threaded = true;','static constexpr bool Threaded = false;')
    (source/'thirdparty/sljit.h').write_text('#pragma once\n// No Sljit backend is linked by this software-only adapter.\n',encoding='utf-8')
    (source/'nall/nall/recompiler/generic/generic.hpp').write_text('#pragma once\nnamespace nall::recompiler { struct generic { static constexpr bool supported = false; }; }\n',encoding='utf-8')
    sh2=source/'ares/component/processor/sh2/sh2.hpp';sh2_text=sh2.read_text(encoding='utf-8')
    nojit='''  struct Recompiler {
    explicit Recompiler(SH2&) {}
    struct Block { u8* code = nullptr; u8 size = 0; auto execute(SH2&) -> void { ::abort(); } };
    bool enabled = false, callInstructionPrologue = false;
    bump_allocator allocator;
    auto reset() -> void {}
    auto invalidateCached() -> void {}
    auto invalidate(u32, u8) -> void {}
    auto block(u32) -> Block* { return nullptr; }
  } recompiler{*this};'''
    sh2_text,count=re.subn(r'  struct Recompiler : recompiler::generic \{.*?  \} recompiler\{\*this\};',nojit,sh2_text,flags=re.DOTALL)
    if count!=1:raise ValueError('Pinned shared SH2 recompiler declaration changed')
    sh2.write_text(sh2_text,encoding='utf-8',newline='\n')
    replace(source/'ares/component/processor/sh2/sh2.cpp','#include "recompiler.cpp"','// Dynamic-code backend omitted; shared SH2 types use the interpreter.')
    shutil.copy2(ROOT/'engines/native/adapters/windows-libco.c',source/'libco/omniplay-windows-fibers.c')
    shutil.copy2(ROOT/'engines/native/adapters/LICENSE.libco',source/'libco/LICENSE.omniplay')
    replace(source/'libco/libco.c','#if defined(__clang__) || defined(__GNUC__)','#if defined(_WIN32)\n#include "omniplay-windows-fibers.c"\n#elif defined(__clang__) || defined(__GNUC__)')
    thread=source/'ares/ares/scheduler/thread.cpp'
    cleanup='  std::erase_if(EntryPoints(), [this](const EntryPoint& entry) { return entry.handle == _handle; });\n'
    replace(thread,'inline auto Thread::create(double frequency, std::function<void ()> entryPoint) -> void {','inline auto Thread::create(double frequency, std::function<void ()> entryPoint) -> void {\n'+cleanup)
    replace(thread,'inline auto Thread::restart(std::function<void()> entryPoint) -> void {','inline auto Thread::restart(std::function<void()> entryPoint) -> void {\n'+cleanup)
    replace(thread,'inline auto Thread::destroy() -> void {','inline auto Thread::destroy() -> void {\n'+cleanup)
    # The MD interface still names SH2 types shared with the upstream 32X code.
    # Its allocator is ordinary non-executable storage; no executable allocation
    # or page-protection change is made by this adapted implementation.
    (source/'ares/ares/memory/fixed-allocator.cpp').write_text('''#include <ares/ares.hpp>
namespace ares::Memory {
FixedAllocator::FixedAllocator() { _allocator.resize(64_MiB, 0); }
auto FixedAllocator::get() -> bump_allocator& { static FixedAllocator value; return value._allocator; }
}
''',encoding='utf-8')
    replace(source/'ares/md/cartridge/board/board.hpp','  virtual auto save() -> void {}','  virtual auto save() -> void {}\n  virtual auto omniSaveMemory() -> std::span<u8> { return {}; }')
    replace(source/'ares/md/cartridge/board/board.hpp','namespace Board {','namespace Board {\ninline bool omni_save_failed = false;')
    board_helpers=source/'ares/md/cartridge/board/board.cpp'
    original_helpers=board_helpers.read_text(encoding='utf-8')
    first=original_helpers.index('auto Interface::save(Memory::Writable<n16>&')
    if original_helpers[first:].count('auto Interface::save(')!=2 or not original_helpers.rstrip().endswith('}'):
        raise ValueError('Pinned ares persistent-memory helper boundary changed')
    board_helpers.write_text(original_helpers[:first]+'#include "omniplay-save-memory.inc"\n\n}\n',encoding='utf-8',newline='\n')
    shutil.copy2(ROOT/'engines/ares-md/save-memory.inc',source/'ares/md/cartridge/board/omniplay-save-memory.inc')
    bounded_serializer(source)
    memory='''  auto omniSaveMemory() -> std::span<u8> override {
    if(wram) return {reinterpret_cast<u8*>(wram.data()), size_t(wram.size()) * sizeof(n16)};
    if(uram) return {reinterpret_cast<u8*>(uram.data()), size_t(uram.size()) * sizeof(n8)};
    if(lram) return {reinterpret_cast<u8*>(lram.data()), size_t(lram.size()) * sizeof(n8)};
    if(m24c) return {reinterpret_cast<u8*>(m24c.memory), size_t(m24c.size())};
    return {};
  }
'''
    for board in ('standard','linear','banked'):
        replace(source/('ares/md/cartridge/board/'+board+'.cpp'),'  auto save() -> void override {',memory+'\n  auto save() -> void override {')
    replace(source/'ares/md/md.hpp','  auto option(string name, string value) -> bool;','  auto option(string name, string value) -> bool;\n  auto omni_save_memory() -> std::span<u8>;')
    replace(source/'ares/md/cartridge/cartridge.cpp','Cartridge& cartridge = cartridgeSlot.cartridge;','Cartridge& cartridge = cartridgeSlot.cartridge;\nauto omni_save_memory() -> std::span<u8> { return cartridge.board ? cartridge.board->omniSaveMemory() : std::span<u8>{}; }')
    # Preserve the upstream mapper/storage/region analysis, without its file IO,
    # original UI or embedded SVP firmware fallback. No byte array is transplanted.
    raw=(original/'mia/medium/mega-drive.cpp').read_text(encoding='utf-8')
    declaration=raw[:raw.index('\nauto MegaDrive::load(')].replace('struct MegaDrive : Cartridge {','struct MegaDrive {')
    declaration,count=re.subn(r'^  auto (?:name|extensions|load|save)\([^\n]*\n','',declaration,flags=re.MULTILINE)
    if count!=4:raise ValueError('Pinned ares cartridge analysis declaration changed')
    analysis=raw[raw.index('auto MegaDrive::analyze(std::vector<u8>& rom)'):].replace('Medium::name(location)','string{"Game"}')
    if 'Resource::' in analysis or 'file::' in analysis or 'directory::' in analysis:raise ValueError('Cartridge analysis now references external files or firmware resources')
    frontend=(original/'desktop-ui/emulator/mega-drive.cpp').read_text(encoding='utf-8')
    match=re.search(r'static const std::vector<string> serialPrefixes = \{(.*?)\n  \};',frontend,re.DOTALL)
    if not match:raise ValueError('Pinned six-button compatibility list changed')
    prefixes=re.findall(r'"([^"\n]+)"',match[1])
    if not 1<=len(prefixes)<=64:raise ValueError('Controller compatibility list exceeds its bound')
    pad='inline bool six_button_safe(const string& serial) {\n  static const char* const prefixes[] = {'+','.join(json.dumps(v) for v in prefixes)+'};\n  for(auto prefix:prefixes) if(serial.beginsWith(prefix)) return false; return true;\n}\n'
    (source/'adapter/cartridge_analysis.hpp').write_text('// Adapted from pinned ares mia; upstream ISC and component notices are in LICENSE.\n#pragma once\nnamespace omni_md {\n'+declaration+analysis+'\n'+pad+'}\n',encoding='utf-8')
    (source/'adapter/core.cpp').write_text('''#include <ares/ares.hpp>
#include <nall/recompiler/generic/generic.hpp>
#include <ares/debug/debug.cpp>
#include <ares/node/node.cpp>
namespace ares {
Platform* platform = nullptr;
atomic<bool> _runAhead = false;
const string Name = "ares (OmniPlay Mega Drive)";
const string Version = "v146-omniplay-9408cb43d494";
const string Copyright = "ares authors; see LICENSE";
const string License = "ISC";
const string LicenseURI = "https://opensource.org/licenses/ISC";
const string Website = "ares-emu.net";
const string WebsiteURI = "https://ares-emu.net/";
const u32 SerializerSignature = 0x31545342;
static_assert(!Video::Threaded);
static_assert(!nall::recompiler::generic::supported);
}
''',encoding='utf-8')
    cpp=['adapter/adapter.cpp','adapter/core.cpp','ares/ares/memory/fixed-allocator.cpp','ares/md/md.cpp','thirdparty/qon/qon.cpp','libco/libco.c']+['ares/component/'+part+'/'+part.split('/')[-1]+'.cpp' for part in COMPONENTS]
    if any(not (source/path).is_file() for path in cpp):raise ValueError('Selected ares translation unit is missing')
    listing='\n'.join('  "'+path+'"' for path in cpp)
    cmake='''cmake_minimum_required(VERSION 3.28)
project(OmniPlayAresMD LANGUAGES C CXX)
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(CMAKE_MSVC_RUNTIME_LIBRARY MultiThreaded)
if(NOT WIN32)
  message(FATAL_ERROR "The initial MD containment package uses the Windows fiber backend")
endif()
add_library(ares_md_libretro SHARED
'''+listing+'''
)
target_include_directories(ares_md_libretro PRIVATE adapter ares nall . thirdparty)
target_compile_definitions(ares_md_libretro PRIVATE PROFILE_ACCURACY NALL_HEADER_ONLY BUILD_RELEASE OMNIPLAY_FILE_SAVES OMNIPLAY_NATIVE_FILES OMNIPLAY_NATIVE_MEMORY_FILES RETRO_EXPORT_SYMBOLS NOMINMAX _CRT_SECURE_NO_WARNINGS)
set_target_properties(ares_md_libretro PROPERTIES PREFIX "")
if(MSVC)
  target_compile_options(ares_md_libretro PRIVATE /O2 /GS /EHsc /utf-8)
  target_link_options(ares_md_libretro PRIVATE /DYNAMICBASE /NXCOMPAT /HIGHENTROPYVA)
else()
  target_compile_options(ares_md_libretro PRIVATE -O3 -fstack-protector-strong)
  target_link_options(ares_md_libretro PRIVATE -static -static-libgcc -static-libstdc++)
endif()
'''
    (source/'CMakeLists.txt').write_text(cmake,encoding='utf-8')
    changes=['md_only_frontend','memory_only_cartridge_analysis','no_resource_aggregator_or_embedded_firmware','user_svp_file_when_needed','no_jit_backend','non_executable_allocator','windows_fibers_stable_handles_and_rederive','retired_pending_thread_entrypoints','synchronous_video_callback','standard_linear_banked_nonvolatile_memory_interface','checked_pak_memory_writes','bounded_synchronized_state_decode','nall_header_only_definitions','bounded_mdc1_persistent_layout_descriptor','native_memory_file_table_without_host_write_fallback']
    (source/'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version':1,'upstream_revision':PIN,'changes':changes,'initial_platform':'windows-x86_64','compiled':False,'verification':'not_run'},indent=2)+'\n',encoding='utf-8')
    (source/'SOURCE_INPUTS.json').write_text(json.dumps({'schema_version':1,'ares_revision':PIN,'core_version':VERSION,'libretro_header_repository':'https://github.com/LIJI32/SameBoy.git','libretro_header_revision':HEADER_PIN,'libretro_header_sha256':hashlib.sha256((source/'adapter/libretro.h').read_bytes()).hexdigest(),'adapter_sha256':hashlib.sha256((source/'adapter/adapter.cpp').read_bytes()).hexdigest(),'analysis_sha256':hashlib.sha256((source/'adapter/cartridge_analysis.hpp').read_bytes()).hexdigest()},indent=2)+'\n',encoding='utf-8')
    return changes
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--prepare-only',action='store_true');parser.add_argument('--project',type=Path);parser.add_argument('--cmake',type=Path);parser.add_argument('--generator',default='Ninja');parser.add_argument('--cc',type=Path);parser.add_argument('--cxx',type=Path);parser.add_argument('--toolset');parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32 or not args.prepare_only and not args.cmake:raise ValueError('Select bounded parallelism and an explicit compiler recipe')
    original=ROOT/'third_party/sources/ares'
    if subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()!=PIN:raise ValueError('Ares checkout differs from its immutable pin')
    if subprocess.check_output(['git','-C',str(ROOT/'third_party/sources/sameboy'),'rev-parse','HEAD'],text=True).strip()!=HEADER_PIN:raise ValueError('Libretro interface source differs from its immutable pin')
    if args.project and not args.prepare_only:raise ValueError('An explicit fresh source project is only available in prepare-only mode')
    output=owned(args.project.absolute()if args.project else ROOT/('out/native-sources/ares-md' if args.prepare_only else 'out/native-cores/ares-md'))
    if args.project and not output.resolve().is_relative_to((ROOT/'out/native-sources').resolve()):raise ValueError('An explicit MD source project must stay under out/native-sources')
    if output.exists():raise ValueError('Use a fresh Mega Drive output; do not overwrite existing corresponding source')
    policy=ROOT/'engines/policies/ares-md-options.json'
    validate_policy(json.loads(policy.read_text(encoding='utf-8')))
    committed,archive=materialize('ares',PIN,output/'inputs',selected_paths=SOURCE_PATHS)
    archive_hash=file_hash(archive)
    header=subprocess.check_output(['git','-C',str(ROOT/'third_party/sources/sameboy'),'show',HEADER_PIN+':libretro/libretro.h'])
    if not 1024<=len(header)<=2*1024*1024:raise ValueError('Committed libretro interface exceeds its bound')
    header_path=output/'libretro-input.h';header_path.write_bytes(header)
    changes=prepare(committed,output/'source',libretro_header=header_path);shutil.copy2(Path(__file__),output/'build_ares_md.py')
    shutil.copy2(ROOT/'tools/pinned_core_source.py',output/'pinned_core_source.py')
    shutil.copy2(policy,output/'core-options-policy.json')
    shutil.copy2(ROOT/'tools/web_core_options_policy.py',output/'web_core_options_policy.py')
    retire_inputs(output/'inputs',output);header_path.unlink()
    preparation={'schema_version':1,'core':'ares-md','core_version':VERSION,'revision':PIN,'corresponding_source':str((output/'source').resolve()),'upstream_source_sha256':archive_hash,'source_selection':list(SOURCE_PATHS),'upstream_input_tree_retained':False,'prepared_source_sha256':source_inventory(output/'source'),'adaptations':changes,'configure':'not_started','build':'not_started','verification':'not_run'}
    if args.prepare_only:
        (output/'source-preparation.json').write_text(json.dumps(preparation,indent=2)+'\n',encoding='utf-8')
        print('Authored standalone Mega Drive corresponding source and CMake recipe. No configure, compiler or test was started.');return
    command=[args.cmake,'-S',output/'source','-B',output/'build','-G',args.generator,'-DCMAKE_BUILD_TYPE=Release']
    if args.cc:command.append('-DCMAKE_C_COMPILER='+str(args.cc))
    if args.cxx:command.append('-DCMAKE_CXX_COMPILER='+str(args.cxx))
    if args.toolset:command+=['-T',args.toolset]
    subprocess.run([str(v) for v in command],check=True)
    subprocess.run([str(args.cmake),'--build',str(output/'build'),'--config','Release','--target','ares_md_libretro','--parallel',str(args.parallel)],check=True)
    libraries=list((output/'build').rglob('ares_md_libretro.dll'))
    if len(libraries)!=1:raise ValueError('The MD build did not produce one explicit Windows core library')
    receipt={'schema_version':1,'core':'ares-md','revision':PIN,'core_version':VERSION,'library':str(libraries[0].resolve()),'library_sha256':hashlib.sha256(libraries[0].read_bytes()).hexdigest(),'corresponding_source':str((output/'source').resolve()),'verification':'not_run','pause_barrier':True,'file_io_protocol':1,'filesystem_save_memory':False,'core_save_directory':'save/MD','adapter_sha256':{name:hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for name,path in {'adapter.cpp':'engines/ares-md/adapter.cpp','web-files.hpp':'engines/ares-md/web-files.hpp','save-memory.inc':'engines/ares-md/save-memory.inc','windows-libco.c':'engines/native/adapters/windows-libco.c','md-save-publish.c':'engines/native/adapters/md-save-publish.c','writable_files_v1.h':'engines/native/src/writable_files_v1.h'}.items()},'firmware':[{'role':'mega_drive_svp','filename':'svp.rom','allowed_bytes':[2048],'optional':True}]}
    receipt['core_options_policy_sha256']=hashlib.sha256((output/'core-options-policy.json').read_bytes()).hexdigest()
    receipt['portable_contract_protocol']=1
    receipt['writable_files_protocol']=1
    receipt['upstream_source_sha256']=archive_hash
    receipt['source_selection']=list(SOURCE_PATHS)
    receipt['prepared_source_sha256']=preparation['prepared_source_sha256']
    (output/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print('Mega Drive library compiled. Compatibility, saves, timing and containment evidence remain separate.')
if __name__=='__main__':main()
