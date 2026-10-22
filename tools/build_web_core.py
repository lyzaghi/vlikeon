"""Build pinned NES/GBA cores for the shared isolated web runtime.

Builds are explicit. This file does not run games or tests. Original checkouts
remain pristine; any build adaptation is retained in the corresponding-source
directory alongside the exact upstream source used for distribution.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
CORES={
    "fceumm":{
        "revision":"ac0d00a7fc01166f84c7bac01d1084ebfd2b865e",
        "repository":"https://github.com/libretro/libretro-fceumm.git",
        "core_version":"0.0.0+ac0d00a7fc01","runtime_version_string":"(SVN)",
        "systems":["nes"],"license":"GPL-2.0-or-later","notice":"COPYING",
    },
    "mgba":{
        "revision":"fd9dc5c2cbf234365068855851107e762fc6211b",
        "repository":"https://github.com/mgba-emu/mgba.git",
        "core_version":"0.11.0","runtime_version_string":"0.11.0",
        "systems":["gba"],"license":"MPL-2.0","notice":"LICENSE",
    },
}

def run(arguments,cwd=None):
    subprocess.run([str(value) for value in arguments],cwd=cwd,check=True)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("core",choices=CORES)
    parser.add_argument("--emcc",required=True,type=Path)
    parser.add_argument("--emxx",required=True,type=Path)
    parser.add_argument("--emar",required=True,type=Path)
    parser.add_argument("--emcmake",type=Path)
    parser.add_argument("--make",default="make")
    parser.add_argument("--cmake",default="cmake")
    parser.add_argument("--git",default="git")
    parser.add_argument("--distribution-approved",action="store_true",help="Record an explicit distribution decision after reviewing the core, dependencies and corresponding source")
    args=parser.parse_args()
    specification=CORES[args.core]
    original=ROOT/"third_party/sources"/args.core
    revision=subprocess.check_output([args.git,"-C",str(original),"rev-parse","HEAD"],text=True).strip()
    if revision!=specification["revision"]:
        raise SystemExit("Core checkout does not match its pinned source revision")
    work=ROOT/"out/web-cores"/args.core
    source=work/"source"
    if source.exists():
        raise SystemExit("Use a new output directory for a reproducible build; preserve the prior corresponding source")
    shutil.copytree(original,source,ignore=shutil.ignore_patterns(".git","build","*.o","*.bc"))
    adaptation=[]
    archive=work/f"{args.core}.a"
    if args.core=="fceumm":
        run([args.make,"-f","Makefile.libretro","platform=emscripten",f"CC={args.emcc}",f"CXX={args.emxx}",f"AR={args.emar}",f"TARGET={archive}",'GIT_VERSION=" unknown"',"HAVE_HDPACK=0","HAVE_NTSC=1","-j4"],source)
    else:
        if not args.emcmake:
            raise SystemExit("The GBA core needs --emcmake")
        cmake=source/"CMakeLists.txt"
        text=cmake.read_text(encoding="utf-8")
        anchor="add_library(${BINARY_NAME}_libretro SHARED ${CORE_SRC} ${RETRO_SRC} ${CORE_VFS_SRC})"
        if text.count(anchor)!=1:
            raise SystemExit("The pinned GBA libretro target changed")
        cmake.write_text(text.replace(anchor,anchor.replace(" SHARED "," STATIC ")),encoding="utf-8")
        adaptation.append("Build the libretro target as a static archive for final Emscripten linking; emulator logic is unchanged")
        build=work/"build"
        options=["BUILD_QT","BUILD_SDL","BUILD_TEST","BUILD_SUITE","BUILD_CINEMA","BUILD_HEADLESS","BUILD_EXAMPLE","BUILD_PYTHON","BUILD_SHARED","BUILD_GL","BUILD_GLES2","BUILD_GLES3","ENABLE_DEBUGGERS","ENABLE_GDB_STUB","ENABLE_SCRIPTING","USE_FFMPEG","USE_ZLIB","USE_MINIZIP","USE_PNG","USE_LIBZIP","USE_SQLITE3","USE_ELF","USE_LUA","USE_JSON_C","USE_FREETYPE","USE_LZMA","USE_DISCORD_RPC","M_CORE_GB"]
        run([args.emcmake,args.cmake,"-S",source,"-B",build,"-DCMAKE_BUILD_TYPE=Release","-DBUILD_LIBRETRO=ON","-DSKIP_LIBRARY=ON","-DSKIP_GIT=ON","-DDISABLE_DEPS=ON","-DM_CORE_GBA=ON"]+[f"-D{option}=OFF" for option in options],source)
        run([args.cmake,"--build",build,"--target","mgba_libretro","--parallel","4"],source)
        candidates=list(build.rglob("*mgba_libretro*.a"))
        if len(candidates)!=1:
            raise SystemExit("The GBA build did not produce one unambiguous static archive")
        shutil.copy2(candidates[0],archive)
    output=ROOT/"apps/game-hub/cores"/args.core
    output.mkdir(parents=True,exist_ok=True)
    frontend=source/"omniplay-frontend.c";shutil.copy2(ROOT/"engines/web/frontend.c",frontend)
    adaptation.append("Link the retained OmniPlay filesystem constants frontend; emulator logic is unchanged")
    run([args.emcc,"-O3",archive,frontend,"--no-entry","-sMODULARIZE=1","-sEXPORT_ES6=1","-sENVIRONMENT=worker","-sALLOW_MEMORY_GROWTH=1","-sMAXIMUM_MEMORY=1073741824","-sALLOW_TABLE_GROWTH=1","-sFILESYSTEM=1","-sEXPORTED_FUNCTIONS=@"+str(ROOT/"engines/web/exports.json"),"-sEXPORTED_RUNTIME_METHODS=@"+str(ROOT/"engines/web/runtime-methods.json"),"-o",output/f"{args.core}.js"],source)
    wasm=output/f"{args.core}.wasm"
    prefix=f"cores/{args.core}/{args.core}"
    manifest={
        "schema_version":1,"id":f"{args.core}-web","core_version":specification["core_version"],
        "runtime_version_string":specification["runtime_version_string"],"source_revision":revision,"systems":specification["systems"],
        "module":prefix+".js","wasm":prefix+".wasm","wasm_bytes":wasm.stat().st_size,"wasm_sha256":hashlib.sha256(wasm.read_bytes()).hexdigest(),
        "license_expression":specification["license"],"state_format_version":f"{args.core}-{revision[:16]}","in_game_format_version":f"{args.core}-memory-envelope-v1",
        "capabilities":["pause","gamepad","haptics","keyboard_mouse","touch_controls","save_ram","save_state"],
        "content_formats":["raw_rom"],"media_filesystem_protocol":1,"frontend_source_sha256":hashlib.sha256(frontend.read_bytes()).hexdigest(),
        "firmware_protocol":1,"firmware":[],
        "distribution_approved":args.distribution_approved,"verification":"not_run",
    }
    shutil.copy2(source/specification["notice"],output/f"LICENSE.{args.core}.txt")
    (output/"manifest.json").write_text(json.dumps(manifest,indent=2)+"\n",encoding="utf-8")
    provenance={"repository":specification["repository"],"revision":revision,"build_adaptations":adaptation,"license":specification["license"],"upstream_source_included":True,"test_state":"not_run"}
    (work/"corresponding-source.json").write_text(json.dumps(provenance,indent=2)+"\n",encoding="utf-8")
    shutil.copy2(Path(__file__),work/"build_web_core.py")
    shutil.copytree(ROOT/"engines/web",work/"omniplay-web-runtime",dirs_exist_ok=True)

if __name__=="__main__":
    main()
