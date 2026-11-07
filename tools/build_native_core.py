"""Build pinned native software cores; explicit build, no gameplay or tests.

The bsnes adaptation removes its embedded 64-byte manufacturer IPL image. A user
must import a permitted firmware file into the private firmware vault. The Windows
fiber backend keeps executable-heap trampolines out of the contained worker.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PINS = {"sameboy": "26a56bd6a122a6318a456abd80558408742d5adf",
        "fceumm": "ac0d00a7fc01166f84c7bac01d1084ebfd2b865e",
        "mgba": "fd9dc5c2cbf234365068855851107e762fc6211b",
        "bsnes": "63ee22f7af8f5d1ada295c2c98fd7bf261fa62a7"}
SAMEBOY_SOURCES = ["gb", "sgb", "apu", "memory", "mbc", "timing", "display", "camera",
                   "sm83_cpu", "joypad", "save_state", "random", "rumble"]
SAMEBOY_BOOTS = ["agb", "cgb", "cgb0", "mgb", "dmg", "sgb", "sgb2"]


def pb12(data):
    """Portable translation of SameBoy BootROMs/pb12.c (Expat license)."""
    data = data.rstrip(b"\0")
    if len(data) > 0x4000:
        raise ValueError("SameBoy logo exceeds the upstream compressor limit")
    output = bytearray()
    previous = [-1, -1]
    literals = bytearray()
    position = bits = control = 0
    while True:
        if position == len(data):
            if bits == 0:
                break
            byte = 0
        else:
            byte = data[position]
            position += 1
        if byte in previous:
            bits += 2
            control = (control << 2) | 2 | (1 if byte == previous[1] else 0)
        else:
            bits += 2
            control <<= 2
            last = previous[1]
            options = [(last | ((last << 1) & 255)) & 255, (last & (last << 1)) & 255,
                       (last | ((last >> 1) & 255)) & 255, (last & (last >> 1)) & 255]
            if byte in options:
                control = ((control | 1) << 2) | options.index(byte)
                bits += 2
            else:
                literals.append(byte)
        previous = [previous[1], byte]
        if bits >= 8:
            out_control = control >> (bits - 8)
            if out_control == 1:
                raise ValueError("PB12 encoder emitted a reserved control byte")
            output.append(out_control)
            output.extend(literals)
            bits -= 8
            control &= (1 << bits) - 1
            literals.clear()
    output.append(1)
    return bytes(output)


def adapt_sameboy(source, rgbasm, rgbgfx, rgblink):
    for tool in (rgbasm, rgbgfx, rgblink):
        if tool is None or not tool.is_file():
            raise ValueError("Native SameBoy requires explicit --rgbasm, --rgbgfx, and --rgblink binaries")
    generated = source / "omniplay_bootroms"
    generated.mkdir(parents=True, exist_ok=True)
    tiles = generated / "SameBoyLogo.2bpp"
    run([rgbgfx, "-Z", "-u", "-c", "embedded", "-o", tiles, source / "BootROMs/SameBoyLogo.png"], source)
    (generated / "SameBoyLogo.pb12").write_bytes(pb12(tiles.read_bytes()))
    boot_files = []
    for name in SAMEBOY_BOOTS:
        asm = source / "BootROMs" / f"{name}_boot.asm"
        obj = generated / f"{name}_boot.o"
        binary = generated / f"{name}_boot.bin"
        run([rgbasm, "-I", str(generated) + os.sep, "-I", str(source / "BootROMs") + os.sep, "-o", obj, asm],
            source / "BootROMs")
        run([rgblink, "-x", "-o", binary, obj], source / "BootROMs")
        data = binary.read_bytes()
        if not data or len(data) > 65536:
            raise ValueError("Assembled SameBoy open-source boot program has an invalid length")
        c_file = generated / f"{name}_boot.c"
        c_file.write_text(
            "/* Generated from SameBoy open-source BootROMs. */\n"
            + f"const unsigned char {name}_boot[] = {{"
            + ",".join(f"0x{byte:02x}" for byte in data)
            + "};\n"
            + f"const unsigned {name}_boot_length = sizeof({name}_boot);\n",
            encoding="utf-8",
            newline="\n",
        )
        boot_files.append(c_file.relative_to(source).as_posix())
    files = [f"Core/{name}.c" for name in SAMEBOY_SOURCES] + boot_files + ["libretro/libretro.c"]
    listing = "\n".join(f'  "{name}"' for name in files)
    cmake = f'''cmake_minimum_required(VERSION 3.25)
project(OmniPlaySameBoy LANGUAGES C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
add_library(sameboy_libretro SHARED
{listing})
target_include_directories(sameboy_libretro PRIVATE . Core libretro)
target_compile_definitions(sameboy_libretro PRIVATE
  GB_INTERNAL __LIBRETRO__ GB_DISABLE_TIMEKEEPING GB_DISABLE_REWIND
  GB_DISABLE_DEBUGGER GB_DISABLE_CHEATS GB_VERSION="1.0.4"
  GB_COPYRIGHT_YEAR="2026" _GNU_SOURCE _USE_MATH_DEFINES _CRT_SECURE_NO_WARNINGS)
set_target_properties(sameboy_libretro PROPERTIES PREFIX "" WINDOWS_EXPORT_ALL_SYMBOLS ON)
if(MSVC)
  target_compile_options(sameboy_libretro PRIVATE /O2 /GS /utf-8 /experimental:c11atomics)
  target_link_options(sameboy_libretro PRIVATE /DYNAMICBASE /NXCOMPAT)
else()
  target_compile_options(sameboy_libretro PRIVATE -O3 -fPIC)
  target_link_libraries(sameboy_libretro PRIVATE m)
endif()
'''
    (source / "CMakeLists.txt").write_text(cmake, encoding="utf-8", newline="\n")


def adapt_fceumm(source):
    files = sorted(
        path.relative_to(source).as_posix()
        for folder in ("src/boards", "src/input")
        for path in (source / folder).glob("*.c")
    )
    files += [
        "src/ntsc/nes_ntsc.c",
        "src/drivers/libretro/libretro.c",
        "src/drivers/libretro/libretro_dipswitch.c",
        "src/cart.c", "src/cheat.c", "src/crc32.c", "src/fceu-endian.c",
        "src/fceu-memory.c", "src/fceu.c", "src/fds.c", "src/fds_apu.c",
        "src/file.c", "src/filter.c", "src/general.c", "src/input.c",
        "src/md5.c", "src/nsf.c", "src/palette.c", "src/ppu.c",
        "src/sound.c", "src/state.c", "src/video.c", "src/vsuni.c",
        "src/ines.c", "src/unif.c", "src/x6502.c",
        "src/drivers/libretro/libretro-common/streams/memory_stream.c",
        "src/drivers/libretro/libretro-common/compat/compat_posix_string.c",
        "src/drivers/libretro/libretro-common/compat/compat_snprintf.c",
        "src/drivers/libretro/libretro-common/compat/compat_strcasestr.c",
        "src/drivers/libretro/libretro-common/compat/compat_strl.c",
        "src/drivers/libretro/libretro-common/compat/fopen_utf8.c",
        "src/drivers/libretro/libretro-common/encodings/encoding_utf.c",
        "src/drivers/libretro/libretro-common/file/file_path.c",
        "src/drivers/libretro/libretro-common/file/file_path_io.c",
        "src/drivers/libretro/libretro-common/streams/file_stream.c",
        "src/drivers/libretro/libretro-common/streams/file_stream_transforms.c",
        "src/drivers/libretro/libretro-common/string/stdstring.c",
        "src/drivers/libretro/libretro-common/time/rtime.c",
        "src/drivers/libretro/libretro-common/vfs/vfs_implementation.c",
    ]
    if len(files) < 80 or any(not (source / name).is_file() for name in files):
        raise ValueError("The pinned fceumm source inventory is incomplete")
    listing = "\n".join(f'  "{name}"' for name in files)
    cmake = f'''cmake_minimum_required(VERSION 3.25)
project(OmniPlayFceumm LANGUAGES C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
add_library(fceumm_libretro SHARED
{listing})
target_include_directories(fceumm_libretro PRIVATE
  src src/boards src/input src/ntsc src/drivers/libretro
  src/drivers/libretro/libretro-common/include)
target_compile_definitions(fceumm_libretro PRIVATE
  __LIBRETRO__ PATH_MAX=1024 FCEU_VERSION_NUMERIC=9900
  FRONTEND_SUPPORTS_RGB888 HAVE_NTSC_FILTER _CRT_SECURE_NO_WARNINGS)
if(WIN32)
  target_compile_definitions(fceumm_libretro PRIVATE PSS_STYLE=2)
else()
  target_compile_definitions(fceumm_libretro PRIVATE PSS_STYLE=1)
endif()
set_target_properties(fceumm_libretro PROPERTIES PREFIX "" WINDOWS_EXPORT_ALL_SYMBOLS ON)
if(MSVC)
  target_compile_definitions(fceumm_libretro PRIVATE INLINE=_inline)
  target_compile_options(fceumm_libretro PRIVATE /O2 /GS /utf-8)
  target_link_options(fceumm_libretro PRIVATE /DYNAMICBASE /NXCOMPAT)
else()
  target_compile_options(fceumm_libretro PRIVATE -O3 -fPIC)
  target_link_libraries(fceumm_libretro PRIVATE m)
endif()
'''
    (source / "CMakeLists.txt").write_text(cmake, encoding="utf-8", newline="\n")


def run(command, cwd=None):
    subprocess.run([str(value) for value in command], cwd=cwd, check=True)


def adapt_mgba(source):
    cmake = source / "CMakeLists.txt"
    raw = cmake.read_text(encoding="utf-8")
    for old, new in [
        ("cmake_minimum_required(VERSION 3.12)", "cmake_minimum_required(VERSION 3.25)"),
        ("list(APPEND OS_LIB ws2_32 shlwapi)", ""),
        ("file(GLOB OS_SRC ${CMAKE_CURRENT_SOURCE_DIR}/src/platform/windows/*.c)",
         "set(OS_SRC ${CMAKE_CURRENT_SOURCE_DIR}/src/platform/windows/memory.c)"),
    ]:
        if raw.count(old) != 1:
            raise ValueError("The pinned mGBA CMake configuration changed")
        raw = raw.replace(old, new, 1)
    cmake.write_text(raw, encoding="utf-8", newline="\n")
    config = source / "src/core/config.c"
    raw_cfg = config.read_text(encoding="utf-8")
    for old, new in [
        ("#ifdef _WIN32\n\tWCHAR wpath[MAX_PATH];", "#if defined(_WIN32) && !defined(MINIMAL_CORE)\n\tWCHAR wpath[MAX_PATH];"),
        ("#elif defined(PSP2)\n\tsnprintf(out, outLength, \"ux0:data/%s\", projectName);",
         "#elif defined(MINIMAL_CORE)\n\tUNUSED(outLength);\n\tout[0] = '\\0';\n#elif defined(PSP2)\n\tsnprintf(out, outLength, \"ux0:data/%s\", projectName);"),
        ("#ifdef _WIN32\n\twchar_t wpath[MAX_PATH];", "#if defined(_WIN32) && !defined(MINIMAL_CORE)\n\twchar_t wpath[MAX_PATH];"),
        ("#elif defined(PSP2) || defined(GEKKO) || defined(__SWITCH__) || defined(__3DS__)",
         "#elif defined(PSP2) || defined(GEKKO) || defined(__SWITCH__) || defined(__3DS__) || defined(MINIMAL_CORE)"),
    ]:
        if raw_cfg.count(old) != 1:
            raise ValueError("The pinned mGBA config path helper changed")
        raw_cfg = raw_cfg.replace(old, new, 1)
    config.write_text(raw_cfg, encoding="utf-8", newline="\n")
    vfs = source / "src/util/vfs.c"
    raw_vfs = vfs.read_text(encoding="utf-8")
    old_vfs = "\tWCHAR wpath[PATH_MAX];\n\tMultiByteToWideChar(CP_UTF8, 0, path, -1, wpath, PATH_MAX);\n\treturn !PathIsRelativeW(wpath);"
    new_vfs = "\treturn path && (path[0] == '/' || path[0] == '\\\\' || (((path[0] >= 'A' && path[0] <= 'Z') || (path[0] >= 'a' && path[0] <= 'z')) && path[1] == ':' && (path[2] == '/' || path[2] == '\\\\')));"
    if raw_vfs.count(old_vfs) != 1:
        raise ValueError("The pinned mGBA VFS path check changed")
    vfs.write_text(raw_vfs.replace(old_vfs, new_vfs, 1), encoding="utf-8", newline="\n")


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
    traits = source / "nall/traits.hpp"
    raw_traits = traits.read_text(encoding="utf-8")
    old_traits = "  using std::is_signed;\n  using std::is_signed_v;\n  using std::is_unsigned;\n  using std::is_unsigned_v;"
    new_traits = "  template<typename T> struct is_signed : std::is_signed<T> {};\n  template<typename T> struct is_unsigned : std::is_unsigned<T> {};\n  template<typename T> inline constexpr bool is_signed_v = is_signed<T>::value;\n  template<typename T> inline constexpr bool is_unsigned_v = is_unsigned<T>::value;\n#if INTMAX_BITS >= 128\n  template<> struct is_signed<int128_t> : std::true_type {};\n  template<> struct is_unsigned<uint128_t> : std::true_type {};\n#endif"
    old_std_spec = "namespace std {\n  #if INTMAX_BITS >= 128\n  template<> struct is_signed<int128_t> : true_type {};\n  template<> struct is_unsigned<uint128_t> : true_type {};\n  #endif\n}"
    if raw_traits.count(old_traits) != 1 or raw_traits.count(old_std_spec) != 1:
        raise ValueError("The pinned bsnes traits declaration changed")
    traits.write_text(raw_traits.replace(old_traits, new_traits, 1).replace(old_std_spec, "", 1), encoding="utf-8", newline="\n")
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
target_compile_definitions(bsnes_libretro PRIVATE STDC_HEADERS __STDC_LIMIT_MACROS __LIBRETRO__ SFC_LAGFIX GB_INTERNAL DISABLE_DEBUGGER NOMINMAX _GNU_SOURCE _USE_MATH_DEFINES _CRT_SECURE_NO_WARNINGS)
set_target_properties(bsnes_libretro PROPERTIES PREFIX "" WINDOWS_EXPORT_ALL_SYMBOLS ON)
if(MSVC)
  target_compile_options(bsnes_libretro PRIVATE /O2 /GS /EHsc /utf-8)
  target_link_options(bsnes_libretro PRIVATE /DYNAMICBASE /NXCOMPAT)
elseif(WIN32)
  target_compile_options(bsnes_libretro PRIVATE -O3)
  target_link_options(bsnes_libretro PRIVATE -static -static-libgcc -static-libstdc++)
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
    parser.add_argument("--rgbasm", type=Path)
    parser.add_argument("--rgbgfx", type=Path)
    parser.add_argument("--rgblink", type=Path)
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
    elif args.core == "sameboy":
        adapt_sameboy(source, args.rgbasm, args.rgbgfx, args.rgblink)
    elif args.core == "fceumm":
        adapt_fceumm(source)
    elif args.core == "mgba":
        adapt_mgba(source)
    build = output / "build"
    configure = [args.cmake, "-S", source, "-B", build, "-G", args.generator,
                 "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded"]
    if args.toolset:
        configure += ["-T", args.toolset]
    if args.c_compiler:
        configure += [f"-DCMAKE_C_COMPILER={args.c_compiler}"]
    if args.cxx_compiler:
        configure += [f"-DCMAKE_CXX_COMPILER={args.cxx_compiler}"]
    target = f"{args.core}_libretro"
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
