"""Build and package an isolated native worker from pinned sources; no tests."""
import argparse
import hashlib
import json
import platform
from pathlib import Path
import re
import shutil
import subprocess
from web_core_options_policy import validate_policy
from windows_actor_imports import require_device_free_imports

ROOT = Path(__file__).resolve().parents[1]
PINS = {"sdl3": "829a65d769d935c4852f8159e964312c0957260a",
        "nlohmann-json": "55f93686c01528224f448c19128836e7df245f72",
        "sameboy": "26a56bd6a122a6318a456abd80558408742d5adf"}
CORES = {
    "mgba": {"id": "mgba-native", "revision": "fd9dc5c2cbf234365068855851107e762fc6211b",
             "name": "mGBA", "library_version": "0.11.0", "version": "0.11.0", "systems": ["gba"],
             "license": "MPL-2.0", "notice": "LICENSE", "memory_mb": 768, "rom_bytes": 64*1024**2},
    "bsnes": {"id": "bsnes-native", "revision": "63ee22f7af8f5d1ada295c2c98fd7bf261fa62a7",
              "name": "bsnes", "library_version": "115", "version": "115.0.0", "systems": ["snes"],
              "license": "GPL-3.0-only", "notice": "LICENSE.txt", "memory_mb": 1536, "rom_bytes": 64*1024**2},
    "beetle-psx":{"id":"beetle-psx-native","revision":"df2b53b306ba52d2116f8e8cc93b9ee33d239be7",
                  "name":"Beetle PSX","library_version":"0.9.44.1+omniplay.native3","version":"0.9.44+1.omniplay.native3","systems":["ps1"],
                  "license":"GPL-2.0-or-later","notice":"COPYING","memory_mb":1536,"rom_bytes":4*1024**3,"pause_barrier":True,"pause_barrier_protocol":1},
    "melonds-ds":{"id":"melonds-ds-native","revision":"f394adbacb5722ee97c1b37c8064da9a25818310",
                  "name":"melonDS DS","library_version":"1.4.0","version":"1.4.0","systems":["nds"],
                  "license":"GPL-3.0-or-later","notice":"LICENSE","memory_mb":2048,"rom_bytes":512*1024**2},
    "ares-md":{"id":"ares-md-native","revision":"9408cb43d4948fc3ea6e152a307a34348df3fe04",
               "source_name":"ares",
               "name":"OmniPlay ares MD","library_version":"0.3.0+ares.9408cb43d494","version":"0.3.0+ares.9408cb43d494","systems":["mega_drive"],
               "license":"ISC AND MIT","notice":"LICENSE","memory_mb":1536,"rom_bytes":64*1024**2,"pause_barrier":True,"pause_barrier_protocol":1},
    "mupen64plus-next":{"id":"mupen64plus-next-native","revision":"12edd2c74a517ff86dfa8cfc71ad75e4c10486d5",
                        "name":"Mupen64Plus-Next","library_version":"3.0.0+omniplay.sw1.12edd2c74a51","version":"3.0.0+omniplay.sw1.12edd2c74a51","systems":["n64"],
                        "license":"GPL-3.0-or-later AND LicenseRef-Angrylion-Provenance-Pending","notice":"LICENSE","memory_mb":1536,"rom_bytes":64*1024**2,"research_only":True},
    "mupen64plus-gl":{"id":"mupen64plus-gl-native","revision":"12edd2c74a517ff86dfa8cfc71ad75e4c10486d5","source_name":"mupen64plus-next",
                      "name":"Mupen64Plus-Next","library_version":"3.0.0+omniplay.gl1.12edd2c74a51","version":"3.0.0+omniplay.gl1.12edd2c74a51","systems":["n64"],
                      "license":"GPL-2.0-only","notice":"LICENSE","memory_mb":1536,"rom_bytes":64*1024**2,"video_backend":"opengl_core","gpu_max_width":640,"gpu_max_height":480},
    "ppsspp":{"id":"ppsspp-native","revision":"a5275067b62433375bbde12921e96329a795dff0",
              "name":"PPSSPP","library_version":"1.20.4+omniplay.sw3.a5275067b624","version":"1.20.4+omniplay.sw3.a5275067b624","systems":["psp"],
              "license":"GPL-2.0-or-later","notice":"LICENSE.TXT","memory_mb":3072,"rom_bytes":4*1024**3,"pause_barrier":True,"pause_barrier_protocol":1},
    "ppsspp-gl":{"id":"ppsspp-gl-native","revision":"a5275067b62433375bbde12921e96329a795dff0","source_name":"ppsspp",
                 "name":"PPSSPP","library_version":"1.20.4+omniplay.gl3.a5275067b624","version":"1.20.4+omniplay.gl3.a5275067b624","systems":["psp"],
                 "license":"GPL-2.0-or-later","notice":"LICENSE.TXT","memory_mb":3072,"rom_bytes":4*1024**3,"video_backend":"opengl_core","gpu_max_width":480,"gpu_max_height":272,"prepare_completion":True,"pause_barrier":True,"pause_barrier_protocol":1},
}

def require_x64_pe(path):
    with path.open("rb") as source:
        header = source.read(64)
        if len(header) != 64 or header[:2] != b"MZ":
            raise ValueError("Native package contains a non-PE binary")
        offset = int.from_bytes(header[60:64], "little")
        if offset > 1024**2:
            raise ValueError("Native PE header offset exceeds its bound")
        source.seek(offset)
        pe = source.read(6)
        if pe[:4] != b"PE\0\0" or int.from_bytes(pe[4:6], "little") != 0x8664:
            raise ValueError("The current native package requires Windows x86-64 binaries")


def first_party_sources():
    result={}
    for base in [ROOT/'engines/native',ROOT/'engines/presentation']:
        for path in sorted(base.rglob('*')):
            if not path.is_file():continue
            if path.is_symlink() or getattr(path,'is_junction',lambda:False)() or path.stat().st_size>4*1024*1024:raise ValueError('Native frontend source must remain an ordinary bounded source file')
            result[path.relative_to(ROOT).as_posix()]=hashlib.sha256(path.read_bytes()).hexdigest()
    recipe=Path(__file__).resolve();result[recipe.relative_to(ROOT).as_posix()]=hashlib.sha256(recipe.read_bytes()).hexdigest()
    if not result or len(result)>256:raise ValueError('Native frontend source exceeds its inventory bound')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("core", choices=CORES)
    parser.add_argument("--core-receipt", required=True, type=Path)
    parser.add_argument("--cmake", required=True, type=Path)
    parser.add_argument("--generator", default="Ninja")
    parser.add_argument("--toolset")
    parser.add_argument("--parallel", type=int, default=8)
    parser.add_argument("--support-library", action="append", default=[], type=Path)
    parser.add_argument("--presentation-mode",choices=['direct','brokered_rgba'],default='direct')
    parser.add_argument("--presentation-width",type=int,default=4096)
    parser.add_argument("--presentation-height",type=int,default=4096)
    parser.add_argument("--presentation-memory-mb",type=int,default=512)
    parser.add_argument("--distribution-approved", action="store_true")
    parser.add_argument("--git", default="git")
    args = parser.parse_args()
    if platform.system() != "Windows":
        raise ValueError("The current native sandbox package targets Windows; other sandbox adapters are still required")
    if not 1 <= args.parallel <= 32:
        raise ValueError("Build parallelism exceeds its bound")
    spec = CORES[args.core]
    brokered=args.presentation_mode=='brokered_rgba'
    if brokered and spec.get('video_backend','software')!='software':raise ValueError('Private RGBA presentation requires a software core; no implicit GPU fallback is permitted')
    if not 1<=args.presentation_width<=4096 or not 1<=args.presentation_height<=4096 or not 256<=args.presentation_memory_mb<=2048 or args.presentation_width*args.presentation_height*16+128*1024*1024>args.presentation_memory_mb*1024*1024:raise ValueError('Presentation dimensions exceed their signed memory policy')
    if args.distribution_approved and spec.get("research_only"):
        raise ValueError("This research renderer profile is not release eligible; select a separately reviewed product renderer")
    receipt = json.loads(args.core_receipt.read_text(encoding="utf-8"))
    if args.distribution_approved and receipt.get("distribution_review_required"):
        raise ValueError("Resolve and record all component licensing and source review items before distribution approval")
    if receipt["core"] != args.core or receipt["revision"] != spec["revision"]:
        raise ValueError("The native core receipt differs from its pinned source")
    if args.core in {"ppsspp","ppsspp-gl"}:
        from ffmpeg_source import PIN as decoder_pin
        from ppsspp_native_source import POLICY as psp_source_policy
        from pinned_core_source import file_hash,source_inventory
        decoder=receipt.get('ffmpeg_build_receipt',{})
        if receipt.get('audio_output_protocol')!=1 or receipt.get('ffmpeg_revision')!=decoder_pin or decoder.get('revision')!=decoder_pin or decoder.get('target')!='windows-x86_64' or decoder.get('runtime_crt')!='msvc_static' or decoder.get('source_only') is not False:
            raise ValueError('The current PSP package requires its exact modern static decoder build receipt')
        if receipt.get('version')!=spec['version'] or receipt.get('source_policy')!=psp_source_policy or receipt.get('source_only') is not False:
            raise ValueError('The native PSP package requires its compiled committed-source profile')
        source=Path(receipt['corresponding_source']).resolve(strict=True)
        if source_inventory(source)!=receipt.get('compiled_source_sha256') or file_hash(source.parent/'committed-source.json')!=receipt.get('committed_source_sha256'):
            raise ValueError('The compiled PSP source/provenance changed after its build receipt')
        if file_hash(Path(receipt['library']))!=receipt.get('library_sha256'):
            raise ValueError('The native PSP library differs from its compiled-source receipt')
    if receipt.get("video_backend","software")!=spec.get("video_backend","software"):
        raise ValueError("The native core receipt differs from the selected presentation policy")
    if any(receipt.get(field,False)!=spec.get(field,False) for field in ['prepare_completion','pause_barrier']):
        raise ValueError("The native core receipt differs from its declared lifecycle barriers")
    pause_protocol=receipt.get('pause_barrier_protocol',0)
    if type(pause_protocol)is not int or pause_protocol!=spec.get('pause_barrier_protocol',0):
        raise ValueError('Native pause requires its exact checked-barrier build receipt')
    if args.core=="beetle-psx":
        if receipt.get("version")!=spec["library_version"] or receipt.get("file_io_protocol")!=1 or receipt.get("filesystem_save_memory")is not True or type(receipt.get("writable_files_protocol"))is not int or receipt['writable_files_protocol']!=1:
            raise ValueError("Native PS1 needs its exact checked card and combined-save source adaptation")
        if receipt.get("adapter_sha256")!=hashlib.sha256((ROOT/"engines/native/adapters/beetle-save-io.c.inc").read_bytes()).hexdigest() or receipt.get("core_options_policy_sha256")!=hashlib.sha256((ROOT/"engines/policies/beetle-options.json").read_bytes()).hexdigest():
            raise ValueError("Native PS1 source adapters/options differ from their build receipt")
        inputs={name:ROOT/'engines/native/adapters'/name for name in ['beetle-memory-files.c','beetle-memory-files.h','beetle-memory-frontio.c.inc']}
        inputs['writable_files_v1.h']=ROOT/'engines/native/src/writable_files_v1.h'
        if receipt.get('memory_adapter_sha256')!={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in inputs.items()}:raise ValueError('Native PS1 requires its exact memory-card bridge and ABI header')
    for name, pin in {**PINS, args.core: spec["revision"]}.items():
        source_name=CORES.get(name,{}).get("source_name",name)
        revision = subprocess.check_output([args.git, "-C", str(ROOT/"third_party/sources"/source_name), "rev-parse", "HEAD"], text=True).strip()
        if revision != pin:
            raise ValueError("Native dependency does not match its pinned revision")
    library = Path(receipt["library"]).resolve(strict=True)
    if args.core=="ares-md":
        adapter_inputs={'adapter.cpp':'engines/ares-md/adapter.cpp','web-files.hpp':'engines/ares-md/web-files.hpp','save-memory.inc':'engines/ares-md/save-memory.inc','windows-libco.c':'engines/native/adapters/windows-libco.c','md-save-publish.c':'engines/native/adapters/md-save-publish.c','writable_files_v1.h':'engines/native/src/writable_files_v1.h'}
        expected={name:hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for name,path in adapter_inputs.items()}
        if receipt.get('core_version')!=spec['version'] or receipt.get('adapter_sha256')!=expected or receipt.get('core_options_policy_sha256')!=hashlib.sha256((ROOT/'engines/policies/ares-md-options.json').read_bytes()).hexdigest() or receipt.get('pause_barrier') is not True or receipt.get('file_io_protocol')!=1 or type(receipt.get('portable_contract_protocol')) is not int or receipt['portable_contract_protocol']!=1 or receipt.get('filesystem_save_memory') is not False or receipt.get('core_save_directory')!='save/MD' or type(receipt.get('writable_files_protocol')) is not int or receipt['writable_files_protocol']!=1 or receipt.get('library_sha256')!=hashlib.sha256(library.read_bytes()).hexdigest():
            raise ValueError('Native MD requires its matching checked persistent-file/fiber build receipt')
    if args.core=="beetle-psx" and receipt.get("library_sha256")!=hashlib.sha256(library.read_bytes()).hexdigest():
        raise ValueError("Native PS1 artifact differs from its checked-adapter build receipt")
    if library.suffix != ".dll":
        raise ValueError("Choose the Windows native core library")
    build = ROOT / "out/native-workers" / spec["id"]
    output = ROOT / "apps/native-runtimes" / spec["id"]
    if build.exists() or output.exists():
        raise ValueError("Use a fresh native runtime output directory")
    compiled_sources=first_party_sources()
    command = [args.cmake, "-S", ROOT/"engines/native", "-B", build, "-G", args.generator,
               f"-DOMNIPLAY_PRESENTATION_SOURCE={ROOT/'engines/presentation'}",
        f"-DOMNIPLAY_BUILD_PRESENTATION_HOST={'ON' if brokered else 'OFF'}",
        f"-DOMNIPLAY_DEVICE_FREE_ACTOR={'ON' if brokered else 'OFF'}",
        "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded",
        f"-DOMNIPLAY_SDL_SOURCE={ROOT/'third_party/sources/sdl3'}",
        f"-DOMNIPLAY_JSON_SOURCE={ROOT/'third_party/sources/nlohmann-json'}",
        f"-DOMNIPLAY_LIBRETRO_INCLUDE={ROOT/'third_party/sources/sameboy/libretro'}",
        f"-DOMNIPLAY_RUNTIME_ID={spec['id']}", f"-DOMNIPLAY_CORE_FILENAME={library.name}",f"-DOMNIPLAY_VIDEO_BACKEND={spec.get('video_backend','software')}"]
    if args.toolset:
        command += ["-T", args.toolset]
    subprocess.run([str(value) for value in command], check=True)
    subprocess.run([str(args.cmake), "--build", str(build), "--config", "Release", "--target","omniplay-native-runtime-build","--parallel", str(args.parallel)], check=True)
    if first_party_sources()!=compiled_sources:raise ValueError('Native frontend/helper source changed during its build')
    workers = list(build.rglob("omniplay-native-worker.exe"))
    if len(workers) != 1:
        raise ValueError("A single native worker executable was not produced")
    require_x64_pe(workers[0]); require_x64_pe(library)
    actor_imports=require_device_free_imports(workers[0]) if brokered else None
    helpers=list(build.rglob('omniplay-presentation-host.exe'))if brokered else []
    if brokered and len(helpers)!=1:raise ValueError('A single private presentation helper executable was not produced')
    for helper in helpers:require_x64_pe(helper)
    output.mkdir(parents=True)
    for source in [workers[0], library,*helpers,*args.support_library]:
        if source.is_symlink() or not source.is_file() or (output/source.name).exists():
            raise ValueError("Native runtime file is missing, linked or collides with another file")
        shutil.copy2(source, output/source.name)
    source_name=spec.get("source_name",args.core)
    shutil.copy2(ROOT/"third_party/sources"/source_name/spec["notice"], output/f"LICENSE.{args.core}.txt")
    shutil.copy2(ROOT/"third_party/sources/sdl3/LICENSE.txt", output/"LICENSE.SDL3.txt")
    shutil.copy2(ROOT/"third_party/sources/nlohmann-json/LICENSE.MIT", output/"LICENSE.JSON.txt")
    if args.core=="beetle-psx":shutil.copy2(ROOT/"third_party/sources/beetle-psx/deps/openbios/LICENSE.txt",output/"LICENSE.OpenBIOS.txt")
    if args.core in {'ares-md','bsnes'}:shutil.copy2(ROOT/'engines/native/adapters/LICENSE.libco',output/'LICENSE.OmniPlayFibers.txt')
    if args.core=='ares-md':
        interface=subprocess.check_output([args.git,'-C',str(ROOT/'third_party/sources/sameboy'),'show',PINS['sameboy']+':libretro/libretro.h']).decode('utf-8')
        end=interface.find('*/')
        if not 0<end<16384:raise ValueError('Libretro interface notice boundary changed')
        (output/'LICENSE.libretro-interface.txt').write_text(interface[:end+2]+'\n',encoding='utf-8')
    for index,notice in enumerate(receipt.get("additional_notices",[])):
        notice=Path(notice).resolve(strict=True)
        if not notice.is_file() or notice.stat().st_size>4*1024*1024:raise ValueError("Core attribution file is missing or exceeds its bound")
        shutil.copy2(notice,output/f"LICENSE.{args.core}.dependencies-{index}.txt")
    asset_paths=set();asset_bytes=0
    for entry in receipt.get("system_assets",[]):
        relative=entry["path"];parts=relative.split('/')
        if not relative.startswith('assets/') or len(parts)>8 or any(not re.fullmatch(r'[A-Za-z0-9_-][A-Za-z0-9_.-]{0,127}',part) or part.endswith('.') for part in parts) or relative.casefold() in asset_paths:raise ValueError("Native system asset has an unsafe or duplicate path")
        asset_paths.add(relative.casefold());asset_bytes+=entry["bytes"]
        if len(asset_paths)>128 or not 0<entry['bytes']<=32*1024*1024 or asset_bytes>128*1024*1024:raise ValueError("Native system assets exceed their inventory bound")
        original=Path(entry['source']).resolve(strict=True)
        if not original.is_file() or original.is_symlink() or original.stat().st_size!=entry['bytes'] or hashlib.sha256(original.read_bytes()).hexdigest()!=entry['sha256']:raise ValueError("Native system asset differs from its build receipt")
        target=output.joinpath(*parts)
        if target.exists():raise ValueError("Native asset collides with another package file")
        target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(original,target)
    files = [{"path": f"native-runtimes/{spec['id']}/{path.name}",
              "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "bytes": path.stat().st_size}
             for path in sorted(output.iterdir()) if path.is_file()]
    files += [{"path":f"native-runtimes/{spec['id']}/{path.relative_to(output).as_posix()}","sha256":hashlib.sha256(path.read_bytes()).hexdigest(),"bytes":path.stat().st_size} for path in sorted((output/'assets').rglob('*')) if path.is_file()]
    if len(files)>256:raise ValueError("Native package exceeds its admitted file count")
    worker = f"native-runtimes/{spec['id']}/omniplay-native-worker.exe"
    package = {"manifest": {"schema_version": 1, "id": spec["id"], "version": spec["version"],
        "kind": "native_emulator", "systems": spec["systems"],
        "capabilities": ["pause", "save_ram", "save_state", "keyboard_mouse", "gamepad", "haptics", "performance_measurement"]+(["disc_control"] if args.core=="beetle-psx" else []),
        "platform": "windows-x86_64", "executable_sha256": next(f["sha256"] for f in files if f["path"] == worker),
        "state_format_version": f"{args.core}-{spec['revision'][:16]}-opns1-native64",
        "source_revision": spec["revision"], "license_expression": spec["license"],
        "distribution_approved": args.distribution_approved, "security_revoked": False},
        "worker": worker, "core_library": f"native-runtimes/{spec['id']}/{library.name}",
        "expected_library_name": spec["name"], "expected_library_version": spec["library_version"],
        "in_game_format_version": f"{args.core}-memory-envelope-v1-native64",
        "maximum_rom_bytes": spec["rom_bytes"], "memory_limit_mb": spec["memory_mb"],
        "files": files, "minimum_system_ram_bytes":4*1024*1024 if args.core=="melonds-ds" else 64*1024 if args.core=="ares-md" else 32*1024*1024 if args.core in {"ppsspp","ppsspp-gl"} else 0,
        "reset_after_ram_load":args.core in {"mupen64plus-next","mupen64plus-gl"},
        "video_backend":spec.get("video_backend","software"),"gpu_max_width":spec.get("gpu_max_width",0),"gpu_max_height":spec.get("gpu_max_height",0),
        "prepare_completion":spec.get("prepare_completion",False),"pause_barrier":spec.get("pause_barrier",False),"pause_barrier_protocol":spec.get("pause_barrier_protocol",0),
        "core_options": ({"beetle_psx_use_mednafen_memcard0_method":"libretro","beetle_psx_enable_memcard1":"disabled","beetle_psx_cpu_dynarec":"disabled","beetle_psx_skip_bios":"disabled"} if args.core=="beetle-psx" else {"melonds_console_mode":"ds","melonds_boot_mode":"direct","melonds_sysfile_mode":"native","melonds_firmware_nds_path":"firmware.bin","melonds_touch_mode":"touch","melonds_screen_layout1":"top-bottom","melonds_number_of_screen_layouts":"1","melonds_jit_enable":"disabled","melonds_render_mode":"software","melonds_mic_input":"silence","melonds_network_mode":"disabled","melonds_homebrew_sdcard":"disabled","melonds_dsi_sdcard":"disabled","melonds_slot2_device":"auto"} if args.core=="melonds-ds" else {}),
        "content_formats": (["raw_rom","cue","m3u","chd","iso","pbp"] if args.core=="beetle-psx" else ["raw_rom"]), "firmware_requirements": receipt.get("firmware", [])}
    if brokered:
        package['presentation']={'protocol':2,'executable':f"native-runtimes/{spec['id']}/omniplay-presentation-host.exe",'maximum_width':args.presentation_width,'maximum_height':args.presentation_height,'memory_limit_mb':args.presentation_memory_mb}
        package['manifest']['capabilities'].remove('performance_measurement')
    if args.core=="beetle-psx":
        policy_path=ROOT/"engines/policies/beetle-options.json"
        policy=validate_policy(json.loads(policy_path.read_text(encoding="utf-8")))
        definitions=policy["definitions"]
        package["user_core_options"]=definitions
        package["core_options"].update({entry["key"]:entry["default_value"] for entry in definitions})
        package.update({"filesystem_save_directory":"save/PSX","core_save_directory":"save/PSX","filesystem_save_memory":True,"file_io_protocol":1,"writable_files_protocol":1,"in_game_format_version":"beetle-psx-opnm1-cards1-native64"})
        package["manifest"]["state_format_version"]=f"beetle-psx-{spec['revision'][:16]}-om2-opns2-cards1-native64"
    if args.core=="ares-md":
        policy=validate_policy(json.loads((ROOT/'engines/policies/ares-md-options.json').read_text(encoding='utf-8')))
        package['user_core_options']=policy['definitions']
        package['core_options']={entry['key']:entry['default_value'] for entry in policy['definitions']}
        package.update({'filesystem_save_directory':'save/MD','core_save_directory':'save/MD','filesystem_save_memory':False,'file_io_protocol':1,'writable_files_protocol':1,'in_game_format_version':'ares-md-canonical-files-v1','portable_in_game_format':'ares-md-canonical-files-v1'})
        package['manifest']['state_format_version']=f"ares-md-{spec['revision'][:16]}-om2-opns2-sync-native64"
    if args.core=="mupen64plus-next":
        package["content_formats"]=["raw_rom","n64_z64","n64_v64","n64_n64"]
        package["use_scratch_system_directory"]=True
        package["core_options"]={"mupen64plus-cpucore":"pure_interpreter","mupen64plus-rdp-plugin":"angrylion","mupen64plus-rsp-plugin":"cxd4","mupen64plus-ThreadedRenderer":"False","mupen64plus-angrylion-multithread":"1","mupen64plus-angrylion-sync":"Low","mupen64plus-FrameDuping":"False",**{f"mupen64plus-pak{port}":"memory" for port in range(1,5)}}
    if args.core=="mupen64plus-gl":
        package["content_formats"]=["raw_rom","n64_z64","n64_v64","n64_n64"]
        package["use_scratch_system_directory"]=True
        package["core_options"]={"mupen64plus-cpucore":"cached_interpreter","mupen64plus-rdp-plugin":"gliden64","mupen64plus-rsp-plugin":"hle","mupen64plus-ThreadedRenderer":"False","mupen64plus-aspect":"4:3","mupen64plus-43screensize":"640x480","mupen64plus-EnableNativeResFactor":"1","mupen64plus-FrameDuping":"False",**{f"mupen64plus-pak{port}":"memory" for port in range(1,5)}}
    if args.core in {"ppsspp","ppsspp-gl"}:
        package.update({"filesystem_save_directory":"save/PSP/SAVEDATA","has_system_assets":True,"content_formats":["iso","cso","pbp","chd"],"in_game_format_version":"ppsspp-files-opsf2","core_options":{"ppsspp_cpu_core":"Interpreter","ppsspp_backend":"none","ppsspp_software_rendering":"enabled","ppsspp_internal_resolution":"480x272","ppsspp_enable_wlan":"disabled","ppsspp_enable_builtin_pro_ad_hoc_server":"disabled","ppsspp_fast_memory":"disabled","ppsspp_frameskip":"0","ppsspp_auto_frameskip":"disabled"}})
        package["manifest"]["state_format_version"]=f"ppsspp-{spec['revision'][:16]}-opns2-fs2-native64"
        package["portable_in_game_format"]="ppsspp-files-opsf2"
        if args.core=="ppsspp-gl":package["core_options"].update({"ppsspp_backend":"opengl","ppsspp_software_rendering":"disabled","ppsspp_cpu_core":"IR JIT"})
    # Catalog publication is a build artifact. Its integrity is covered by the signed
    # outer package inventory, not by an untrusted renderer's approval assertion.
    catalog_path = output.parent/"catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8")) if catalog_path.exists() else {"schema_version": 1, "runtimes": []}
    if any(item["manifest"]["id"] == spec["id"] for item in catalog["runtimes"]):
        raise ValueError("Native catalog already contains this runtime identity")
    catalog["runtimes"].append(package)
    catalog_path.write_text(json.dumps(catalog, indent=2)+"\n", encoding="utf-8")
    (build/"corresponding-source.json").write_text(json.dumps({"schema_version": 1,
        "core_source": receipt["corresponding_source"], "worker_source": str(ROOT/"engines/native"),
        "presentation_source":str(ROOT/"engines/presentation"),"presentation_protocol":2,"default_presentation_mode":"direct","selected_presentation_mode":args.presentation_mode,
        "windows_actor_kind":"device_free_software" if brokered else "desktop_sdl",
        "windows_actor_static_imports":actor_imports,
        "compiled_first_party_sources":compiled_sources,"worker_sha256":hashlib.sha256(workers[0].read_bytes()).hexdigest(),"presentation_sha256":hashlib.sha256(helpers[0].read_bytes()).hexdigest()if helpers else None,
        "dependencies": PINS, "build_recipe": str(Path(__file__).resolve()),
        "core_dependency_sources":receipt.get("dependency_sources",{}),"core_source_lock":receipt.get("source_lock"),
        "core_asset_source":receipt.get("asset_corresponding_source"),"core_asset_build_receipt":receipt.get("asset_build_receipt"),
        "verification": "not_run"}, indent=2)+"\n", encoding="utf-8")
    print("Native worker package authored. Runtime and sandbox verification remain pending.")


if __name__ == "__main__":
    main()
