"""Build and package a Linux x86-64 native runtime (static-PIE bootstrap, device-free worker, SDL3 presentation helper, and closed ELF DT_NEEDED image closure); no tests."""
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import re
import shutil
import stat
import subprocess
from web_core_options_policy import validate_policy

ROOT = Path(__file__).resolve().parents[1]
PINS = {
    "sdl3": "829a65d769d935c4852f8159e964312c0957260a",
    "nlohmann-json": "55f93686c01528224f448c19128836e7df245f72",
    "sameboy": "26a56bd6a122a6318a456abd80558408742d5adf",
}
CORES = {
    "mgba": {
        "id": "mgba-native",
        "revision": "fd9dc5c2cbf234365068855851107e762fc6211b",
        "name": "mGBA",
        "library_version": "0.11.0",
        "version": "0.11.0",
        "systems": ["gba"],
        "license": "MPL-2.0",
        "notice": "LICENSE",
        "memory_mb": 768,
        "rom_bytes": 64 * 1024**2,
    },
    "bsnes": {
        "id": "bsnes-native",
        "revision": "63ee22f7af8f5d1ada295c2c98fd7bf261fa62a7",
        "name": "bsnes",
        "library_version": "115",
        "version": "115.0.0",
        "systems": ["snes"],
        "license": "GPL-3.0-only",
        "notice": "LICENSE.txt",
        "memory_mb": 1536,
        "rom_bytes": 64 * 1024**2,
    },
    "beetle-psx": {
        "id": "beetle-psx-native",
        "revision": "df2b53b306ba52d2116f8e8cc93b9ee33d239be7",
        "name": "Beetle PSX",
        "library_version": "0.9.44.1+omniplay.native3",
        "version": "0.9.44+1.omniplay.native3",
        "systems": ["ps1"],
        "license": "GPL-2.0-or-later",
        "notice": "COPYING",
        "memory_mb": 1536,
        "rom_bytes": 4 * 1024**3,
        "pause_barrier": True,
        "pause_barrier_protocol": 1,
    },
    "melonds-ds": {
        "id": "melonds-ds-native",
        "revision": "f394adbacb5722ee97c1b37c8064da9a25818310",
        "name": "melonDS DS",
        "library_version": "1.4.0",
        "version": "1.4.0",
        "systems": ["nds"],
        "license": "GPL-3.0-or-later",
        "notice": "LICENSE",
        "memory_mb": 2048,
        "rom_bytes": 512 * 1024**2,
    },
    "ares-md": {
        "id": "ares-md-native",
        "revision": "9408cb43d4948fc3ea6e152a307a34348df3fe04",
        "source_name": "ares",
        "name": "OmniPlay ares MD",
        "library_version": "0.3.0+ares.9408cb43d494",
        "version": "0.3.0+ares.9408cb43d494",
        "systems": ["mega_drive"],
        "license": "ISC AND MIT",
        "notice": "LICENSE",
        "memory_mb": 1536,
        "rom_bytes": 64 * 1024**2,
        "pause_barrier": True,
        "pause_barrier_protocol": 1,
    },
    "mupen64plus-next": {
        "id": "mupen64plus-next-native",
        "revision": "12edd2c74a517ff86dfa8cfc71ad75e4c10486d5",
        "name": "Mupen64Plus-Next",
        "library_version": "3.0.0+omniplay.sw1.12edd2c74a51",
        "version": "3.0.0+omniplay.sw1.12edd2c74a51",
        "systems": ["n64"],
        "license": "GPL-3.0-or-later AND LicenseRef-Angrylion-Provenance-Pending",
        "notice": "LICENSE",
        "memory_mb": 1536,
        "rom_bytes": 64 * 1024**2,
        "research_only": True,
    },
    "ppsspp": {
        "id": "ppsspp-native",
        "revision": "a5275067b62433375bbde12921e96329a795dff0",
        "name": "PPSSPP",
        "library_version": "1.20.4+omniplay.sw3.a5275067b624",
        "version": "1.20.4+omniplay.sw3.a5275067b624",
        "systems": ["psp"],
        "license": "GPL-2.0-or-later",
        "notice": "LICENSE.TXT",
        "memory_mb": 3072,
        "rom_bytes": 4 * 1024**3,
        "pause_barrier": True,
        "pause_barrier_protocol": 1,
    },
}

ELF_BASENAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_+.-]{0,126}\Z")


def inspect_x64_elf(path: Path, role: str) -> dict:
    raw = path.read_bytes()
    if len(raw) < 64 or raw[:4] != b"\x7fELF":
        raise ValueError(f"{path} is not an ELF binary")
    if (
        raw[4] != 2
        or raw[5] != 1
        or raw[6] != 1
        or raw[7] not in (0, 3)
        or int.from_bytes(raw[16:18], "little") != 3
        or int.from_bytes(raw[18:20], "little") != 62
        or int.from_bytes(raw[20:24], "little") != 1
        or int.from_bytes(raw[52:54], "little") != 64
        or int.from_bytes(raw[54:56], "little") != 56
    ):
        raise ValueError(f"{path} is not an x86-64 ET_DYN PIE/shared ELF image")

    entry = int.from_bytes(raw[24:32], "little")
    phoff = int.from_bytes(raw[32:40], "little")
    phnum = int.from_bytes(raw[56:58], "little")
    if not 1 <= phnum <= 128 or phoff < 64 or phoff + phnum * 56 > len(raw):
        raise ValueError(f"{path} has an invalid ELF program header table")

    loads = []
    dynamic = None
    interpreter = False
    stack = False
    relro = False
    exec_entry = False

    for index in range(phnum):
        ph = raw[phoff + index * 56 : phoff + (index + 1) * 56]
        p_type = int.from_bytes(ph[0:4], "little")
        p_flags = int.from_bytes(ph[4:8], "little")
        p_offset = int.from_bytes(ph[8:16], "little")
        p_vaddr = int.from_bytes(ph[16:24], "little")
        p_filesz = int.from_bytes(ph[32:40], "little")
        p_memsz = int.from_bytes(ph[40:48], "little")
        if p_flags & ~7 != 0 or p_memsz < p_filesz or p_offset + p_filesz > len(raw):
            raise ValueError(f"{path} has an out-of-bounds ELF segment")
        if p_type == 1:
            if (p_flags & 3) == 3:
                raise ValueError(f"{path} contains a writable+executable PT_LOAD segment")
            loads.append((p_vaddr, p_offset, p_filesz, p_memsz, p_flags))
            if (p_flags & 1) != 0 and p_vaddr <= entry < p_vaddr + p_filesz:
                exec_entry = True
        elif p_type == 2:
            if dynamic is not None or p_filesz == 0 or p_filesz > 16384 or p_filesz % 16 != 0:
                raise ValueError(f"{path} has an invalid PT_DYNAMIC segment")
            dynamic = (p_offset, p_filesz, p_vaddr)
        elif p_type == 3:
            interp = raw[p_offset : p_offset + p_filesz]
            if role in ("bootstrap", "loader") or interpreter or interp != b"/lib64/ld-linux-x86-64.so.2\0":
                raise ValueError(f"{path} has an unapproved PT_INTERP")
            interpreter = True
        elif p_type == 0x6474E551:
            if stack or (p_flags & 1) != 0:
                raise ValueError(f"{path} has an executable PT_GNU_STACK")
            stack = True
        elif p_type == 0x6474E552:
            if relro or p_memsz == 0:
                raise ValueError(f"{path} has an invalid PT_GNU_RELRO")
            relro = True

    if not loads or not stack or dynamic is None:
        raise ValueError(f"{path} is missing required PIE/stack/dynamic segments")
    if role != "bootstrap" and not relro:
        raise ValueError(f"{path} is missing PT_GNU_RELRO")
    if role in ("bootstrap", "presentation", "worker", "loader") and (entry == 0 or not exec_entry):
        raise ValueError(f"{path} has no executable entry point")
    if role in ("presentation", "worker") and not interpreter:
        raise ValueError(f"{path} is missing the approved /lib64/ld-linux-x86-64.so.2 interpreter")

    dyn_offset, dyn_size, _ = dynamic
    needed_offsets = []
    soname_offset = None
    strtab_vaddr = None
    strsz = None
    ended = False
    for pos in range(0, dyn_size, 16):
        tag = int.from_bytes(raw[dyn_offset + pos : dyn_offset + pos + 8], "little")
        val = int.from_bytes(raw[dyn_offset + pos + 8 : dyn_offset + pos + 16], "little")
        if tag == 0:
            ended = True
            break
        if tag in (15, 22, 29, 0x6FFFFEFA, 0x6FFFFEFB, 0x6FFFFEFC, 0x7FFFFFFD, 0x7FFFFFFF):
            raise ValueError(f"{path} contains forbidden DT_RPATH/RUNPATH/TEXTREL/audit tags")
        if tag == 1:
            if role == "bootstrap":
                raise ValueError("Static-PIE bootstrap must not contain DT_NEEDED dependencies")
            needed_offsets.append(val)
        elif tag == 5:
            strtab_vaddr = val
        elif tag == 10:
            strsz = val
        elif tag == 14:
            soname_offset = val

    if not ended:
        raise ValueError(f"{path} PT_DYNAMIC table is not DT_NULL terminated")

    needed = []
    soname = None
    if needed_offsets or soname_offset is not None:
        if strtab_vaddr is None or strsz is None or not 0 < strsz <= 4 * 1024 * 1024:
            raise ValueError(f"{path} has an invalid ELF string table")
        strtab_file_offset = None
        for vaddr, offset, filesz, _, _ in loads:
            if vaddr <= strtab_vaddr and strtab_vaddr + strsz <= vaddr + filesz:
                strtab_file_offset = offset + (strtab_vaddr - vaddr)
                break
        if strtab_file_offset is None:
            raise ValueError(f"{path} DT_STRTAB is not mapped inside a single PT_LOAD")

        def read_dyn_str(str_off: int) -> str:
            if str_off >= strsz:
                raise ValueError(f"{path} dynamic string offset out of bounds")
            end = raw.find(b"\0", strtab_file_offset + str_off, strtab_file_offset + strsz)
            if end < 0:
                raise ValueError(f"{path} dynamic string is not NUL-terminated")
            text = raw[strtab_file_offset + str_off : end].decode("ascii")
            if not ELF_BASENAME.fullmatch(text) or text.endswith("."):
                raise ValueError(f"{path} dynamic string {text!r} is not a closed basename")
            return text

        if soname_offset is not None:
            soname = read_dyn_str(soname_offset)
        for off in needed_offsets:
            name = read_dyn_str(off)
            if name in needed:
                raise ValueError(f"{path} has duplicate DT_NEEDED {name}")
            needed.append(name)

    if role == "worker":
        if soname is not None:
            raise ValueError("Worker executable must not declare DT_SONAME")
        for item in needed:
            low = item.lower()
            if any(token in low for token in ("sdl", "x11", "wayland", "gl", "vulkan", "pulse", "alsa", "pipewire", "drm", "gbm")):
                raise ValueError(f"Device-free Linux worker links forbidden library {item}")
    elif role in ("loader", "dependency"):
        if soname != path.name:
            raise ValueError(f"{path} role={role} must declare DT_SONAME matching basename {path.name} (got {soname!r})")
    elif role == "core":
        if soname is not None and soname != path.name:
            raise ValueError(f"Core library {path} DT_SONAME {soname!r} does not match basename {path.name}")

    return {"needed": needed, "soname": soname}


def verify_linux_image_closure(images_meta: list[dict]) -> None:
    providers = {}
    dependencies = {}
    pending = []
    for item in images_meta:
        role = item["role"]
        if role == "asset":
            continue
        target = item["target"]
        basename = target.rsplit("/", 1)[-1]
        inspected = item["elf"]
        if role != "worker":
            if basename in providers:
                raise ValueError(f"Duplicate ELF provider basename {basename}")
            providers[basename] = target
        if role in ("worker", "loader", "core"):
            pending.append(target)
        dependencies[target] = inspected["needed"]

    for target, needed in dependencies.items():
        missing = [name for name in needed if name not in providers]
        if missing:
            raise ValueError(f"Linux image {target} has unclosed DT_NEEDED dependencies: {missing}")

    reachable = set()
    while pending:
        current = pending.pop()
        if current in reachable:
            continue
        reachable.add(current)
        for dep_name in dependencies[current]:
            pending.append(providers[dep_name])

    unreachable = [target for target in dependencies if target not in reachable]
    if unreachable:
        raise ValueError(f"Linux package includes unreachable ELF images: {unreachable}")


def first_party_sources() -> dict[str, str]:
    result = {}
    for base in [ROOT / "engines/native", ROOT / "engines/presentation", ROOT / "engines/isolation-linux"]:
        for path in sorted(base.rglob("*")):
            if not path.is_file():
                continue
            if path.is_symlink() or path.stat().st_size > 4 * 1024 * 1024:
                raise ValueError("Native frontend/isolation source must remain an ordinary bounded source file")
            result[path.relative_to(ROOT).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    recipe = Path(__file__).resolve()
    result[recipe.relative_to(ROOT).as_posix()] = hashlib.sha256(recipe.read_bytes()).hexdigest()
    if not result or len(result) > 320:
        raise ValueError("Native frontend source exceeds its inventory bound")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("core", choices=CORES)
    parser.add_argument("--core-receipt", required=True, type=Path)
    parser.add_argument("--cmake", required=True, type=Path)
    parser.add_argument("--generator", default="Ninja")
    parser.add_argument("--parallel", type=int, default=8)
    parser.add_argument("--loader", required=True, type=Path, help="Explicitld-linux-x86-64.so.2 image")
    parser.add_argument("--dependency", action="append", default=[], type=Path, help="Closed shared libraries required by worker/core")
    parser.add_argument("--presentation-width", type=int, default=4096)
    parser.add_argument("--presentation-height", type=int, default=4096)
    parser.add_argument("--presentation-memory-mb", type=int, default=512)
    parser.add_argument("--root-limit-mb", type=int, default=256)
    parser.add_argument("--scratch-limit-mb", type=int, default=64)
    parser.add_argument("--save-limit-mb", type=int, default=64)
    parser.add_argument("--maximum-file-mb", type=int, default=64)
    parser.add_argument("--cpu-cores", type=int, default=4)
    parser.add_argument("--distribution-approved", action="store_true")
    parser.add_argument("--git", default="git")
    args = parser.parse_args()

    if platform.system() != "Linux" or platform.machine() not in ("x86_64", "amd64"):
        raise ValueError("The Linux native sandbox packager requires an x86-64 GNU/Linux host")
    if not 1 <= args.parallel <= 32:
        raise ValueError("Build parallelism exceeds its bound")
    spec = CORES[args.core]
    if not 1 <= args.presentation_width <= 4096 or not 1 <= args.presentation_height <= 4096 or not 256 <= args.presentation_memory_mb <= 2048 or args.presentation_width * args.presentation_height * 16 + 128 * 1024 * 1024 > args.presentation_memory_mb * 1024 * 1024:
        raise ValueError("Presentation dimensions exceed their signed memory policy")
    if not 16 <= args.root_limit_mb <= 1024 or not 4 <= args.scratch_limit_mb <= 512 or not 4 <= args.save_limit_mb <= 512 or not 1 <= args.maximum_file_mb <= 256 or not 1 <= args.cpu_cores <= 8:
        raise ValueError("Linux package limits exceed their closed policy")
    if args.distribution_approved and spec.get("research_only"):
        raise ValueError("This research renderer profile is not release eligible")

    receipt = json.loads(args.core_receipt.read_text(encoding="utf-8"))
    if args.distribution_approved and receipt.get("distribution_review_required"):
        raise ValueError("Resolve and record all component licensing and source review items before distribution approval")
    if receipt["core"] != args.core or receipt["revision"] != spec["revision"]:
        raise ValueError("The native core receipt differs from its pinned source")
    if receipt.get("video_backend", "software") != "software":
        raise ValueError("Linux native runtime requires a software core with private RGBA presentation")
    if any(receipt.get(field, False) != spec.get(field, False) for field in ["prepare_completion", "pause_barrier"]):
        raise ValueError("The native core receipt differs from its declared lifecycle barriers")
    if receipt.get("pause_barrier_protocol", 0) != spec.get("pause_barrier_protocol", 0):
        raise ValueError("Native pause requires its exact checked-barrier build receipt")

    for name, pin in {**PINS, args.core: spec["revision"]}.items():
        source_name = CORES.get(name, {}).get("source_name", name)
        revision = subprocess.check_output(
            [args.git, "-C", str(ROOT / "third_party/sources" / source_name), "rev-parse", "HEAD"],
            text=True,
        ).strip()
        if revision != pin:
            raise ValueError(f"Native dependency {name} does not match its pinned revision")

    library = Path(receipt["library"]).resolve(strict=True)
    if ".so" not in library.name:
        raise ValueError("Choose the Linux native core shared object (.so)")

    build_root = ROOT / "out/linux-native-workers" / spec["id"]
    bootstrap_build = build_root / "bootstrap"
    worker_build = build_root / "worker"
    output = ROOT / "apps/native-runtimes" / spec["id"]
    if build_root.exists() or output.exists():
        raise ValueError("Use a fresh Linux native runtime output directory")

    compiled_sources = first_party_sources()
    subprocess.run(
        [
            str(args.cmake),
            "-S",
            str(ROOT / "engines/isolation-linux"),
            "-B",
            str(bootstrap_build),
            "-G",
            args.generator,
            "-DCMAKE_BUILD_TYPE=Release",
        ],
        check=True,
    )
    subprocess.run(
        [
            str(args.cmake),
            "--build",
            str(bootstrap_build),
            "--config",
            "Release",
            "--target",
            "omniplay-linux-bootstrap",
            "--parallel",
            str(args.parallel),
        ],
        check=True,
    )

    subprocess.run(
        [
            str(args.cmake),
            "-S",
            str(ROOT / "engines/native"),
            "-B",
            str(worker_build),
            "-G",
            args.generator,
            f"-DOMNIPLAY_PRESENTATION_SOURCE={ROOT / 'engines/presentation'}",
            "-DOMNIPLAY_BUILD_PRESENTATION_HOST=ON",
            "-DOMNIPLAY_DEVICE_FREE_ACTOR=ON",
            "-DCMAKE_BUILD_TYPE=Release",
            f"-DOMNIPLAY_SDL_SOURCE={ROOT / 'third_party/sources/sdl3'}",
            f"-DOMNIPLAY_JSON_SOURCE={ROOT / 'third_party/sources/nlohmann-json'}",
            f"-DOMNIPLAY_LIBRETRO_INCLUDE={ROOT / 'third_party/sources/sameboy/libretro'}",
            f"-DOMNIPLAY_RUNTIME_ID={spec['id']}",
            f"-DOMNIPLAY_CORE_FILENAME={library.name}",
            "-DOMNIPLAY_VIDEO_BACKEND=software",
        ],
        check=True,
    )
    subprocess.run(
        [
            str(args.cmake),
            "--build",
            str(worker_build),
            "--config",
            "Release",
            "--target",
            "omniplay-native-runtime-build",
            "--parallel",
            str(args.parallel),
        ],
        check=True,
    )
    if first_party_sources() != compiled_sources:
        raise ValueError("Native frontend/isolation source changed during its build")

    bootstraps = list(bootstrap_build.rglob("omniplay-linux-bootstrap"))
    workers = list(worker_build.rglob("omniplay-native-worker"))
    helpers = list(worker_build.rglob("omniplay-presentation-host"))
    if len(bootstraps) != 1 or len(workers) != 1 or len(helpers) != 1:
        raise ValueError("Expected exactly one bootstrap, one worker, and one presentation host executable")

    inspect_x64_elf(bootstraps[0], "bootstrap")
    inspect_x64_elf(helpers[0], "presentation")
    worker_elf = inspect_x64_elf(workers[0], "worker")
    core_elf = inspect_x64_elf(library, "core")
    loader_path = args.loader.resolve(strict=True)
    if loader_path.name != "ld-linux-x86-64.so.2":
        raise ValueError("Loader filename must be ld-linux-x86-64.so.2")
    loader_elf = inspect_x64_elf(loader_path, "loader")

    output.mkdir(parents=True)
    os.chmod(output, 0o755)
    prefix = f"native-runtimes/{spec['id']}"

    def copy_package_file(src: Path, rel_dest: str, executable: bool) -> dict:
        dest = output / rel_dest
        if src.is_symlink() or not src.is_file() or dest.exists():
            raise ValueError(f"Package source {src} is invalid or collides at {dest}")
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dest)
        os.chmod(dest, 0o555 if executable else 0o444)
        return {
            "path": f"{prefix}/{rel_dest}",
            "sha256": hashlib.sha256(dest.read_bytes()).hexdigest(),
            "bytes": dest.stat().st_size,
            "executable": executable,
        }

    copy_package_file(bootstraps[0], "omniplay-linux-bootstrap", True)
    copy_package_file(helpers[0], "omniplay-presentation-host", True)
    copy_package_file(workers[0], "omniplay-native-worker", True)
    copy_package_file(loader_path, "ld-linux-x86-64.so.2", False)
    copy_package_file(library, library.name, False)

    linux_images = [
        {
            "source": f"{prefix}/omniplay-native-worker",
            "target": "runtime/omniplay-native-worker",
            "role": "worker",
            "elf": worker_elf,
        },
        {
            "source": f"{prefix}/ld-linux-x86-64.so.2",
            "target": "lib64/ld-linux-x86-64.so.2",
            "role": "loader",
            "elf": loader_elf,
        },
        {
            "source": f"{prefix}/{library.name}",
            "target": f"runtime/{library.name}",
            "role": "core",
            "elf": core_elf,
        },
    ]

    for dep in args.dependency:
        dep_resolved = dep.resolve(strict=True)
        dep_elf = inspect_x64_elf(dep_resolved, "dependency")
        copy_package_file(dep_resolved, dep_resolved.name, False)
        linux_images.append(
            {
                "source": f"{prefix}/{dep_resolved.name}",
                "target": f"runtime/{dep_resolved.name}",
                "role": "dependency",
                "elf": dep_elf,
            }
        )

    verify_linux_image_closure(linux_images)

    source_name = spec.get("source_name", args.core)
    copy_package_file(ROOT / "third_party/sources" / source_name / spec["notice"], f"LICENSE.{args.core}.txt", False)
    copy_package_file(ROOT / "third_party/sources/sdl3/LICENSE.txt", "LICENSE.SDL3.txt", False)
    copy_package_file(ROOT / "third_party/sources/nlohmann-json/LICENSE.MIT", "LICENSE.JSON.txt", False)
    copy_package_file(
        ROOT / "engines/isolation-linux/uapi/licenses/GPL-2.0.txt",
        "LICENSE.LinuxUAPI-GPL-2.0.txt",
        False,
    )
    copy_package_file(
        ROOT / "engines/isolation-linux/uapi/licenses/Linux-syscall-note.txt",
        "LICENSE.Linux-syscall-note.txt",
        False,
    )

    asset_paths = set()
    asset_bytes = 0
    for entry in receipt.get("system_assets", []):
        relative = entry["path"]
        parts = relative.split("/")
        if (
            not relative.startswith("assets/")
            or len(parts) > 8
            or any(not re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.-]{0,127}", part) or part.endswith(".") for part in parts)
            or relative in asset_paths
        ):
            raise ValueError("Native system asset has an unsafe or duplicate path")
        asset_paths.add(relative)
        asset_bytes += entry["bytes"]
        if len(asset_paths) > 128 or not 0 < entry["bytes"] <= 32 * 1024 * 1024 or asset_bytes > 128 * 1024 * 1024:
            raise ValueError("Native system assets exceed their inventory bound")
        original = Path(entry["source"]).resolve(strict=True)
        if (
            not original.is_file()
            or original.is_symlink()
            or original.stat().st_size != entry["bytes"]
            or hashlib.sha256(original.read_bytes()).hexdigest() != entry["sha256"]
        ):
            raise ValueError("Native system asset differs from its build receipt")
        copy_package_file(original, relative, False)
        linux_images.append(
            {
                "source": f"{prefix}/{relative}",
                "target": f"runtime/{relative}",
                "role": "asset",
                "elf": None,
            }
        )

    files = [
        {
            "path": f"{prefix}/{path.relative_to(output).as_posix()}",
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "bytes": path.stat().st_size,
        }
        for path in sorted(output.rglob("*"))
        if path.is_file()
    ]
    if len(files) > 256:
        raise ValueError("Linux native package exceeds its admitted file count")

    worker_rel = f"{prefix}/omniplay-native-worker"
    bootstrap_rel = f"{prefix}/omniplay-linux-bootstrap"
    presentation_rel = f"{prefix}/omniplay-presentation-host"
    clean_images = [{"source": item["source"], "target": item["target"], "role": item["role"]} for item in linux_images]

    package = {
        "manifest": {
            "schema_version": 1,
            "id": spec["id"],
            "version": spec["version"],
            "kind": "native_emulator",
            "systems": spec["systems"],
            "capabilities": ["pause", "save_ram", "save_state", "keyboard_mouse", "gamepad", "haptics"]
            + (["disc_control"] if args.core == "beetle-psx" else []),
            "platform": "linux-x86_64",
            "executable_sha256": next(f["sha256"] for f in files if f["path"] == worker_rel),
            "state_format_version": f"{args.core}-{spec['revision'][:16]}-opns1-linux64",
            "source_revision": spec["revision"],
            "license_expression": spec["license"],
            "distribution_approved": args.distribution_approved,
            "security_revoked": False,
        },
        "worker": worker_rel,
        "core_library": f"{prefix}/{library.name}",
        "expected_library_name": spec["name"],
        "expected_library_version": spec["library_version"],
        "in_game_format_version": f"{args.core}-memory-envelope-v1-linux64",
        "maximum_rom_bytes": spec["rom_bytes"],
        "memory_limit_mb": spec["memory_mb"],
        "files": files,
        "minimum_system_ram_bytes": 4 * 1024 * 1024
        if args.core == "melonds-ds"
        else 64 * 1024
        if args.core == "ares-md"
        else 32 * 1024 * 1024
        if args.core == "ppsspp"
        else 0,
        "reset_after_ram_load": args.core == "mupen64plus-next",
        "video_backend": "software",
        "gpu_max_width": 0,
        "gpu_max_height": 0,
        "prepare_completion": spec.get("prepare_completion", False),
        "pause_barrier": spec.get("pause_barrier", False),
        "pause_barrier_protocol": spec.get("pause_barrier_protocol", 0),
        "has_system_assets": bool(asset_paths),
        "core_options": {},
        "content_formats": ["raw_rom", "cue", "m3u", "chd", "iso", "pbp"] if args.core == "beetle-psx" else ["raw_rom"],
        "firmware_requirements": receipt.get("firmware", []),
        "presentation": {
            "protocol": 2,
            "executable": presentation_rel,
            "maximum_width": args.presentation_width,
            "maximum_height": args.presentation_height,
            "memory_limit_mb": args.presentation_memory_mb,
        },
        "linux": {
            "schema_version": 1,
            "bootstrap": bootstrap_rel,
            "images": clean_images,
            "root_limit_mb": args.root_limit_mb,
            "scratch_limit_mb": args.scratch_limit_mb,
            "save_limit_mb": args.save_limit_mb,
            "maximum_file_mb": args.maximum_file_mb,
            "cpu_cores": args.cpu_cores,
        },
    }

    catalog_path = output.parent / "catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8")) if catalog_path.exists() else {"schema_version": 1, "runtimes": []}
    if any(item["manifest"]["id"] == spec["id"] for item in catalog["runtimes"]):
        raise ValueError("Native catalog already contains this runtime identity")
    catalog["runtimes"].append(package)
    catalog_path.write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")

    (build_root / "corresponding-source.json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "platform": "linux-x86_64",
                "core_source": receipt["corresponding_source"],
                "worker_source": str(ROOT / "engines/native"),
                "presentation_source": str(ROOT / "engines/presentation"),
                "bootstrap_source": str(ROOT / "engines/isolation-linux"),
                "presentation_protocol": 2,
                "selected_presentation_mode": "brokered_rgba",
                "compiled_first_party_sources": compiled_sources,
                "bootstrap_sha256": next(f["sha256"] for f in files if f["path"] == bootstrap_rel),
                "worker_sha256": next(f["sha256"] for f in files if f["path"] == worker_rel),
                "presentation_sha256": next(f["sha256"] for f in files if f["path"] == presentation_rel),
                "linux_images": clean_images,
                "dependencies": PINS,
                "build_recipe": str(Path(__file__).resolve()),
                "verification": "not_run",
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print("Linux x86-64 native runtime package authored. Runtime and sandbox verification remain pending.")


if __name__ == "__main__":
    main()
