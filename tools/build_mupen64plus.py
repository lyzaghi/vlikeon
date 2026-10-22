"""Author/build the pinned N64 software/interpreter core. No automatic execution.

Source preparation is separate from the later compiler and real-device pass.
The baseline keeps Chromium/native dynamic-code and graphics policies intact.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
PIN='12edd2c74a517ff86dfa8cfc71ad75e4c10486d5'
VERSION='3.0.0+omniplay.sw1.12edd2c74a51'
GL_VERSION='3.0.0+omniplay.gl1.12edd2c74a51'
def replace(path,old,new,count=1):
    text=path.read_text(encoding='utf-8')
    if text.count(old)!=count:raise ValueError('Pinned N64 adaptation anchor changed: '+path.as_posix())
    path.write_text(text.replace(old,new),encoding='utf-8',newline='\n')
def prepare(original,source,profile='software',materialized=False):
    if profile not in {'software','opengl'}:raise ValueError('Select an explicit N64 source profile')
    gpu=profile=='opengl';version=GL_VERSION if gpu else VERSION
    excluded=['mupen64plus-video-angrylion','mupen64plus-rsp-cxd4','mupen64plus-rsp-paraLLEl','mupen64plus-video-paraLLEl'] if gpu else []
    if materialized:
        if original!=source or not source.is_dir() or any((source/name).exists()for name in excluded):raise ValueError('Prepared native N64 profile must already have its exact excluded source selection')
    else:
        shutil.copytree(original,source,ignore=shutil.ignore_patterns('.git','*.o','*.a','*.dll','*.exe','*.rom','*.bin','*.z64','*.n64','*.v64','*.ndd',*excluded))
    replace(source/'libretro-common/libco/libco.c','#if defined _MSC_VER','#if defined(_WIN32)\n#include "fiber.c"\n#elif defined _MSC_VER')
    fiber=source/'libretro-common/libco/fiber.c'
    replace(fiber,'#define WINVER 0x0400','#ifndef WINVER\n#define WINVER 0x0A00\n#endif')
    replace(fiber,'#define _WIN32_WINNT 0x0400','#ifndef _WIN32_WINNT\n#define _WIN32_WINNT 0x0A00\n#endif')
    replace(fiber,'ConvertThreadToFiber(0);','ConvertThreadToFiberEx(0, FIBER_FLAG_FLOAT_SWITCH);',count=2)
    replace(fiber,'return (cothread_t)CreateFiber(heapsize, co_thunk, (void*)coentry);','return (cothread_t)CreateFiberEx(heapsize, heapsize, FIBER_FLAG_FLOAT_SWITCH, co_thunk, (void*)coentry);')
    core=source/'libretro/libretro.c'
    replace(core,'info->library_version = "3.0" FLAVOUR_VERSION GIT_VERSION;','info->library_version = "'+version+'";')
    replace(core,'CoreStartup(FRONTEND_API_VERSION, ".", ".", NULL, n64DebugCallback, 0, n64StateCallback)','CoreStartup(FRONTEND_API_VERSION, "scratch", "scratch", NULL, n64DebugCallback, 0, n64StateCallback)')
    replace(core,'    if(ret && log_cb)','    if (ret) environ_cb(RETRO_ENVIRONMENT_SHUTDOWN, NULL);\n    if(ret && log_cb)')
    # Each signed package fixes its renderer/RSP; variables cannot switch to an
    # excluded backend or create a second graphics thread behind the frontend.
    rdp='RDP_PLUGIN_GLIDEN64' if gpu else 'RDP_PLUGIN_ANGRYLION';rsp='RSP_PLUGIN_HLE' if gpu else 'RSP_PLUGIN_CXD4'
    bindings=f'\n    plugin_connect_rdp_api({rdp});\n    plugin_connect_rsp_api({rsp});\n    EnableThreadedRenderer = 0;'
    replace(core,'    environ_cb(RETRO_ENVIRONMENT_GET_RUMBLE_INTERFACE, &rumble);','    environ_cb(RETRO_ENVIRONMENT_GET_RUMBLE_INTERFACE, &rumble);'+bindings)
    replace(core,'    update_variables(true);','    update_variables(true);'+bindings)
    replace(core,'    CoreDoCommand(M64CMD_RESET, 0, (void*)0);','    if (!initializing) CoreDoCommand(M64CMD_RESET, 0, (void*)0);')
    # These APIs may be used while the host is restoring progress before Start.
    # Queueing a job lets the core's own coroutine initialize before servicing it.
    # The frontend retains an exact before-state and rolls back rejected loads.
    before='''   if (initializing)
      return false;

   retro_savestate_complete = false;'''
    after='''   if (!load_game_successful || data == NULL || size != retro_serialize_size())
      return false;

   retro_savestate_complete = false;'''
    replace(core,before,after,count=2)
    raw=core.read_text(encoding='utf-8')
    start=raw.index('bool retro_unserialize(const void *data, size_t size)')
    end=raw.index('//Needed to be able to detach controllers',start)
    load=raw[start:end]
    if load.count('   return true;')!=1:raise ValueError('N64 load-result declaration changed')
    raw=raw[:start]+load.replace('   return true;','   return !!retro_savestate_result;')+raw[end:]
    core.write_text(raw,encoding='utf-8',newline='\n')
    # Cached instruction storage never needs executable permission in this build.
    recomp=source/'mupen64plus-core/src/device/r4300/recomp.c'
    replace(recomp,'MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE','MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE')
    replace(recomp,'PROT_READ | PROT_WRITE | PROT_EXEC','PROT_READ | PROT_WRITE')
    # Preserve the full build inputs; do not resolve compiler runtimes via PATH in
    # the installed contained process. There is no emulator process launch here.
    makefile=source/'Makefile'
    with makefile.open('a',encoding='utf-8') as output:output.write('\n# OmniPlay standalone runtime and deterministic CPU math.\nLDFLAGS += -static -static-libgcc -static-libstdc++\noverride CFLAGS += -fno-fast-math -fno-unsafe-math-optimizations -ffp-contract=off\noverride CXXFLAGS += -fno-fast-math -fno-unsafe-math-optimizations -ffp-contract=off\n')
    changes=(['opengl_core_gliden64_rdp','hle_rsp','cached_interpreter_cpu_configuration','restricted_angrylion_and_cxd4_source_excluded'] if gpu else ['software_angrylion_rdp','interpreter_cxd4_rsp','pure_interpreter_cpu_configuration'])+['no_dynarec_or_parallel_backends','windows_fibers_with_float_state_without_executable_heap','private_scratch_configuration','non_executable_cached_instruction_storage','bounded_exact_size_snapshot_input','truthful_snapshot_load_result','pre_start_snapshot_job_initialization','static_windows_compiler_runtime','cpu_fast_math_disabled']
    (source/'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version':1,'upstream_revision':PIN,'core_version':version,'profile':profile,'changes':changes,'excluded_source_directories':excluded,'initial_platform':'windows-x86_64','compiled':False,'verification':'not_run','distribution_review':'compiled_licenses_and_corresponding_source_pending' if gpu else 'research_only_not_release_eligible'},indent=2)+'\n',encoding='utf-8')
    return changes
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare-only',action='store_true')
    for name in ['make','cc','cxx','ar']:parser.add_argument('--'+name,type=Path)
    parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32 or not args.prepare_only and any(getattr(args,name) is None for name in ['make','cc','cxx','ar']):raise ValueError('Select bounded parallelism and explicit compiler tools')
    original=ROOT/'third_party/sources/mupen64plus-next'
    if subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()!=PIN:raise ValueError('N64 source differs from the immutable pin')
    output=ROOT/('out/native-sources/mupen64plus-next' if args.prepare_only else 'out/native-cores/mupen64plus-next')
    if output.exists():raise ValueError('Use a fresh N64 output directory to preserve prior corresponding source')
    source=output/'source';changes=prepare(original,source);shutil.copy2(Path(__file__),output/'build_mupen64plus.py')
    if args.prepare_only:
        (output/'source-preparation.json').write_text(json.dumps({'schema_version':1,'core':'mupen64plus-next','revision':PIN,'corresponding_source':str(source.resolve()),'adaptations':changes,'configure':'not_started','build':'not_started','verification':'not_run'},indent=2)+'\n',encoding='utf-8')
        print('Authored adapted N64 source and explicit software/interpreter recipe. No compiler, runtime or test was started.');return
    command=[args.make,'-f','Makefile','platform=win','ARCH=x86_64','MSYSTEM=MINGW64','WITH_DYNAREC=','HAVE_THR_AL=1','LLE=1','HAVE_PARALLEL_RSP=0','HAVE_PARALLEL_RDP=0','HAVE_NEON=0','HAVE_LTCG=0','SYSTEM_ZLIB=0','SYSTEM_MINIZIP=0','SYSTEM_LIBPNG=0','SYSTEM_XXHASH=0','DEBUG=0','GIT_VERSION=" unknown"','CC='+str(args.cc),'CXX='+str(args.cxx),'AR='+str(args.ar),'-j'+str(args.parallel)]
    subprocess.run([str(v) for v in command],cwd=source,check=True)
    library=source/'mupen64plus_next_libretro.dll'
    if not library.is_file():raise ValueError('N64 build did not produce its exact declared core library')
    notices=[source/'mupen64plus-core/LICENSES',source/'mupen64plus-core/doc/gpl-license',source/'mupen64plus-core/doc/lgpl-license',source/'mupen64plus-rsp-cxd4/COPYING',source/'mupen64plus-rsp-hle/LICENSES',source/'GLideN64/LICENSE']
    receipt={'schema_version':1,'core':'mupen64plus-next','revision':PIN,'library':str(library.resolve()),'corresponding_source':str(source.resolve()),'additional_notices':[str(path.resolve()) for path in notices],'verification':'not_run','firmware':[],'distribution_review_required':['angrylion_original_license_provenance','compiled_dependency_inventory','corresponding_source_completeness']}
    (output/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print('N64 software library compiled. Gameplay, save, timing and containment verification remain separate.')
if __name__=='__main__':main()
