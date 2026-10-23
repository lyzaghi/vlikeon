"""Build the pinned upstream core and its original open-source boot programs.

This is an authoring/build command, not an emulator conformance test. It is not
invoked automatically while the user's author-all-first instruction is active.
No copyrighted manufacturer boot ROM is downloaded or synthesized.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "third_party/sources/sameboy"
REVISION = "26a56bd6a122a6318a456abd80558408742d5adf"
SOURCES = ["gb", "sgb", "apu", "memory", "mbc", "timing", "display", "camera", "sm83_cpu", "joypad", "save_state", "random", "rumble"]
BOOTS = ["agb", "cgb", "cgb0", "mgb", "dmg", "sgb", "sgb2"]

def run(command, cwd=None):
    subprocess.run([str(value) for value in command], cwd=cwd, check=True)

def pb12(data):
    """Portable translation of SameBoy BootROMs/pb12.c (Expat license)."""
    data=data.rstrip(b"\0")
    if len(data)>0x4000:
        raise SystemExit("SameBoy logo exceeds the upstream compressor limit")
    output=bytearray()
    previous=[-1,-1]
    literals=bytearray()
    position=bits=control=0
    while True:
        if position==len(data):
            if bits==0:
                break
            byte=0
        else:
            byte=data[position]
            position+=1
        if byte in previous:
            bits+=2
            control=(control<<2)|2|(1 if byte==previous[1] else 0)
        else:
            bits+=2
            control<<=2
            last=previous[1]
            options=[(last|((last<<1)&255))&255,(last&(last<<1))&255,(last|((last>>1)&255))&255,(last&(last>>1))&255]
            if byte in options:
                control=((control|1)<<2)|options.index(byte)
                bits+=2
            else:
                literals.append(byte)
        previous=[previous[1],byte]
        if bits>=8:
            out_control=control>>(bits-8)
            if out_control==1:
                raise SystemExit("PB12 encoder emitted a reserved control byte")
            output.append(out_control)
            output.extend(literals)
            bits-=8
            control&=(1<<bits)-1
            literals.clear()
    output.append(1)
    return bytes(output)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--emcc", required=True, type=Path)
    parser.add_argument("--rgbasm", required=True, type=Path)
    parser.add_argument("--rgbgfx", required=True, type=Path)
    parser.add_argument("--rgblink", required=True, type=Path)
    parser.add_argument("--git", default="git")
    parser.add_argument("--distribution-approved",action="store_true",help="Record an explicit distribution decision after reviewing this core and bundled notices")
    args = parser.parse_args()
    revision = subprocess.check_output([args.git, "-C", str(CORE), "rev-parse", "HEAD"], text=True).strip()
    if revision != REVISION:
        raise SystemExit("SameBoy source does not match the pinned revision")
    for executable in [args.emcc, args.rgbasm, args.rgblink, args.rgbgfx]:
        if not executable.is_file():
            raise SystemExit(f"Required build tool is missing: {executable.name}")
    output = ROOT / "apps/game-hub/cores/sameboy"
    generated = ROOT / "out/sameboy-web"
    output.mkdir(parents=True, exist_ok=True)
    generated.mkdir(parents=True, exist_ok=True)
    # The color boot programs embed SameBoy's own logo. Generate the tile stream
    # with RGBDS, then reproduce the upstream PB12 encoding in portable Python.
    tiles=generated/"SameBoyLogo.2bpp"
    # Match upstream's RGBDS >= 0.7 flags. -h now means help, not traversal.
    run([args.rgbgfx,"-Z","-u","-c","embedded","-o",tiles,CORE/"BootROMs/SameBoyLogo.png"],CORE)
    (generated/"SameBoyLogo.pb12").write_bytes(pb12(tiles.read_bytes()))
    generated_sources = []
    for name in BOOTS:
        source = CORE / "BootROMs" / f"{name}_boot.asm"
        obj = generated / f"{name}_boot.o"
        binary = generated / f"{name}_boot.bin"
        run([args.rgbasm,"-I",str(generated)+os.sep, "-I", str(CORE / "BootROMs") + os.sep, "-o", obj, source], CORE / "BootROMs")
        run([args.rgblink, "-x", "-o", binary, obj], CORE / "BootROMs")
        data = binary.read_bytes()
        if not data or len(data) > 65536:
            raise SystemExit("Assembled boot program has an invalid length")
        c_source = generated / f"{name}_boot.c"
        c_source.write_text("/* Generated from SameBoy open-source BootROMs. */\n" + f"const unsigned char {name}_boot[] = {{" + ",".join(f"0x{byte:02x}" for byte in data) + "};\n" + f"const unsigned {name}_boot_length = sizeof({name}_boot);\n", encoding="utf-8")
        generated_sources.append(c_source)
    command = [args.emcc, "-O3", "-std=gnu11", "-DGB_INTERNAL", "-D__LIBRETRO__", "-DGB_DISABLE_TIMEKEEPING", "-DGB_DISABLE_REWIND", "-DGB_DISABLE_DEBUGGER", "-DGB_DISABLE_CHEATS", '-DGB_VERSION="1.0.4"', '-DGB_COPYRIGHT_YEAR="2026"', "-D_GNU_SOURCE", "-D_USE_MATH_DEFINES", "-I", CORE]
    command += [CORE / "Core" / f"{name}.c" for name in SOURCES]
    frontend=generated/"omniplay-frontend.c";shutil.copy2(ROOT/"engines/web/frontend.c",frontend)
    command += generated_sources + [frontend, CORE / "libretro/libretro.c", "-lm", "--no-entry", "-sMODULARIZE=1", "-sEXPORT_ES6=1", "-sEXPORT_NAME=createOmniPlaySameBoy", "-sENVIRONMENT=worker", "-sALLOW_MEMORY_GROWTH=1", "-sMAXIMUM_MEMORY=1073741824", "-sALLOW_TABLE_GROWTH=1", "-sFILESYSTEM=1", "-sEXPORTED_FUNCTIONS=@" + str(ROOT / "engines/web/exports.json"), "-sEXPORTED_RUNTIME_METHODS=@" + str(ROOT / "engines/web/runtime-methods.json"), "-o", output / "sameboy.js"]
    run(command, CORE)
    wasm = output / "sameboy.wasm"
    wasm_hash = hashlib.sha256(wasm.read_bytes()).hexdigest()
    shutil.copy2(CORE / "LICENSE", output / "LICENSE.sameboy.txt")
    manifest = {
        "schema_version": 1, "id": "sameboy-web", "core_version": "1.0.4", "source_revision": REVISION,
        "systems": ["gb", "gbc"], "module": "cores/sameboy/sameboy.js", "wasm": "cores/sameboy/sameboy.wasm",
        "wasm_sha256": wasm_hash, "wasm_bytes": wasm.stat().st_size,
        "license_expression": "MIT", "state_format_version": "sameboy-1.0.4", "in_game_format_version": "sameboy-memory-envelope-v1",
        "capabilities": ["pause", "gamepad", "haptics", "keyboard_mouse", "touch_controls", "save_ram", "save_state"],
        "content_formats":["raw_rom"],"media_filesystem_protocol":1,"frontend_source_sha256":hashlib.sha256(frontend.read_bytes()).hexdigest(),
        "firmware_protocol":1,"firmware":[],
        "distribution_approved": args.distribution_approved,
        "verification": "not_run"
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

if __name__ == "__main__":
    main()
