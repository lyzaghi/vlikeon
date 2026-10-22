"""Explicit pinned offline NDS software-core build. Reserved until final authoring.

No build is performed by source assembly. The actual invocation requires all
locked dependency checkouts, an explicit compiler/toolchain and a fresh output.
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
PIN='f394adbacb5722ee97c1b37c8064da9a25818310'
def replace(path,old,new):
    text=path.read_text(encoding='utf-8')
    if text.count(old)!=1:raise ValueError('Pinned NDS adaptation anchor changed: '+path.name)
    path.write_text(text.replace(old,new,1),encoding='utf-8',newline='\n')
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cmake',type=Path);parser.add_argument('--generator',default='Ninja')
    parser.add_argument('--prepare-only',action='store_true',help='Author adapted corresponding source only, without configure, compilation or checks')
    parser.add_argument('--cc',type=Path);parser.add_argument('--cxx',type=Path);parser.add_argument('--toolset');parser.add_argument('--toolchain-file',type=Path)
    parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32:raise ValueError('Build parallelism exceeds its bound')
    if not args.prepare_only and not args.cmake:raise ValueError('An explicit CMake executable is required for a build')
    lock=json.loads((ROOT/'security/melonds-source.lock.json').read_text(encoding='utf-8'))
    if lock['schema_version']!=1 or lock['core']['revision']!=PIN:raise ValueError('NDS source lock differs from the core pin')
    original=ROOT/'third_party/sources/melonds-ds'
    if subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()!=PIN:raise ValueError('NDS checkout differs from its pin')
    dependencies={}
    for entry in lock['dependencies']:
        if not re.fullmatch('[A-Za-z0-9_-]{1,64}',entry['name']) or not re.fullmatch('[a-f0-9]{40}',entry['revision']):raise ValueError('NDS dependency identity is invalid')
        path=ROOT/'third_party/sources/melonds-deps'/entry['name']
        if subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()!=entry['revision']:raise ValueError('NDS dependency differs from its pin: '+entry['name'])
        dependencies[entry['name']]=path
    output=ROOT/('out/native-sources/melonds-ds' if args.prepare_only else 'out/native-cores/melonds-ds')
    if output.exists():raise ValueError('Use a fresh NDS output directory to preserve the prior corresponding source')
    source=output/'source';shutil.copytree(original,source,ignore=shutil.ignore_patterns('.git'))
    local={}
    for name,path in dependencies.items():
        destination=output/'dependencies'/name;shutil.copytree(path,destination,ignore=shutil.ignore_patterns('.git'));local[name]=destination
    # Upstream fetches/builds libslirp even with the networking option disabled.
    # Omit the unused backend instead of packaging a network stack in this core.
    replace(source/'cmake/FetchDependencies.cmake','fetch_dependency(libslirp "https://github.com/JesseTG/libslirp-mirror" "e61dbd4")','')
    replace(source/'cmake/FetchDependencies.cmake','set(CMAKE_MODULE_PATH "${FETCHCONTENT_BASE_DIR}/melonds-src/cmake" "${FETCHCONTENT_BASE_DIR}/embed-binaries-src/cmake" "${CMAKE_MODULE_PATH}")','set(CMAKE_MODULE_PATH "${FETCHCONTENT_SOURCE_DIR_MELONDS}/cmake" "${FETCHCONTENT_SOURCE_DIR_EMBED-BINARIES}/cmake" "${CMAKE_MODULE_PATH}")')
    replace(source/'cmake/FetchDependencies.cmake','FetchContent_MakeAvailable(melonDS libretro-common embed-binaries glm zlib libslirp pntr fmt yamc span-lite date)','FetchContent_MakeAvailable(melonDS libretro-common embed-binaries glm zlib pntr fmt yamc span-lite date)')
    replace(source/'cmake/ConfigureDependencies.cmake','include(cmake/libslirp.cmake)','')
    replace(source/'cmake/GenerateAttributions.cmake','file(READ "${libslirp_SOURCE_DIR}/COPYRIGHT" SLIRP_LICENSE)','')
    replace(source/'cmake/melondsds-LICENSE.txt.in','# Slirp ################################################################################################################\n\n${SLIRP_LICENSE}\n\n','')
    replace(source/'src/libretro/config/console.cpp','#include <string_view>','#include <string_view>\n#include <stdexcept>')
    replace(source/'src/libretro/config/console.cpp','retro::warn("Falling back to built-in firmware");','throw std::runtime_error("User-selected DS firmware could not be loaded");')
    replace(source/'src/libretro/config/console.cpp','retro::warn("Falling back to FreeBIOS");','throw std::runtime_error("User-selected DS BIOS files could not be loaded");')
    (source/'OMNIPLAY_ADAPTATIONS.json').write_text(json.dumps({'schema_version':1,'revision':PIN,'changes':['offline_dependency_source_overrides','no_jit','software_renderer','no_networking_backend','no_tracy','system_files_from_native_user_vault','native_system_file_failure_is_not_a_silent_builtin_fallback'],'upstream_original_freebios':'BSD-2-Clause original replacement; attribution retained; not manufacturer firmware'},indent=2)+'\n',encoding='utf-8')
    if args.prepare_only:
        (output/'source-preparation.json').write_text(json.dumps({'schema_version':1,'core_revision':PIN,'source_lock':lock,'corresponding_source':str(source.resolve()),'dependency_sources':{name:str(path.resolve()) for name,path in local.items()},'configure':'not_started','build':'not_started','verification':'not_run'},indent=2)+'\n',encoding='utf-8')
        shutil.copy2(Path(__file__),output/'build_melonds.py')
        print('Authored adapted NDS corresponding source and pinned local dependency trees. No configure, compilation or test was started.')
        return
    build=output/'build'
    command=[args.cmake,'-S',source,'-B',build,'-G',args.generator,'-DCMAKE_BUILD_TYPE=Release','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DBUILD_AS_SHARED_LIBRARY=ON','-DBUILD_SHARED_LIBS=OFF','-DFETCHCONTENT_FULLY_DISCONNECTED=ON','-DFETCHCONTENT_UPDATES_DISCONNECTED=ON','-DENABLE_JIT=OFF','-DENABLE_OPENGL=OFF','-DENABLE_COMPUTE_RENDERER=OFF','-DENABLE_OGLRENDERER=OFF','-DENABLE_NETWORKING=OFF','-DENABLE_DYNAMIC=OFF','-DENABLE_GDBSTUB=OFF','-DENABLE_THREADED_RENDERER=ON','-DENABLE_LTO=OFF','-DENABLE_LTO_RELEASE=OFF','-DBUILD_TESTING=OFF','-DENABLE_TESTING=OFF','-DTRACY_ENABLE=OFF','-DBUILD_QT_SDL=OFF']
    if args.cc:command.append('-DCMAKE_C_COMPILER='+str(args.cc))
    if args.cxx:command.append('-DCMAKE_CXX_COMPILER='+str(args.cxx))
    if args.toolset:command+=['-T',args.toolset]
    if args.toolchain_file:command.append('-DCMAKE_TOOLCHAIN_FILE='+str(args.toolchain_file))
    for name,path in local.items():
        # FetchContent's source override uses the upper-case original dependency
        # name (including hyphens), distinct from upstream's C identifier alias.
        command.append('-DFETCHCONTENT_SOURCE_DIR_'+name.upper()+'='+str(path))
        key=re.sub('[^A-Z0-9_]','_',name.upper());entry=next(item for item in lock['dependencies'] if item['name']==name)
        command.append('-D'+key+'_REPOSITORY_TAG='+entry['revision'])
    subprocess.run([str(v) for v in command],check=True)
    subprocess.run([str(args.cmake),'--build',str(build),'--config','Release','--target','melondsds_libretro','--parallel',str(args.parallel)],check=True)
    binaries=[p for p in build.rglob('*melondsds_libretro*') if p.suffix in {'.dll','.so','.dylib'}]
    if len(binaries)!=1:raise ValueError('NDS build did not produce one unambiguous core library')
    attribution=list(build.rglob('melondsds-LICENSE.txt'))
    if len(attribution)!=1:raise ValueError('The generated NDS dependency attribution is missing or ambiguous')
    receipt={'schema_version':1,'core':'melonds-ds','revision':PIN,'library':str(binaries[0].resolve()),'corresponding_source':str(source.resolve()),'dependency_sources':{name:str(path.resolve()) for name,path in local.items()},'source_lock':lock,'additional_notices':[str(attribution[0].resolve())],'verification':'not_run','firmware':[{'role':role,'filename':filename,'allowed_bytes':sizes} for role,filename,sizes in [('nds_bios7','bios7.bin',[16384]),('nds_bios9','bios9.bin',[4096]),('nds_firmware','firmware.bin',[131072,262144,524288])]]}
    (output/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    shutil.copy2(Path(__file__),output/'build_melonds.py')
    print('NDS software library compiled; compatibility, touch, save and containment verification are separate.')
if __name__=='__main__':main()
