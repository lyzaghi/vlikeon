"""Prepare or explicitly build the pinned PSP software/pthread Web candidate.

Preparation only reads immutable Git objects, authors bounded adapters and source
inventories. It never configures, compiles or executes a core, SDK or test.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from pinned_core_source import ROOT, file_hash, materialize, owned, source_inventory
from web_core_options_policy import validate_policy
from ffmpeg_source import PIN as FFMPEG_PIN
from ppsspp_ffmpeg import install_source as install_ffmpeg_source,adapt as adapt_ffmpeg
from build_ffmpeg import admit_receipt as admit_ffmpeg_receipt
from ppsspp_atlas_receipt import admit_generated as admit_atlas

PIN='a5275067b62433375bbde12921e96329a795dff0'
SDK='e13ee988ce3d87a738dd8f3661d02223308c412a'
VERSION='1.20.4+omniplay.websw3.a5275067b624'
ATLAS_FONT_PINS={'noto-cjk':'f8d157532fbfaeda587e826d4cd5b21a49186f7c','noto-fonts':'ffebf8c1ee449e544955a7e813c54f9b73848eac','freetype':'063ed27f481f0bcd3962fd468e1546739d7d285a'}
EXT=('at3_standalone','basis_universal','cityhash','disarm.cpp','disarm.h','gason','glew','glslang-build','imgui','jpge',
    'libchdr-build','libkirk','libpng17','libzip','loongarch-disasm.cpp','loongarch-disasm.h','lua-build','lzma-sdk',
    'nanosvg','native','portable-file-dialogs','pugixml','rcheevos-build','riscv-disas.cpp','riscv-disas.h','sfmt19937',
    'sha1','snappy','sol','stb','udis86','vma','xbrz','xxhash.c','xxhash.h','zlib','zstd-build')
SELECTED=('LICENSE.TXT','README.md','ppsspp_config.h','Common','Core','GPU','UI','headless','libretro','assets/compat.ini','assets/flash0/font',
    *('ext/'+name for name in EXT), 'ext/vulkan/vulkan.h','ext/vulkan/vulkan_core.h','ext/vulkan/vk_platform.h','ext/vulkan/vk_video')
RECIPES=('build_ppsspp_web.py','pinned_core_source.py','web_core_options_policy.py','web_thread_policy.py','web_system_assets_policy.py','ffmpeg_source.py','ppsspp_ffmpeg.py','build_ffmpeg.py','ppsspp_audio.py','ppsspp_atlas_receipt.py')
ADAPTERS=('threaded-frontend.c','LICENSE.threaded','libretro-results.c','ppsspp-thread-pre.js','ppsspp-save-barrier.cpp.inc')
SDK_FILES=('LICENSE','src/lib/libpthread.js','src/runtime_pthread.js','src/lib/libcore.js','src/settings.js','system/include/emscripten/em_asm.h','system/include/emscripten/threading.h','emscripten-version.txt')
FLAGS=['-O3','-pthread','-msimd128','-msse3','-mssse3','-msse4.1','-fexceptions','-fno-fast-math','-ffp-contract=off',
    '-sENVIRONMENT=worker','-sMODULARIZE=1','-sEXPORT_ES6=1','-sEXPORT_NAME=createOmniPlayPspCore',
    '-sPTHREAD_POOL_SIZE=16','-sPTHREAD_POOL_SIZE_STRICT=2','-sPTHREAD_POOL_DELAY_LOAD=0',
    '-sSTACK_SIZE=4194304','-sDEFAULT_PTHREAD_STACK_SIZE=2097152','-sINITIAL_MEMORY=268435456',
    '-sMAXIMUM_MEMORY=1073741824','-sALLOW_MEMORY_GROWTH=1','-sALLOW_TABLE_GROWTH=1',
    '-sFILESYSTEM=1','-sWASMFS=0','-sCROSS_ORIGIN=0','-sDYNAMIC_EXECUTION=0','-sWASM_ESM_INTEGRATION=0','-sMIN_WEBGL_VERSION=2','-sMAX_WEBGL_VERSION=2',
    '-sSTACK_OVERFLOW_CHECK=2','-sASSERTIONS=1']


def replace(path,before,after,count=1):
    text=path.read_text(encoding='utf-8')
    if text.count(before)!=count:raise ValueError('Pinned PSP Web source anchor changed: '+path.name)
    path.write_text(text.replace(before,after),encoding='utf-8',newline='\n')


def materialize_sdk(work,git):
    original=ROOT/'third_party/sources/emscripten'
    if subprocess.check_output([git,'-C',str(original),'rev-parse','HEAD'],text=True).strip()!=SDK:raise ValueError('The selected SDK source checkout changed')
    destination=owned(work/'sdk-source/source')
    if destination.exists():raise ValueError('Preserve the previous SDK source selection')
    destination.mkdir(parents=True)
    # A sparse partial-clone archive can try to lazily fetch unrelated excluded
    # blobs. Select each immutable object directly; no working-tree bytes or
    # remote latest pointer is used, and no compiler/script is executed.
    for name in SDK_FILES:
        identity=SDK+':'+name
        count=int(subprocess.check_output([git,'-C',str(original),'cat-file','-s',identity],text=True).strip())
        if not 0<count<=4*1024**2:raise ValueError('The committed SDK contract file exceeds its bound')
        path=destination/name;path.parent.mkdir(parents=True,exist_ok=True)
        with path.open('xb') as target:subprocess.run([git,'-C',str(original),'show',identity],stdout=target,check=True)
        if path.stat().st_size!=count:raise ValueError('The committed SDK contract was truncated')
    return destination


def prepare(work,git):
    lock_path=ROOT/'security/ppsspp-source.lock.json';lock=json.loads(lock_path.read_text(encoding='utf-8'))
    sdk_path=ROOT/'security/emscripten-source.lock.json';sdk=json.loads(sdk_path.read_text(encoding='utf-8'))
    if lock['core']['revision']!=PIN or sdk['revision']!=SDK or sdk['version']!='6.0.13':raise ValueError('The PSP/SDK lock differs from this recipe')
    source,archive=materialize('ppsspp',PIN,work,git,SELECTED);archives={'ppsspp':file_hash(archive)};archive.unlink()
    inputs=owned(work/'inputs');inputs.mkdir()
    for entry in lock['dependencies']:
        if entry['name']=='ffmpeg':continue
        selection=('LICENSE','COPYING','lib') if entry['name']=='ext-zstd' else ()
        tree,archive=materialize('ppsspp-deps/'+entry['name'],entry['revision'],inputs/entry['name'],git,selection)
        archives[entry['name']]=file_hash(archive);archive.unlink();target=owned(source/entry['path'])
        if target.exists():
            if target.is_symlink() or any(target.iterdir()):raise ValueError('A PSP gitlink destination contains unrelated source')
            target.rmdir()
        target.parent.mkdir(parents=True,exist_ok=True);tree.rename(target)
    decoder_source=install_ffmpeg_source(work,source,git)
    sdk_tree=materialize_sdk(work,git)
    # Retain the exact SDK worker contracts and license, not generated glue or a
    # claim that LLVM/Binaryen/cache binaries have been installed/qualified.
    sdk_hashes={name:file_hash(sdk_tree/name) for name in SDK_FILES}
    core=source/'libretro/libretro.cpp'
    replace(core,'#include <cstdlib>','#include <cstdlib>\n#include <cstdio>\n#include "Core/HLE/sceIo.h"')
    replace(core,'info->library_version  = PPSSPP_GIT_VERSION;','info->library_version  = "'+VERSION+'";')
    replace(core,'   g_Config.internalDataDirectory = retro_base_dir;','   g_Config.internalDataDirectory = Path("/scratch");')
    replace(core,'   g_Config.currentDirectory = retro_base_dir;','   g_Config.currentDirectory = Path("/game");')
    replace(core,'   g_Config.defaultCurrentDirectory = retro_base_dir;','   g_Config.defaultCurrentDirectory = Path("/game");')
    text=core.read_text(encoding='utf-8');start=text.index("   // Check if '<system_dir>/PPSSPP/compat.ini'");end=text.index('   g_Config.currentDirectory',start)
    text=text[:start]+'''   if (!File::Exists(Path(retro_base_dir / "compat.ini"))) {
      environ_cb(RETRO_ENVIRONMENT_SHUTDOWN, nullptr);
      return;
   }

'''+text[end:];core.write_text(text,encoding='utf-8',newline='\n')
    replace(core,'   g_threadManager.Init(cpu_info.num_cores, cpu_info.logical_cpu_count);','   g_threadManager.Init(1, 1); // bounded compute + two IO workers')
    replace(source/'Common/Thread/ThreadManager.cpp','const int MIN_IO_BLOCKING_THREADS = 4;','const int MIN_IO_BLOCKING_THREADS = 2;')
    replace(core,'''   case SYSPROP_CAN_JIT:
#if PPSSPP_PLATFORM(IOS)
      bool can_jit;
      return (environ_cb(RETRO_ENVIRONMENT_GET_JIT_CAPABLE, &can_jit) && can_jit);
#else
      return true;
#endif''','   case SYSPROP_CAN_JIT:\n      return false;')
    replace(core,'   coreParam.bUseVertexDecoderJit = System_GetPropertyBool(SYSPROP_CAN_JIT);','   coreParam.bUseVertexDecoderJit = false;')
    replace(core,'   check_variables(coreParam);','''   check_variables(coreParam);
   g_Config.iCpuCore = (int)CPUCore::IR_INTERPRETER;
   g_Config.bVertexDecoderJit = false;
   g_Config.bSoftwareRendering = true;
   g_Config.iInternalResolution = 1;
   g_Config.iRewindSnapshotInterval = 0;
   g_Config.iMaxRecent = 0;
   g_Config.bSaveSettings = false;
   g_Config.bCacheFullIsoInRam = false;
   g_Config.bEnableWlan = false;
   g_Config.bEnableAdhocServer = false;
   g_Config.bEnableUPnP = false;
   g_Config.bAchievementsEnable = false;
   g_Config.bEnablePlugins = false;
   g_Config.bEnableFileHandlerPlugins = false;
   g_Config.bEnableCheats = false;
   g_Config.bAutoUpgradeFirmware = false;
   g_Config.sReportHost.clear();
   coreParam.cpuCore = CPUCore::IR_INTERPRETER;
   coreParam.bUseVertexDecoderJit = false;''')
    replace(core,'   if (!PSP_InitStart(coreParam)) {\n      g_bootErrorString = coreParam.errorString;\n      // Can\'t really fail, the errors normally happen later during InitUpdate\n      ERROR_LOG(Log::Boot, "%s", g_bootErrorString.c_str());\n      g_pendingBoot = false;\n      return false;\n   }\n\n   return true;',
'''   if (PSP_Init(coreParam, &g_bootErrorString) != BootState::Complete) {
      g_pendingBoot = false;
      return false;
   }
   g_pendingBoot = false;
   coreState = CORE_RUNNING_CPU;
   g_bootErrorString.clear();
   return gpu && Memory::IsActive();''')
    context=source/'libretro/LibretroGraphicsContext.cpp';text=context.read_text(encoding='utf-8');start=text.index('LibretroGraphicsContext *LibretroGraphicsContext::CreateGraphicsContext() {');end=text.index('\nstd::vector<u32> ConvertFramebufferForLibretro',start)
    text=text[:start]+'''LibretroGraphicsContext *LibretroGraphicsContext::CreateGraphicsContext() {
   auto *context = new LibretroSoftwareContext();
   std::string error;
   if (!context->InitAPI(nullptr, nullptr, &error)) { delete context; return nullptr; }
   return context;
}
'''+text[end:];context.write_text(text,encoding='utf-8',newline='\n')
    replace(core,'   ctx       = LibretroGraphicsContext::CreateGraphicsContext();','   ctx       = LibretroGraphicsContext::CreateGraphicsContext();\n   if (!ctx) return false;')
    replace(core,'   if (!gpu) {\n      void *buffer = malloc(size);\n      if (!buffer)\n         return false;\n      free(unserialize_data);\n      unserialize_data = buffer;\n      unserialize_size = size;\n      memcpy(unserialize_data, data, size);\n      return true;\n   }','   if (!gpu || !data || !size || size > 256u*1024u*1024u) return false;')
    replace(core,'   if (useEmuThread)\n      EmuThreadStart();','   // Only retro_run may resume the guest after a snapshot.',2)
    with core.open('a',encoding='utf-8') as target:target.write('\n'+(ROOT/'engines/web/adapters/ppsspp-save-barrier.cpp.inc').read_text(encoding='utf-8'))
    # A single decoder thread prevents an automatic host-CPU-derived pool from
    # exhausting the fixed SDK worker budget during video/audio initialization.
    for relative in ['Core/HW/MediaEngine.cpp','Core/HW/SimpleAudioDec.cpp']:
        path=source/relative;text=path.read_text(encoding='utf-8')
        for variable in ['m_pCodecCtx','codecCtx_']:
            call='avcodec_open2('+variable+','
            if call in text:
                text=text.replace(call,'('+variable+'->thread_count = 1, '+variable+'->thread_type = 0, avcodec_open2('+variable+',')
                # Preserve all actual call arguments and add only the comma
                # expression's closing parenthesis at that statement boundary.
                lines=text.splitlines()
                for index,line in enumerate(lines):
                    if '->thread_count = 1' in line:
                        if not line.endswith(';'):raise ValueError('Pinned FFmpeg open statement changed')
                        lines[index]=line[:-1]+');'
                text='\n'.join(lines)+'\n'
        path.write_text(text,encoding='utf-8',newline='\n')
    make=source/'libretro/Makefile'
    replace(make,'WITH_DYNAREC := 1','WITH_DYNAREC := 0')
    replace(make,'-msimd128 -msse3 -mssse3 -msse4.1 -pthread -D__EMSCRIPTEN__ -DNO_MMAP','-msimd128 -msse3 -mssse3 -msse4.1 -pthread -fexceptions -D__EMSCRIPTEN__ -DNO_MMAP -fno-fast-math -ffp-contract=off')
    replace(make,'-DWITH_UPNP ','-DHTTPS_NOT_AVAILABLE ')
    replace(make,'ifneq ($(platform), emscripten)\n\t# ffmpeg does not work with emscripten\n\tCOREFLAGS += -DUSE_FFMPEG\nendif','COREFLAGS += -DUSE_FFMPEG')
    common=source/'libretro/Makefile.common';text=common.read_text(encoding='utf-8');start=text.index('INCFLAGS += -I$(EXTDIR)/miniupnp-build');end=text.index('\nSOURCES_CXX +=',start);text=text[:start]+text[end:]
    text=text.replace('COREFLAGS += -DVK_USE_PLATFORM_XLIB_KHR','')
    start=text.index('GIT_VERSION_SRC = ');text=text[:start]+'''GIT_VERSION_SRC = $(CORE_DIR)/git-version.cpp
SOURCES_CXX += $(GIT_VERSION_SRC)
'''
    text=text.replace('ifneq ($(STATIC_LINKING), 1)\nSOURCES_C += \\\n\t\t$(EXTDIR)/zlib/adler32.c','ifeq ($(STATIC_LINKING), 1)\nSOURCES_C += \\\n\t\t$(EXTDIR)/zlib/adler32.c')
    common.write_text(text,encoding='utf-8',newline='\n')
    decoder_limits=adapt_ffmpeg(source)
    (source/'git-version.cpp').write_text('const char *PPSSPP_GIT_VERSION = "'+VERSION+'";\n#define PPSSPP_GIT_VERSION_NO_UPDATE 1\n',encoding='utf-8')
    # The selected SDK protocol uses integer commands 1..9. Its source identity
    # is retained alongside the exact root/dependency source, not inferred from
    # a documentation version or from a different installed compiler.
    adapters=source/'omniplay';adapters.mkdir()
    for name in ADAPTERS:shutil.copy2(ROOT/'engines/web/adapters'/name,adapters/name)
    shutil.copy2(ROOT/'engines/web/frontend.c',adapters/'frontend.c')
    exports=json.loads((ROOT/'engines/web/exports.json').read_text(encoding='utf-8'))
    exports+=['_om_web_libretro_call','_om_web_log_interface','_om_web_flush_save_io','_om_web_thread_abi','_om_web_thread_callback_pointer']
    (work/'exports.json').write_text(json.dumps(sorted(set(exports)))+'\n',encoding='utf-8')
    shutil.copy2(ROOT/'engines/web/runtime-methods.json',work/'runtime-methods.json')
    policy=ROOT/'engines/policies/ppsspp-web-options.json';validate_policy(json.loads(policy.read_text(encoding='utf-8')));shutil.copy2(policy,work/'core-options-policy.json')
    shutil.copy2(lock_path,work/'ppsspp-source.lock.json');shutil.copy2(sdk_path,work/'emscripten-source.lock.json')
    shutil.copytree(ROOT/'engines/web',work/'omniplay-web-runtime')
    for name in RECIPES:shutil.copy2(ROOT/'tools'/name,work/name)
    record={'schema_version':1,'core':'ppsspp-web','revision':PIN,'version':VERSION,'sdk_revision':SDK,
        'source_only':True,'verification':'not_run','distribution_approved':False,'selected_committed_paths':SELECTED,
        'source_archive_sha256':archives,'decoder_source':decoder_source,'decoder_limits':decoder_limits,'ffmpeg_revision':FFMPEG_PIN,'sdk_source_materialization':'git_show_exact_committed_files_v1','sdk_contract_sha256':sdk_hashes,'prepared_source_sha256':source_inventory(source),
        'web_runtime_source_sha256':source_inventory(work/'omniplay-web-runtime'),
        'recipe_sha256':{name:file_hash(work/name) for name in RECIPES},
        'auxiliary_sha256':{name:file_hash(work/name) for name in ('exports.json','runtime-methods.json','core-options-policy.json','ppsspp-source.lock.json','emscripten-source.lock.json')},
        'execution':{'cpu':'ir_interpreter','gpu':'software','internal_resolution':1,'pool_workers':16,'compute_workers':1,'io_workers':2,'native_jit':False,'network':False},
        'remaining':['pinned SDK/LLVM/Binaryen/cache binaries and all compilation','multimedia decoder/configuration and all compiled notices',
            'open-font generated atlas and exact PGF provenance','thread/FS/proxy/barrier/boot/state/fault semantics and containment',
            'actual per-device performance/memory/battery and all mobile/GPU/full-product scope','source/SBOM/license/store/security/release approval']}
    (work/'corresponding-source.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')


def build(args,work):
    record=json.loads((work/'corresponding-source.json').read_text(encoding='utf-8'));source=work/'source'
    if record.get('revision')!=PIN or record.get('version')!=VERSION or record.get('sdk_revision')!=SDK or record.get('source_only') is not True or source_inventory(source)!=record['prepared_source_sha256']:raise ValueError('Prepared PSP source differs from its immutable inventory')
    for name,expected in record['recipe_sha256'].items():
        if file_hash(work/name)!=expected or file_hash(ROOT/'tools'/name)!=expected:raise ValueError('The PSP producer changed after preparation')
    for name,expected in record['auxiliary_sha256'].items():
        if file_hash(work/name)!=expected:raise ValueError('Prepared PSP build policy changed')
    if source_inventory(ROOT/'engines/web')!=record['web_runtime_source_sha256'] or source_inventory(work/'omniplay-web-runtime')!=record['web_runtime_source_sha256']:raise ValueError('The shared Web runtime changed after preparation')
    sdk=args.emcc.parent
    for name,expected in record['sdk_contract_sha256'].items():
        if file_hash(sdk/name)!=expected:raise ValueError('The selected compiler does not have the pinned SDK source contract')
    if subprocess.check_output([args.git,'-C',str(sdk),'rev-parse','HEAD'],text=True).strip()!=SDK:raise ValueError('The compiler must belong to the exact selected SDK source checkout')
    atlas=admit_atlas(args.atlas_receipt,PIN,ATLAS_FONT_PINS)
    build_dir=owned(work/'build')
    if build_dir.exists():raise ValueError('Preserve prior build source and choose a fresh prepared output')
    build_source=build_dir/'source';shutil.copytree(source,build_source)
    for item in atlas['assets']:
        asset=Path(item['source']).resolve(strict=True)
        if asset.is_symlink() or not asset.is_file() or not 0<item['bytes']<=32*1024**2 or asset.stat().st_size!=item['bytes'] or file_hash(asset)!=item['sha256']:raise ValueError('The generated atlas differs from its receipt')
        shutil.copy2(asset,build_source/'assets'/item['name'])
    shutil.copy2(args.atlas_receipt,build_dir/'atlas-build-receipt.json')
    decoder_receipt,decoder_prefix=admit_ffmpeg_receipt(args.ffmpeg_receipt,'web')
    ffmpeg_include=decoder_prefix/'include'
    archive=build_dir/'ppsspp.a'
    command=[args.make,'-C',build_source/'libretro','-j'+str(args.parallel),'platform=emscripten','TARGET_ARCH=wasm32','WITH_DYNAREC=0','HAVE_LIBRETRO_VFS=0',
        'CC='+str(args.emcc),'CXX='+str(args.emxx),'AR='+str(args.emar),'TARGET='+str(archive),'FFMPEGINCFLAGS=-I'+str(ffmpeg_include)]
    subprocess.run([str(value) for value in command],check=True)
    output=build_dir/'product';output.mkdir();module=output/'ppsspp.js'
    shim_objects=[]
    for name in ['frontend.c','threaded-frontend.c','libretro-results.c']:
        target=build_dir/(name+'.o')
        subprocess.run([str(args.emcc),'-O3','-pthread','-fexceptions','-DOMNIPLAY_WEB_FILES=1','-I'+str(build_source/'libretro/libretro-common/include'),'-c',str(build_source/'omniplay'/name),'-o',str(target)],check=True);shim_objects.append(target)
    link=[args.emxx,*FLAGS,archive,*shim_objects,
        *[Path(decoder_receipt['libraries'][name]) for name in ('avformat','avcodec','swresample','swscale','avutil')],
        '--pre-js',build_source/'omniplay/ppsspp-thread-pre.js','-sEXPORTED_FUNCTIONS=@'+str(work/'exports.json'),'-sEXPORTED_RUNTIME_METHODS=@'+str(work/'runtime-methods.json'),'-o',module]
    subprocess.run([str(value) for value in link],check=True)
    wasm=output/'ppsspp.wasm'
    if not wasm.is_file() or wasm.stat().st_size>512*1024**2 or not module.is_file() or module.stat().st_size>32*1024**2:raise ValueError('The PSP compiler did not emit bounded module/Wasm resources')
    assets=[]
    for relative in ['compat.ini','ppge_atlas.zim','ppge_atlas.meta',*['flash0/font/'+path.name for path in sorted((build_source/'assets/flash0/font').glob('*.pgf'))]]:
        original=build_source/'assets'/relative;target=output/'assets'/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(original,target)
        assets.append({'path':'PPSSPP/'+relative,'resource':'cores/ppsspp/assets/'+relative,'bytes':target.stat().st_size,'sha256':file_hash(target)})
    manifest={'schema_version':1,'id':'ppsspp-web','core_version':VERSION,'runtime_version_string':VERSION,'source_revision':PIN,'systems':['psp'],
        'module':'cores/ppsspp/ppsspp.js','wasm':'cores/ppsspp/ppsspp.wasm','wasm_bytes':wasm.stat().st_size,'wasm_sha256':file_hash(wasm),
        'license_expression':'GPL-2.0-or-later','state_format_version':'ppsspp-websw3-state','in_game_format_version':'ppsspp-websw1-files',
        'minimum_memory_mb':2048,'minimum_system_ram_bytes':33554432,'capabilities':['pause','gamepad','keyboard_mouse','save_files','save_state'],
        'content_formats':['iso','cso','pbp','chd'],'media_filesystem_protocol':1,'file_backed_content_required':True,'wasm_threads':True,
        'saved_files_protocol':1,'saved_files_io':'pthread_joined_barrier_v1','libretro_result_protocol':2,'thread_callbacks_protocol':1,
        'thread_pool':{'schema_version':1,'api':'emscripten_pthreads_ui_v1','maximum_workers':16,'maximum_memory_bytes':1073741824,'worker_module':'cores/ppsspp/ppsspp.js','worker_type':'module'},
        'pthread_worker_bytes':module.stat().st_size,'pthread_worker_sha256':file_hash(module),'system_assets_protocol':1,'system_assets':assets,
        'core_options_protocol':1,'core_option_definitions':[],'distribution_approved':False,'verification':'not_run'}
    from web_thread_policy import admit as admit_threads
    from web_system_assets_policy import admit as admit_assets
    admit_threads(manifest);admit_assets(manifest)
    notices=[path for path in build_source.rglob('*') if path.is_file() and path.name.lower().startswith(('license','copying','copyright')) and path.stat().st_size<=4*1024**2]
    for index,path in enumerate(sorted(notices)):shutil.copy2(path,output/('LICENSE.component-'+str(index)+'.txt'))
    shutil.copy2(work/'sdk-source/source/LICENSE',output/'LICENSE.emscripten.txt')
    for index,name in enumerate(atlas.get('additional_notices',[])):
        path=Path(name).resolve(strict=True)
        if not path.is_file() or path.stat().st_size>4*1024**2:raise ValueError('The open-font notice is absent or oversized')
        shutil.copy2(path,output/('LICENSE.atlas-'+str(index)+'.txt'))
    (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    (work/'build-receipt.json').write_text(json.dumps(dict(record,source_only=False,module_sha256=file_hash(module),wasm_sha256=file_hash(wasm),
        ffmpeg_revision=FFMPEG_PIN,ffmpeg_receipt_sha256=file_hash(args.ffmpeg_receipt),ffmpeg_build_receipt=decoder_receipt,atlas_receipt_sha256=file_hash(args.atlas_receipt),atlas_source_policy=atlas['source_policy'],atlas_source_evidence_sha256=atlas['source_evidence']['sha256'],product_directory=str(output.resolve()),compiled_source_sha256=source_inventory(build_source)),indent=2,default=str)+'\n',encoding='utf-8')


def main():
    parser=argparse.ArgumentParser(description=__doc__);mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--prepare-only',action='store_true');mode.add_argument('--build-prepared',action='store_true')
    parser.add_argument('--git',default='git');parser.add_argument('--make',type=Path);parser.add_argument('--shell',type=Path);parser.add_argument('--emcc',type=Path);parser.add_argument('--emxx',type=Path);parser.add_argument('--emar',type=Path);parser.add_argument('--emranlib',type=Path);parser.add_argument('--emnm',type=Path);parser.add_argument('--atlas-receipt',type=Path);parser.add_argument('--ffmpeg-receipt',type=Path);parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32:raise ValueError('Build parallelism exceeds its bound')
    work=owned(ROOT/'out/web-cores/ppsspp-v3-committed-atlas')
    if args.prepare_only:
        prepare(work,args.git);print('Authored pinned PSP pthread/software source, SDK contracts and explicit build recipe. No configure, compiler, runtime or test was started.');return
    if any(path is None or not path.is_file() for path in (args.make,args.shell,args.emcc,args.emxx,args.emar,args.emranlib,args.emnm,args.atlas_receipt,args.ffmpeg_receipt)):raise ValueError('Supply explicit real toolchain and generated-atlas receipt paths')
    for name in ('make','shell','emcc','emxx','emar','emranlib','emnm','atlas_receipt','ffmpeg_receipt'):setattr(args,name,getattr(args,name).resolve(strict=True))
    build(args,work);print('Built an unapproved PSP Web candidate. No browser, game, device or test was run.')


if __name__=='__main__':main()
