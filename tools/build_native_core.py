"""Build pinned native software cores; explicit build, no gameplay or tests.

The bsnes adaptation removes its embedded 64-byte manufacturer IPL image. A user
must import a permitted firmware file into the private firmware vault. The Windows
fiber backend keeps executable-heap trampolines out of the contained worker.
"""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PINS = {"mgba": "fd9dc5c2cbf234365068855851107e762fc6211b",
        "bsnes": "63ee22f7af8f5d1ada295c2c98fd7bf261fa62a7"}


def run(command, cwd=None):
    subprocess.run([str(value) for value in command], cwd=cwd, check=True)


def adapt_bsnes(source):
    resources = source / "bsnes/target-libretro/resources.hpp"
    raw = resources.read_text(encoding="utf-8")
    raw, count = re.subn(r"const unsigned char iplrom\[64\]\s*=\s*\{[^}]*\};", "", raw)
    if count != 1:
        raise ValueError("The pinned bsnes firmware declaration changed")
    resources.write_text(raw, encoding="utf-8", newline="\n")
    program = source / "bsnes/target-libretro/program.cpp"
    raw = program.read_text(encoding="utf-8")
    before = "result = vfs::memory::file::open(iplrom, sizeof(iplrom));"
    after = ('const char *omni_directory = nullptr;\n'
             '\t\tif (environ_cb(RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY, &omni_directory) && omni_directory)\n'
             '\t\t\tresult = vfs::fs::file::open({omni_directory, "/ipl.rom"}, mode);')
    if raw.count(before) != 1:
        raise ValueError("The pinned bsnes firmware loader changed")
    program.write_text(raw.replace(before, after, 1), encoding="utf-8", newline="\n")
    libco = source / "libco/libco.c"
    raw = libco.read_text(encoding="utf-8")
    shutil.copy2(ROOT / 'engines/native/adapters/windows-libco.c', source / 'libco/omniplay-windows-fibers.c')
    shutil.copy2(ROOT / 'engines/native/adapters/LICENSE.libco', source / 'libco/LICENSE.omniplay')
    libco.write_text('#if defined(_WIN32)\n#include "omniplay-windows-fibers.c"\n#else\n' + raw + '\n#endif\n',
                     encoding="utf-8", newline="\n")
    # Retain complete adapted corresponding source and a transformation description.
    # Do not put the removed manufacturer byte array into an exported patch file.
    (source / "OMNIPLAY_ADAPTATIONS.json").write_text(json.dumps({"schema_version": 1,
        "upstream_revision": PINS["bsnes"], "changes": ["external_user_supplied_spc_ipl",
        "windows_fibers_without_executable_heap", "owned_stable_handles_and_rederive"]}, indent=2) + "\n", encoding="utf-8")
    common = (source / "Makefile.common").read_text(encoding="utf-8")
    roots = {"CORE_DIR": "", "BSNES_DIR": "bsnes", "GB_DIR": "bsnes/gb", "SNES_DIR": "bsnes/sfc"}
    names = re.findall(r"\$\((CORE_DIR|BSNES_DIR|GB_DIR|SNES_DIR)\)/([A-Za-z0-9_./-]+\.(?:cpp|c)\b)", common)
    files = sorted({(Path(roots[key]) / path).as_posix() for key, path in names})
    if len(files) < 30 or any(not (source / name).is_file() for name in files):
        raise ValueError("The pinned bsnes source inventory is incomplete")
    listing = "\n".join('  "' + name + '"' for name in files)
    cmake = '''cmake_minimum_required(VERSION 3.25)
project(OmniPlayBsnes LANGUAGES C CXX)
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
add_library(bsnes_libretro SHARED
''' + listing + ''')
target_include_directories(bsnes_libretro PRIVATE . bsnes bsnes/gb bsnes/sfc)
target_compile_definitions(bsnes_libretro PRIVATE STDC_HEADERS __STDC_LIMIT_MACROS __LIBRETRO__ SFC_LAGFIX GB_INTERNAL DISABLE_DEBUGGER NOMINMAX _CRT_SECURE_NO_WARNINGS)
set_target_properties(bsnes_libretro PROPERTIES PREFIX "" WINDOWS_EXPORT_ALL_SYMBOLS ON)
if(MSVC)
  target_compile_options(bsnes_libretro PRIVATE /O2 /GS /EHsc /utf-8)
  target_link_options(bsnes_libretro PRIVATE /DYNAMICBASE /NXCOMPAT)
else()
  target_compile_options(bsnes_libretro PRIVATE -O3 -fPIC)
endif()
'''
    (source / "CMakeLists.txt").write_text(cmake, encoding="utf-8", newline="\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("core", choices=PINS)
    parser.add_argument("--cmake", required=True, type=Path)
    parser.add_argument("--generator", default="Ninja")
    parser.add_argument("--toolset")
    parser.add_argument("--c-compiler", type=Path)
    parser.add_argument("--cxx-compiler", type=Path)
    parser.add_argument("--parallel", type=int, default=8)
    parser.add_argument("--git", default="git")
    args = parser.parse_args()
    if not 1 <= args.parallel <= 32:
        raise ValueError("Build parallelism exceeds its bound")
    original = ROOT / "third_party/sources" / args.core
    revision = subprocess.check_output([args.git, "-C", str(original), "rev-parse", "HEAD"], text=True).strip()
    if revision != PINS[args.core]:
        raise ValueError("Native core does not match the pinned revision")
    output = ROOT / "out/native-cores" / args.core
    if output.exists():
        raise ValueError("Choose a fresh native core build; do not overwrite previous evidence")
    source = output / "source"
    shutil.copytree(original, source, ignore=shutil.ignore_patterns(".git"))
    if args.core == "bsnes":
        adapt_bsnes(source)
    build = output / "build"
    configure = [args.cmake, "-S", source, "-B", build, "-G", args.generator,
                 "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded"]
    if args.toolset:
        configure += ["-T", args.toolset]
    if args.c_compiler:
        configure += [f"-DCMAKE_C_COMPILER={args.c_compiler}"]
    if args.cxx_compiler:
        configure += [f"-DCMAKE_CXX_COMPILER={args.cxx_compiler}"]
    target = "bsnes_libretro"
    if args.core == "mgba":
        target = "mgba_libretro"
        configure += ["-DBUILD_LIBRETRO=ON", "-DBUILD_QT=OFF", "-DBUILD_SDL=OFF",
                      "-DBUILD_TEST=OFF", "-DBUILD_SUITE=OFF", "-DBUILD_PERF=OFF", "-DBUILD_EXAMPLE=OFF",
                      "-DBUILD_GL=OFF", "-DBUILD_GLES2=OFF", "-DBUILD_GLES3=OFF", "-DBUILD_PYTHON=OFF",
                      "-DBUILD_LTO=ON", "-DUSE_FFMPEG=OFF", "-DUSE_SQLITE3=OFF", "-DUSE_LIBZIP=OFF",
                      "-DUSE_LZMA=OFF", "-DUSE_PNG=OFF", "-DUSE_ELF=OFF", "-DUSE_LUA=OFF",
                      "-DENABLE_SCRIPTING=OFF", "-DENABLE_DEBUGGERS=OFF", "-DSKIP_GIT=ON",
                      "-DSKIP_LIBRARY=ON", "-DDISABLE_DEPS=ON", "-DM_CORE_GBA=ON", "-DM_CORE_GB=OFF"]
    run(configure)
    run([args.cmake, "--build", build, "--config", "Release", "--target", target,
         "--parallel", str(args.parallel)])
    binaries = [path for path in build.rglob(target + ".*") if path.suffix in {".dll", ".so", ".dylib"}]
    if len(binaries) != 1:
        raise ValueError("A single explicit native core library was not produced")
    (output / "build-receipt.json").write_text(json.dumps({"schema_version": 1,
        "core": args.core, "revision": revision, "library": str(binaries[0].resolve()),
        "corresponding_source": str(source.resolve()), "verification": "not_run",
        "firmware": [{"role": "snes_spc_ipl", "filename": "ipl.rom", "allowed_bytes": [64]}]
                    if args.core == "bsnes" else []}, indent=2) + "\n", encoding="utf-8")
    print("Native core binary authored. No runtime or compatibility tests were run.")


if __name__ == "__main__":
    main()
