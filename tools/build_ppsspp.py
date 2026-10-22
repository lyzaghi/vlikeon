"""Author/build pinned PSP software/OpenGL packages; no automatic execution or tests."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from ffmpeg_source import PIN as FFMPEG_PIN
from ppsspp_ffmpeg import install_source as install_ffmpeg_source,adapt as adapt_ffmpeg
from build_ffmpeg import admit_receipt as admit_ffmpeg_receipt
from pinned_core_source import owned,file_hash,source_inventory
from ppsspp_native_source import prepare as materialize_native,POLICY as SOURCE_POLICY
from ppsspp_atlas_receipt import admit_generated as admit_atlas

ROOT=Path(__file__).resolve().parents[1]
PIN='a5275067b62433375bbde12921e96329a795dff0'
VERSION='1.20.4+omniplay.sw3.a5275067b624'
GL_VERSION='1.20.4+omniplay.gl3.a5275067b624'
ATLAS_FONT_PINS={'noto-cjk':'f8d157532fbfaeda587e826d4cd5b21a49186f7c','noto-fonts':'ffebf8c1ee449e544955a7e813c54f9b73848eac','freetype':'063ed27f481f0bcd3962fd468e1546739d7d285a'}
RECIPES=('build_ppsspp.py','ppsspp_native_source.py','pinned_core_source.py','ppsspp_ffmpeg.py','ppsspp_audio.py','ffmpeg_source.py','build_ffmpeg.py','ppsspp_atlas_receipt.py')
ADAPTERS=('OmniPlayFFmpeg.h','OmniPlayAudioResampler.h','ppsspp-ffmpeg-import.cmake')

def replace(path,old,new,count=1):
    raw=path.read_text(encoding='utf-8')
    if raw.count(old)!=count:raise ValueError('Pinned PSP adaptation anchor changed: '+path.as_posix())
    path.write_text(raw.replace(old,new),encoding='utf-8',newline='\n')

def prepare(original,source,lock,exclude_assets=False,profile='software'):
    if profile not in {'software','opengl'}:raise ValueError('Choose an explicit PSP source profile')
    gpu=profile=='opengl';version=GL_VERSION if gpu else VERSION
    if original!=ROOT/'third_party/sources/ppsspp' or source.name!='source' or lock['core']['revision']!=PIN:
        raise ValueError('The native PSP adapter requires the exact selected parent/source layout')
    materialized,provenance=materialize_native(source.parent,lock,exclude_assets=exclude_assets)
    if materialized!=source:raise ValueError('Committed PSP materialization returned a different source root')
    (source.parent/'committed-source.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    decoder_source=install_ffmpeg_source(source.parent,source)
    # Avoid host Git/version discovery and the parent's extra Windows batch
    # generator. The exact product identity is a retained source input.
    (source/'git-version.cmake').write_text('file(WRITE "${OUTPUT_DIR}/git-version.cpp" [=[const char *PPSSPP_GIT_VERSION = "'+version+'";\n#define PPSSPP_GIT_VERSION_NO_UPDATE 1\n]=])\n',encoding='utf-8')
    replace(source/'Core/CMakeLists.txt','if (WIN32)\n\tset(WIN_VERSION_CMD COMMAND ${CMAKE_SOURCE_DIR}/Windows/git-version-gen.cmd PPSSPPWindows)\nendif()','')
    core=source/'libretro/libretro.cpp'
    replace(core,'#include "Core/Config.h"','#include "Core/Config.h"\n#include "Core/HLE/sceIo.h"')
    replace(core,'#include <thread>','#include <thread>\n#include <chrono>')
    replace(core,'info->library_version  = PPSSPP_GIT_VERSION;','info->library_version  = "'+version+'";')
    replace(core,'   g_Config.internalDataDirectory = retro_base_dir;','   g_Config.internalDataDirectory = retro_save_dir;')
    raw=core.read_text(encoding='utf-8');start=raw.index("   // Check if '<system_dir>/PPSSPP/compat.ini'");end=raw.index('   g_Config.currentDirectory = retro_base_dir;',start)
    raw=raw[:start]+'''   if (!File::Exists(Path(retro_base_dir / "compat.ini"))) {
      environ_cb(RETRO_ENVIRONMENT_SHUTDOWN, nullptr);
      return;
   }

'''+raw[end:];core.write_text(raw,encoding='utf-8',newline='\n')
    old='''   case SYSPROP_CAN_JIT:
#if PPSSPP_PLATFORM(IOS)
      bool can_jit;
      return (environ_cb(RETRO_ENVIRONMENT_GET_JIT_CAPABLE, &can_jit) && can_jit);
#else
      return true;
#endif'''
    replace(core,old,'   case SYSPROP_CAN_JIT:\n      return false; // contained profiles never request executable CPU/vertex memory')
    cpu='IR_INTERPRETER' if gpu else 'INTERPRETER'
    replace(core,'   check_variables(coreParam);',f'''   check_variables(coreParam);
   g_Config.iCpuCore = (int)CPUCore::{cpu};
   g_Config.bVertexDecoderJit = false;
   g_Config.bSoftwareRendering = {'false' if gpu else 'true'};
   g_Config.iInternalResolution = 1;
   g_Config.bEnableWlan = false;
   g_Config.bEnableAdhocServer = false;
   coreParam.cpuCore = CPUCore::{cpu};
   coreParam.bUseVertexDecoderJit = false;''')
    # Complete initialization before returning Ready. No frame or guest input is
    # executed here. The frontend can now reject/roll back a state synchronously.
    old='''   if (!PSP_InitStart(coreParam)) {
      g_bootErrorString = coreParam.errorString;
      // Can't really fail, the errors normally happen later during InitUpdate
      ERROR_LOG(Log::Boot, "%s", g_bootErrorString.c_str());
      g_pendingBoot = false;
      return false;
   }

   return true;'''
    new='''   if (PSP_Init(coreParam, &g_bootErrorString) != BootState::Complete) {
      g_pendingBoot = false;
      return false;
   }
   g_pendingBoot = false;
   coreState = CORE_RUNNING_CPU;
   g_bootErrorString.clear();
   return gpu && Memory::IsActive();'''
    if not gpu:replace(core,old,new)
    # Exported only by this adapted core. The worker calls it with the guest
    # stopped, then captures file contents after the upstream async I/O barrier.
    with core.open('a',encoding='utf-8') as out:
        out.write('\nextern "C" RETRO_API void omniplay_flush_io(void) { if (Memory::IsActive()) { if (gpu) gpu->FlushPendingDrawing(); __UtilityWaitForIO(); __IoWaitForAsync(); } }\n')
        out.write('extern "C" RETRO_API void omniplay_pause_guest(void) { if (Libretro::useEmuThread) Libretro::EmuThreadPause(); }\n')
        if gpu:out.write('''extern "C" RETRO_API bool omniplay_finish_prepare(void) {
   const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(30);
   while (Libretro::g_pendingBoot) {
      const BootState state = PSP_InitUpdate(&Libretro::g_bootErrorString);
      if (state == BootState::Complete) {
         Libretro::g_pendingBoot = false;
         Libretro::g_bootErrorString.clear();
         coreState = CORE_RUNNING_CPU;
         return gpu && Memory::IsActive();
      }
      if (state != BootState::Booting) { Libretro::g_pendingBoot = false; return false; }
      if (std::chrono::steady_clock::now() >= deadline) return false;
      std::this_thread::sleep_for(std::chrono::milliseconds(1));
   }
   return gpu && Memory::IsActive();
}
''')
    context=source/'libretro/LibretroGraphicsContext.cpp'
    selected_context='LibretroGLCoreContext' if gpu else 'LibretroSoftwareContext'
    replace(context,'LibretroGraphicsContext *LibretroGraphicsContext::CreateGraphicsContext() {',f'''LibretroGraphicsContext *LibretroGraphicsContext::CreateGraphicsContext() {{
   auto *fixed = new {selected_context}();
   std::string fixedError;
   if (!fixed->InitAPI(nullptr, nullptr, &fixedError)) {{ delete fixed; return nullptr; }}
   return fixed;
   // Other implementations remain build inputs, but this signed profile cannot
   // silently fall back to a different graphics or CPU trust policy.''')
    replace(context,'\t\tbool success = draw_->CreatePresets();\n\t\t_assert_(success);','\t\tif (!draw_ || !draw_->CreatePresets()) {\n\t\t\tLibretro::environ_cb(RETRO_ENVIRONMENT_SHUTDOWN, nullptr);\n\t\t\treturn;\n\t\t}')
    if gpu:replace(source/'libretro/LibretroGLCoreContext.h','LibretroHWRenderContext(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 1)','LibretroHWRenderContext(RETRO_HW_CONTEXT_OPENGL_CORE, 3, 3)')
    # Missing approved system data is a load failure, not an accepted warning.
    replace(core,'   ctx       = LibretroGraphicsContext::CreateGraphicsContext();','   ctx       = LibretroGraphicsContext::CreateGraphicsContext();\n   if (!ctx) return false;')
    old='''   // The HW renderer isn't ready on first pass.
   // So we save the data until we are ready to use it.
   if (!gpu) {
      void *buffer = malloc(size);
      if (!buffer)
         return false;
      free(unserialize_data);
      unserialize_data = buffer;
      unserialize_size = size;
      memcpy(unserialize_data, data, size);
      return true;
   }'''
    replace(core,old,'   if (!gpu || !data || !size || size > 256u*1024u*1024u) return false;')
    raw=core.read_text(encoding='utf-8');start=raw.index('bool retro_serialize(void *data, size_t size)');end=raw.index('void *retro_get_memory_data(unsigned id)',start);snapshots=raw[start:end]
    restart='   if (useEmuThread)\n      EmuThreadStart();'
    if snapshots.count(restart)!=2:raise ValueError('Pinned PSP snapshot thread-resume behavior changed')
    raw=raw[:start]+snapshots.replace(restart,'   // The frontend owns resume; snapshots leave the guest paused.')+raw[end:];core.write_text(raw,encoding='utf-8',newline='\n')
    adaptations=(['opengl_core_33_context_only','ir_bytecode_interpreter_no_native_jit','post_context_boot_completion_before_ready'] if gpu else ['software_graphics_context_only','cpu_interpreter','synchronous_boot_before_ready'])+['guarded_graphics_context_creation','vertex_jit_disabled','network_options_disabled_and_appcontainer_network_denied','synchronous_state_load_failure','guest_pause_barrier','snapshot_apis_do_not_resume_guest','async_io_barrier_for_filesystem_save_capture','static_bundled_ffmpeg_source','inventoried_readonly_system_assets','private_filesystem_save_and_snapshot_envelopes']
    decoder_limits=adapt_ffmpeg(source,native=True)
    adaptations.append('pinned_ffmpeg_9_0_2_receipt_and_bounded_decode')
    (source/'OMNIPLAY_FFMPEG_SOURCE.json').write_text(json.dumps({'source':decoder_source,'limits':decoder_limits,'overrides_parent_gitlink':next(entry['revision'] for entry in lock['dependencies'] if entry['name']=='ffmpeg')},indent=2)+'\n',encoding='utf-8')
    (source/'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version':1,'upstream_revision':PIN,'core_version':version,'profile':profile,'adaptations':adaptations,'verification':'not_run','distribution_review_required':['pgf_font_asset_provenance','compiled_third_party_notices','complete_corresponding_source']},indent=2)+'\n',encoding='utf-8')
    return adaptations

def prepare_project(work,profile):
    core_name='ppsspp-gl' if profile=='opengl' else 'ppsspp'
    version=GL_VERSION if profile=='opengl' else VERSION
    original=ROOT/'third_party/sources/ppsspp';lock_path=ROOT/'security/ppsspp-source.lock.json';lock=json.loads(lock_path.read_text(encoding='utf-8'))
    source=work/'source';adaptations=prepare(original,source,lock,profile=profile)
    for name in RECIPES:shutil.copy2(ROOT/'tools'/name,work/name)
    shutil.copy2(lock_path,work/'ppsspp-source.lock.json')
    shutil.copy2(ROOT/'security/ffmpeg-source.lock.json',work/'ffmpeg-source.lock.json')
    record={'schema_version':1,'core':core_name,'version':version,'profile':profile,'revision':PIN,
        'source_policy':SOURCE_POLICY,'source_only':True,'corresponding_source':str(source.resolve()),
        'adaptations':adaptations,'configure':'not_started','build':'not_started','verification':'not_run','distribution_approved':False,
        'prepared_source_sha256':source_inventory(source),
        'recipe_sha256':{name:file_hash(work/name) for name in RECIPES},
        'adapter_sha256':{name:file_hash(ROOT/'engines/native/adapters'/name) for name in ADAPTERS},
        'auxiliary_sha256':{name:file_hash(work/name) for name in ('ppsspp-source.lock.json','ffmpeg-source.lock.json','committed-source.json')},
        'remaining':['actual build/compiler/decoder/atlas/tool ABI and generated inputs','compiled source/notices/SBOM/license/provenance review',
            'containment/thread/IO/save/decoder/guest/platform/device/performance qualification','all original product scope and final consolidated verification']}
    (work/'source-preparation.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')


def build_prepared(args,work,output):
    record_path=work/'source-preparation.json'
    record=json.loads(record_path.read_text(encoding='utf-8'))
    core_name='ppsspp-gl' if args.profile=='opengl' else 'ppsspp'
    version=GL_VERSION if args.profile=='opengl' else VERSION
    if record.get('core')!=core_name or record.get('profile')!=args.profile or record.get('revision')!=PIN or record.get('version')!=version or record.get('source_policy')!=SOURCE_POLICY or record.get('source_only') is not True:
        raise ValueError('The prepared native PSP identity differs from this exact build profile')
    if source_inventory(work/'source')!=record['prepared_source_sha256']:raise ValueError('Prepared PSP source changed after authoring')
    if set(record['recipe_sha256'])!=set(RECIPES) or set(record['adapter_sha256'])!=set(ADAPTERS) or set(record['auxiliary_sha256'])!={'ppsspp-source.lock.json','ffmpeg-source.lock.json','committed-source.json'}:
        raise ValueError('The native PSP source inventory omits a required producer input')
    for name,expected in record['recipe_sha256'].items():
        if file_hash(work/name)!=expected or file_hash(ROOT/'tools'/name)!=expected:raise ValueError('The PSP producer changed after preparation')
    for name,expected in record['adapter_sha256'].items():
        if file_hash(ROOT/'engines/native/adapters'/name)!=expected:raise ValueError('The native PSP source adapter changed after preparation')
    for name,expected in record['auxiliary_sha256'].items():
        if file_hash(work/name)!=expected:raise ValueError('The native PSP source lock/provenance changed')
    lock=json.loads((work/'ppsspp-source.lock.json').read_text(encoding='utf-8'))
    decoder_receipt,decoder_prefix=admit_ffmpeg_receipt(args.ffmpeg_receipt,'windows-x86_64')
    atlas=admit_atlas(args.atlas_receipt,PIN,ATLAS_FONT_PINS)
    if output.exists():raise ValueError('Preserve prior build source and choose a fresh output directory')
    output.mkdir(parents=True)
    source=output/'source';shutil.copytree(work/'source',source)
    shutil.copy2(work/'ppsspp-source.lock.json',output/'ppsspp-source.lock.json')
    shutil.copy2(record_path,output/'source-preparation.json')
    shutil.copy2(work/'committed-source.json',output/'committed-source.json')
    shutil.copy2(work/'ffmpeg-source.lock.json',output/'ffmpeg-source.lock.json')
    for name in RECIPES:shutil.copy2(work/name,output/name)
    for item in atlas['assets']:
        asset=Path(item['source']).resolve(strict=True)
        if not asset.is_file() or not 0<item['bytes']<=32*1024*1024 or asset.stat().st_size!=item['bytes'] or hashlib.sha256(asset.read_bytes()).hexdigest()!=item['sha256']:raise ValueError('Generated PSP atlas differs from its receipt')
        shutil.copy2(asset,source/'assets'/item['name'])
    notice_dir=source/'OMNIPLAY_ASSET_NOTICES';notice_dir.mkdir()
    for index,path in enumerate(atlas['additional_notices']):
        original_notice=Path(path).resolve(strict=True)
        if not original_notice.is_file() or original_notice.stat().st_size>4*1024*1024:raise ValueError('Atlas notice is missing or oversized')
        shutil.copy2(original_notice,notice_dir/f'LICENSE.atlas-{index}.txt')
    shutil.copy2(args.atlas_receipt,output/'atlas-build-receipt.json')
    build=output/'build'
    command=[args.cmake,'-S',source,'-B',build,'-G',args.generator,'-DCMAKE_BUILD_TYPE=Release','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DLIBRETRO=ON','-DUNITTEST=OFF','-DHEADLESS=OFF','-DATLAS_TOOL=OFF','-DUSE_DISCORD=OFF','-DUSE_MINIUPNPC=OFF','-DOPENXR=OFF','-DUSE_CCACHE=OFF','-DUSE_FFMPEG=ON','-DBUILD_BUNDLED_FFMPEG=ON','-DOMNIPLAY_FFMPEG_PREFIX='+decoder_prefix.as_posix(),'-DUSE_SYSTEM_FFMPEG=OFF','-DUSE_SYSTEM_LIBPNG=OFF','-DUSE_SYSTEM_ZSTD=OFF','-DUSE_SYSTEM_RAPIDJSON=OFF','-DUSE_SYSTEM_LIBCHDR=OFF','-DHTTPS_NOT_AVAILABLE=ON','-DCMAKE_DISABLE_FIND_PACKAGE_ZLIB=ON','-DBUILD_TESTING=OFF']
    subprocess.run([str(v) for v in command],check=True)
    subprocess.run([str(args.cmake),'--build',str(build),'--config','Release','--target','ppsspp_libretro','--parallel',str(args.parallel)],check=True)
    libraries=list(build.rglob('ppsspp_libretro.dll'))
    if len(libraries)!=1:raise ValueError('Expected exactly one PSP native library')
    assets=[]
    for relative in ['compat.ini','ppge_atlas.zim','ppge_atlas.meta',*['flash0/font/'+path.name for path in sorted((source/'assets/flash0/font').glob('*.pgf'))]]:
        path=source/'assets'/relative
        if not path.is_file() or path.is_symlink():raise ValueError('Declared PSP system asset is absent or linked')
        assets.append({'source':str(path.resolve()),'path':'assets/PPSSPP/'+relative,'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
    notices=[path for path in source.rglob('*') if path.is_file() and path.name.lower().startswith(('license','copying','copyright')) and path.stat().st_size<=4*1024*1024]
    notice=output/'DEPENDENCY-NOTICES.txt'
    text=''.join('\n===== '+path.relative_to(source).as_posix()+' =====\n'+path.read_text(encoding='utf-8',errors='replace')+'\n' for path in sorted(notices))
    if len(text.encode('utf-8'))>4*1024*1024:raise ValueError('Split full PSP attributions into bounded notice files before packaging')
    notice.write_text(text,encoding='utf-8')
    receipt={'schema_version':1,'core':core_name,'version':version,'revision':PIN,'profile':args.profile,'video_backend':'opengl_core' if args.profile=='opengl' else 'software','prepare_completion':args.profile=='opengl','pause_barrier':True,'library':str(libraries[0].resolve()),'library_sha256':file_hash(libraries[0]),'source_policy':SOURCE_POLICY,'source_only':False,'distribution_approved':False,'prepared_source_receipt_sha256':file_hash(record_path),'committed_source_sha256':file_hash(output/'committed-source.json'),'compiled_source_sha256':source_inventory(source),'recipe_sha256':record['recipe_sha256'],'adapter_sha256':record['adapter_sha256'],'cmake_sha256':file_hash(args.cmake),'configure_command':[str(value) for value in command],'corresponding_source':str(source.resolve()),'source_lock':str((output/'ppsspp-source.lock.json').resolve()),'ffmpeg_revision':FFMPEG_PIN,'audio_output_protocol':1,'ffmpeg_receipt_sha256':hashlib.sha256(args.ffmpeg_receipt.read_bytes()).hexdigest(),'ffmpeg_build_receipt':decoder_receipt,'dependency_sources':{entry['name']:str((source/entry['path']).resolve()) for entry in lock['dependencies']},'asset_corresponding_source':atlas['corresponding_source'],'asset_build_receipt':str((output/'atlas-build-receipt.json').resolve()),'additional_notices':[str(notice.resolve())],'system_assets':assets,'firmware':[],'verification':'not_run','distribution_review_required':['pgf_font_asset_provenance','compiled_third_party_notices','complete_corresponding_source']}
    (output/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8');print('PSP '+args.profile+' library compiled; compatibility, containment, content and license verification remain pending.')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--prepare-only',action='store_true');mode.add_argument('--build-prepared',action='store_true')
    parser.add_argument('--profile',choices=['software','opengl'],default='software');parser.add_argument('--source-project',type=Path)
    parser.add_argument('--cmake',type=Path);parser.add_argument('--atlas-receipt',type=Path);parser.add_argument('--ffmpeg-receipt',type=Path);parser.add_argument('--generator',default='Ninja');parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32:raise ValueError('Choose bounded build parallelism')
    core_name='ppsspp-gl' if args.profile=='opengl' else 'ppsspp'
    work=owned(args.source_project.absolute() if args.source_project else ROOT/'out/native-sources'/(core_name+'-v3-committed'))
    if args.prepare_only:
        prepare_project(work,args.profile);print('Authored committed PSP '+args.profile+' source, exact input inventories and an explicit separate build recipe. No configure, compiler, runtime or test was started.');return
    for name in ('cmake','atlas_receipt','ffmpeg_receipt'):
        path=getattr(args,name)
        if path is None or not path.is_file():raise ValueError('Supply actual explicit CMake, generated-atlas and compiled-decoder receipt paths')
        setattr(args,name,path.resolve(strict=True))
    build_prepared(args,work,owned(ROOT/'out/native-cores'/(core_name+'-v3-committed')))

if __name__=='__main__':main()
