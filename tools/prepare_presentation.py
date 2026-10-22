"""Freeze presentation-host and selected pinned SDL source, without building it.

Only Git metadata/index reads, ordinary source copies and SHA-256 inventories run.
No CMake configuration, compiler, SDK, library, helper, OS API or test is started.
"""
import argparse
import hashlib
import json
from pathlib import Path,PurePosixPath
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
SDL_REVISION='829a65d769d935c4852f8159e964312c0957260a'
JSON_REVISION='55f93686c01528224f448c19128836e7df245f72'
SAMEBOY_REVISION='26a56bd6a122a6318a456abd80558408742d5adf'
MAX_FILES=20000
MAX_BYTES=512*1024*1024

def ordinary(path,base,file=False):
    if not path.resolve().is_relative_to(base.resolve()):raise ValueError('Presentation source must remain in its declared root')
    for parent in [path,*path.parents]:
        if parent.is_symlink() or getattr(parent,'is_junction',lambda:False)():raise ValueError('Presentation source ancestry cannot contain links')
        if parent==base:break
    if file and not path.is_file():raise ValueError('Presentation source needs an ordinary file')
    return path

def encoded(value):return (json.dumps(value,ensure_ascii=False,sort_keys=True,indent=2)+'\n').encode('utf-8')

def digest(path):
    result=hashlib.sha256()
    with path.open('rb')as source:
        for block in iter(lambda:source.read(256*1024),b''):result.update(block)
    return result.hexdigest()

def dependency_inputs(base,selected,target,expected):
    ordinary(base,ROOT);revision=subprocess.run(['git','-C',str(base),'rev-parse','HEAD'],check=True,capture_output=True,text=True).stdout.strip()
    if revision!=expected:raise ValueError('Presentation needs its declared pinned dependency source revision')
    raw=subprocess.run(['git','-C',str(base),'ls-files','--stage','-z','--',*selected],check=True,capture_output=True).stdout
    if len(raw)>8*1024*1024:raise ValueError('A presentation dependency source index exceeds its budget')
    inputs=[]
    for item in raw.split(b'\0'):
        if not item:continue
        entry,name=item.split(b'\t',1);mode,blob,stage=entry.decode('ascii').split();path=PurePosixPath(name.decode('utf-8'))
        if mode not in {'100644','100755'}or stage!='0'or path.is_absolute()or any(part in {'','..','.'}for part in path.parts):raise ValueError('Dependency inputs must be ordinary tracked source files')
        inputs.append((ordinary(base.joinpath(*path.parts),base,True),PurePosixPath(target)/path,{'git_blob':blob,'git_mode':mode}))
    if not inputs:raise ValueError('A required presentation dependency source selection is empty')
    return inputs

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--project',type=Path,required=True);parser.add_argument('--sdl-source',type=Path,required=True);args=parser.parse_args()
    project=args.project.absolute();sdl=args.sdl_source.absolute();ordinary(project,ROOT);ordinary(sdl,ROOT)
    if not project.resolve().is_relative_to((ROOT/'out').resolve())or project.exists():raise ValueError('Use a fresh presentation project in workspace out')
    revision=subprocess.run(['git','-C',str(sdl),'rev-parse','HEAD'],check=True,capture_output=True,text=True).stdout.strip()
    if revision!=SDL_REVISION:raise ValueError('Presentation needs the declared pinned SDL source revision')
    selected=['CMakeLists.txt','LICENSE.txt','VERSION.txt','cmake','include','src','build-scripts']
    index=subprocess.run(['git','-C',str(sdl),'ls-files','--stage','-z','--',*selected],check=True,capture_output=True).stdout
    if len(index)>8*1024*1024:raise ValueError('The selected SDL source index exceeds its inventory budget')
    inputs=[]
    for item in index.split(b'\0'):
        if not item:continue
        entry,name=item.split(b'\t',1);mode,blob,stage=entry.decode('ascii').split();relative=name.decode('utf-8');path=PurePosixPath(relative)
        if mode not in {'100644','100755'}or stage!='0'or path.is_absolute()or any(part in {'','..','.'}for part in path.parts):raise ValueError('Selected SDL input must be an ordinary tracked source file')
        inputs.append((ordinary(sdl/relative,sdl,True),PurePosixPath('SDL')/path,{'git_blob':blob,'git_mode':mode}))
    if not any(target.as_posix()=='SDL/CMakeLists.txt'for _,target,_ in inputs)or not any(target.as_posix()=='SDL/LICENSE.txt'for _,target,_ in inputs):raise ValueError('The selected SDL source is incomplete')
    inputs.extend(dependency_inputs(ROOT/'third_party/sources/nlohmann-json',['CMakeLists.txt','cmake','include','single_include','LICENSE.MIT','nlohmann_json.natvis'],'json',JSON_REVISION))
    inputs.extend(dependency_inputs(ROOT/'third_party/sources/sameboy',['libretro/libretro.h','LICENSE'],'interface',SAMEBOY_REVISION))
    for name in ['CMakeLists.txt','host.manifest','macos-presentation.entitlements','src/wire.hpp','src/wire.cpp','src/client.hpp','src/client.cpp','src/owner.hpp','src/owner.cpp','src/main.cpp']:inputs.append((ordinary(ROOT/'engines/presentation'/name,ROOT/'engines/presentation',True),PurePosixPath('presentation')/name,{}))
    native=ROOT/'engines/native'
    for path in[native/'CMakeLists.txt',native/'worker.manifest',native/'macos-actor.entitlements',*sorted((native/'src').rglob('*'))]:
        if not path.is_file():continue
        ordinary(path,native,True)
        if path.suffix not in {'.cpp','.hpp','.h','.in','.txt','.manifest','.entitlements'}:raise ValueError('Unexpected native frontend source input')
        inputs.append((path,PurePosixPath('native')/path.relative_to(native).as_posix(),{}))
    mac_owner=ROOT/'engines/macos-owner'
    for path in sorted(mac_owner.rglob('*')):
        if path.is_file():inputs.append((ordinary(path,mac_owner,True),PurePosixPath('macos-owner')/path.relative_to(mac_owner).as_posix(),{}))
    for name in ['sandbox','native-runtime']:
        base=ROOT/'platform'/name
        for path in[base/'Cargo.toml',*sorted((base/'src').rglob('*.rs'))]:inputs.append((ordinary(path,base,True),PurePosixPath('broker/platform')/name/path.relative_to(base).as_posix(),{}))
    isolation=ROOT/'engines/isolation-linux'
    for path in sorted(isolation.iterdir()):
        if not path.is_file():continue
        ordinary(path,isolation,True)
        if path.name!='CMakeLists.txt'and path.suffix not in {'.c','.h'}:raise ValueError('Unexpected Linux isolation source input')
        inputs.append((path,PurePosixPath('isolation-linux')/path.name,{}))
    for name in ['LINUX_ISOLATION_POLICY.md','LINUX_ISOLATION_BOOTSTRAP.md','LINUX_RUNTIME_LAUNCH.md','LINUX_PRESENTATION_PAIR.md','LINUX_PROFILE_DELEGATION.md','LINUX_HOST_PROVISIONING.md','LINUX_WORK_JOURNAL.md','LINUX_IMAGE_PACKAGE.md','NATIVE_WORK_STORAGE.md','LINUX_INPUT_STAGING.md','LINUX_SESSION_COMPOSITION.md','LINUX_RUNTIME_ADAPTER.md','LINUX_ABANDONED_RECOVERY.md','LINUX_RESUMABLE_RETIREMENT.md','LINUX_CREATOR_RECOVERY.md','MACOS_NATIVE_CONTAINMENT.md','NATIVE_PRESENTATION_HOST.md','NATIVE_PRESENTATION_CLIENT.md','NATIVE_PRESENTATION_FRONTEND.md','NATIVE_PRESENTATION_LAUNCH.md']:
        inputs.append((ordinary(ROOT/'docs'/name,ROOT,True),PurePosixPath('docs')/name,{}))
    for name in ['Cargo.toml','Cargo.lock','platform/core/src/sessions.rs','platform/contracts/src/session.rs','platform/broker/Cargo.toml','platform/broker/src/main.rs','platform/broker/src/linux_host_main.rs','packaging/linux/systemd/user/omniplay-native-host.service','packaging/linux/systemd/user/omniplay-native-host.socket','tools/install_user_release_posix.py','tools/unregister_user_release_posix.py','tools/replace_user_bootstrap_posix.py','apps/game-hub/native-player.js']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath('broker')/name,{}))
    for path in sorted((ROOT/'platform/broker/src').rglob('*.rs')):
        if path.name in {'main.rs','linux_host_main.rs'}:continue
        inputs.append((ordinary(path,ROOT,True),PurePosixPath('broker')/path.relative_to(ROOT).as_posix(),{}))
    inputs.append((ordinary(ROOT/'tools/build_native_runtime.py',ROOT,True),PurePosixPath('tools/build_native_runtime.py'),{}))
    for name in ['platform/updater/Cargo.toml','platform/updater/src/package.rs','platform/updater/src/launcher.rs']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath('broker')/name,{}))
    for name in ['platform/core/Cargo.toml','platform/core/src/lib.rs','platform/core/src/database.rs','platform/core/src/native_work_storage.rs','platform/core/src/diagnostics.rs']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath('broker')/name,{}))
    for path in sorted((ROOT/'platform/core/migrations').glob('*.sql')):
        inputs.append((ordinary(path,ROOT,True),PurePosixPath('broker')/path.relative_to(ROOT).as_posix(),{}))
    inputs.append((ordinary(ROOT/'tools/package_release.py',ROOT,True),PurePosixPath('tools/package_release.py'),{}))
    inputs.append((ordinary(ROOT/'tools/author_macos_runtime_catalog.py',ROOT,True),PurePosixPath('tools/author_macos_runtime_catalog.py'),{}))
    for name in ['WINDOWS_NATIVE_COMPLETION.md','WINDOWS_NATIVE_MAINTENANCE.md','WINDOWS_PRELAUNCH_RECOVERY.md','WINDOWS_DEFAULT_STORAGE.md','WINDOWS_REGISTRY_STORAGE.md','NATIVE_MEMORY_FILES.md','PS1_MEMORY_CARDS.md','NATIVE_PS1_SAVES.md','NATIVE_CONTAINER_RETIREMENT.md']:
        inputs.append((ordinary(ROOT/'docs'/name,ROOT,True),PurePosixPath('docs')/name,{}))
    inputs.append((ordinary(ROOT/'qa/windows/NATIVE_ACCEPTANCE.md',ROOT,True),PurePosixPath('qa/windows/NATIVE_ACCEPTANCE.md'),{}))
    for name in ['qa/windows/storage-probe.cpp','qa/windows/STORAGE_PROBE.md']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['qa/native-audio/CMakeLists.txt','qa/native-audio/audio_ring_cases.cpp','qa/native-audio/ACCEPTANCE.md']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['docs/NATIVE_GUEST_PAUSE.md','qa/native-lifecycle/CMakeLists.txt','qa/native-lifecycle/fixture_core.cpp','qa/native-lifecycle/run_cases.py','qa/native-lifecycle/ACCEPTANCE.md','tools/build_ppsspp.py']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['docs/NATIVE_CONTROL_BOUNDARY.md','qa/native-lifecycle/wire_cases.py','qa/native-lifecycle/temporary_directory_cases.py']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['docs/NATIVE_CONTROL_WORKERS.md','qa/native-lifecycle/CONTROL_WORKERS.md']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    inputs.append((ordinary(ROOT/'docs/WINDOWS_ITEM2_CLOSURE.md',ROOT,True),PurePosixPath('docs/WINDOWS_ITEM2_CLOSURE.md'),{}))
    inputs.append((ordinary(ROOT/'docs/QA_EXECUTION_POLICY.md',ROOT,True),PurePosixPath('docs/QA_EXECUTION_POLICY.md'),{}))
    for name in ['qa/native-memory/CMakeLists.txt','qa/native-memory/memory_files_cases.cpp','qa/native-memory/ACCEPTANCE.md','engines/ares-md/adapter.cpp','engines/ares-md/web-files.hpp','engines/ares-md/save-memory.inc','tools/build_ares_md.py','tools/build_ares_md_web.py']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['beetle-save-io.c.inc','beetle-memory-files.c','beetle-memory-files.h','beetle-memory-frontio.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['qa/ps1-memory/CMakeLists.txt','qa/ps1-memory/ps1_memory_cases.cpp','qa/ps1-memory/ACCEPTANCE.md','tools/build_beetle_psx.py','tools/build_beetle_psx_web.py','tools/psx_source_adaptations.py','engines/policies/beetle-options.json']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['n64-memory-io.h','n64-memory-io.cpp','n64-memory-streams.hpp','n64-memory-gzip.h','n64-memory-gzip.cpp','n64-memory-text.hpp','n64-shader-cache.hpp','n64-shader-storage.cpp.inc','n64-fiber-win.c.inc','n64-memory-path.hpp','n64-texture-cache.hpp','n64-texture-memory.cpp.inc','n64-texture-storage.cpp.inc','n64-texture-pool.hpp','n64-texture-pool.cpp','n64-texture-pool-core.cpp.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['n64-memory-png.h','n64-memory-png.c','n64-texture-dump.cpp.inc','n64-texture-debug.cpp.inc','n64-native-log.cpp.inc','n64-save-files.h','n64-save-files.cpp','n64-file-storage.c.inc','n64-util-save.c.inc','n64-transferpak-ram.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['n64-dd-storage.h','n64-dd-storage.c','n64-dd-format.c.inc','n64-dd-geometry.c.inc','n64-dd-main.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['n64-core-owner.h','n64-core-owner.cpp','n64-core-barrier.c.inc','n64-main-storage-owner.c.inc','n64-gb-storage-owner.h','n64-snapshot-lifecycle.c.inc','n64-snapshot-bounds.h','n64-event-queue.c.inc','n64-tlb-memory.c.inc','n64-gb-storage-access.c.inc','n64-mbc3-rtc.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['qa/n64-memory/owner_cases.cpp','qa/n64-memory/main_owner_fixture.c','qa/n64-memory/owner_fiber_cases.cpp','qa/n64-memory/event_queue_fixture.c','qa/n64-memory/snapshot_bounds_cases.cpp','qa/n64-memory/tlb_fixture.c','qa/n64-memory/tlb_snapshot_cases.cpp','qa/n64-memory/gb_transferpak_cases.cpp']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['qa/n64-memory/mbc3_rtc_cases.cpp','docs/N64_SNAPSHOT_FORMAT.md']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['n64-rtc-storage.h','n64-rtc-storage.c','n64-gb-rtc-bind.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['qa/n64-memory/rtc_storage_cases.cpp','docs/N64_RTC_BATTERY.md']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['n64-device-memory.h','n64-flash-memory.c.inc','n64-si-copy.c.inc','n64-dd-c2.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['qa/n64-memory/peripheral_memory_cases.cpp','qa/n64-memory/si_memory_fixture.c','qa/n64-memory/dd_c2_fixture.c','docs/N64_DEVICE_MEMORY.md']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['n64-pi-memory.c.inc','n64-framebuffer-memory.c.inc','n64-framebuffer-mapping.c.inc','n64-framebuffer-cache.hpp']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['qa/n64-memory/pi_framebuffer_fixture.h','qa/n64-memory/pi_framebuffer_fixture.c','qa/n64-memory/pi_framebuffer_cases.cpp']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['n64-dma-memory.h','n64-sram-memory.c.inc','n64-cart-rom-memory.c.inc','n64-dd-dma-memory.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    for name in ['qa/n64-memory/cartridge_dma_fixture.h','qa/n64-memory/cartridge_dma_fixture.c','qa/n64-memory/cartridge_dma_cases.cpp']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['n64-color-memory.hpp','n64-color-buffer.cpp.inc','n64-color-upload.cpp.inc','n64-rom-image.h','n64-content-load.c.inc','n64-audio-lifecycle.c.inc','n64-audio-output.h','n64-audio-output.c.inc','n64-ai-memory.c.inc','n64-audio-source.c.inc','n64-ai-timing.h','n64-ai-dma.c.inc','n64-sinc-capacity.h','n64-sinc-checked.c.inc','n64-audio-sinc.cpp.inc','n64-ai-fifo.c.inc','n64-event-insert.c.inc']:
        inputs.append((ordinary(ROOT/'engines/native/adapters'/name,ROOT,True),PurePosixPath('native/adapters')/name,{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/color_memory_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/color_memory_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/rom_image_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/rom_image_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/content_load_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/content_load_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/audio_lifecycle_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/audio_lifecycle_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/audio_output_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/audio_output_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/audio_source_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/audio_source_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/ai_timing_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/ai_timing_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/sinc_capacity_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/sinc_capacity_cases.cpp'),{}))
    inputs.append((ordinary(ROOT/'qa/n64-memory/sinc_actual_cases.cpp',ROOT,True),PurePosixPath('qa/n64-memory/sinc_actual_cases.cpp'),{}))
    for name in ['qa/n64-memory/event_insert_fixture.c','qa/n64-memory/event_insert_cases.cpp']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['qa/n64-memory/png_cases.cpp','qa/n64-memory/diagnostic_cases.cpp','qa/n64-memory/save_storage_cases.cpp','qa/n64-memory/dd_format_fixture.c','qa/n64-memory/dd_storage_cases.cpp']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['docs/N64_MEMORY_IO.md','qa/n64-memory/CMakeLists.txt','qa/n64-memory/n64_memory_cases.cpp','qa/n64-memory/fiber_fixture.c','qa/n64-memory/fiber_cases.cpp','qa/n64-memory/texture_core_cases.cpp','qa/n64-memory/texture_fixture.cpp','qa/n64-memory/texture_pool_core_fixture.cpp','qa/n64-memory/texture_pool_cases.cpp','qa/n64-memory/ACCEPTANCE.md','tools/prepare_n64_memory_io.py','tools/prepare_n64_memory_core.py','tools/n64_source_adaptations.py','tools/build_mupen64plus.py','tools/pinned_core_source.py']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath(name),{}))
    for name in ['platform/contracts/src/lib.rs','platform/contracts/src/ipc.rs','platform/contracts/src/native_work.rs','apps/game-hub/app.js','apps/game-hub/background.js','apps/game-hub/component-ownership.js','apps/game-hub/native-work-maintenance.js']:
        inputs.append((ordinary(ROOT/name,ROOT,True),PurePosixPath('broker')/name,{}))
    inputs.append((ordinary(Path(__file__).resolve(),ROOT,True),PurePosixPath('tools/prepare_presentation.py'),{}))
    if len(inputs)>MAX_FILES:raise ValueError('Presentation source file count exceeds its budget')
    source=project/'source';source.mkdir(parents=True);inventory=[];total=0;targets=set()
    for path,target,metadata in inputs:
        name=target.as_posix();size=path.stat().st_size;total+=size
        if name in targets or size>32*1024*1024 or total>MAX_BYTES:raise ValueError('Presentation source inventory exceeds its bound or collides')
        targets.add(name);destination=source.joinpath(*target.parts);destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,destination)
        inventory.append({'path':name,'bytes':size,'sha256':digest(destination),**metadata})
    receipt={'schema_version':4,'source_only':True,'protocol':2,'owner_protocol':1,'sdl_revision':revision,'json_revision':JSON_REVISION,'libretro_header_source_revision':SAMEBOY_REVISION,'sdl_selected_roots':selected,'checkout_bytes_are_git_blob_bytes':False,'source_inventory':sorted(inventory,key=lambda value:value['path']),'source_files':len(inventory),'source_bytes':total,'configuration':'not_started','compiler':'not_started','helper':'not_started','verification':'not_run','core_frontend_source_authored':True,'broker_launch_source_authored':True,'broker_launch_source_targets':['windows-x86_64'],'linux_owned_pair_source_authored':True,'linux_lower_level_launch_source_target':'linux-x86_64-gnu','linux_registry_integration':False,'native_actor_integration_source_authored':True,'native_actor_integration':False,'product_enabled':False,'distribution_approved':False,'qualified_targets':[],'intended_primary_targets':['windows-x86_64','macos-aarch64','linux-x86_64'],'remaining':'actual helper/package/SDK and final Windows pair qualification; Linux registry/adapter/package/cache/staging/delegation and durable pair/work integration, compositor activation; macOS containment/lifecycle; hardware presentation/measurement; dependency/driver/source-license and every original product release requirement'}
    receipt.update({'linux_profile_delegation_owner_source_authored':True,'linux_durable_pair_work_journal_source_authored':True,'host_delegation_provisioning':False,'abandoned_linux_work_recovery':False,'remaining':'actual Windows helper/package/SDK pair qualification; Linux product registry/adapter and authenticated full ELF/dependency/cache/staging, host delegation provisioning/migration, work existing-tree crash recovery/reclamation and compositor activation; macOS containment/lifecycle; hardware presentation/measurement; dependency/driver/source-license and every original complete product/release requirement'})
    receipt.update({'linux_authenticated_catalog_image_source_authored':True,'linux_sealed_dependency_closure_source_authored':True,'linux_shared_profile_image_cache_source_authored':True,'unix_readonly_unpack_source_authored':True,'linux_product_registry_execution':False,'remaining':'actual Windows package/SDK pair qualification; Linux product adapter/private game/firmware/save staging/control/final-save, host delegation provisioning/migration/whole-profile accounting, existing-tree crash recovery/reclamation and compositor activation; macOS containment/lifecycle; hardware presentation/measurement; compiled dependency/SDK/driver/source-license and every original product/release requirement'})
    receipt.update({'database_source_schema':15,'native_copy_debt_source_authored':True,'windows_copy_quota_integration_source_authored':True,'linux_game_firmware_staging_source_authored':True,'linux_staged_input_gate_source_authored':True,'remaining':'actual Windows package/SDK/process/copy-debt qualification; Linux product registry/adapter/control/save/final-save, actual host delegation provisioning/migration/whole-profile accounting, existing-tree crash recovery/confirmed work reclamation and compositor activation; macOS containment/lifecycle; hardware presentation/measurement; compiled dependency/SDK/driver/source-license and every original product/release requirement'})
    receipt.update({'linux_owned_session_composition_source_authored':True,'linux_core_prepare_lifecycle_save_source_authored':True,'linux_runtime_factory_adapter_selected':False,'remaining':'actual Windows package/SDK/process/copy-debt qualification; full Linux product factory/adapter final-save/metrics/disc/close, host delegation provisioning/migration/whole-profile physical accounting, existing-tree crash recovery/confirmed reclamation and compositor activation; macOS containment/lifecycle; hardware presentation/measurement; compiled dependency/SDK/core/driver/license/device and every original product/release requirement'})
    receipt.update({'linux_runtime_adapter_source_authored':True,'linux_explicit_factory_source_authored':True,'linux_default_broker_factory_selected':False,'linux_host_provisioning_source_complete':False,'remaining':'actual Windows package/SDK/process/copy-debt qualification; legitimate Linux host delegation provisioning/migration and normal broker explicit factory selection, existing-tree crash recovery/confirmed work/debt/charge reclamation, compositor activation/whole-profile physical accounting; macOS containment/lifecycle; hardware presentation/measurement; compiled dependency/SDK/core/driver/license/device and every original product/release requirement'})
    receipt.update({'linux_explicit_ended_pair_recovery_source_authored':True,'linux_recovery_signals_disk_pids':False,'linux_partial_creation_recovery_complete':False,'linux_resumable_work_deletion_complete':False,'remaining':'actual Windows package/SDK/process/copy-debt qualification; Linux legitimate host provisioning/migration/startup factory/recovery selection, partial creation/launch recovery/resumable work deletion/fair collector, compositor activation/whole-profile physical accounting; macOS containment/lifecycle; hardware presentation/measurement; compiled SDK/core/driver/license/device and every original product/release requirement'})
    receipt.update({'linux_external_confirmed_retirement_source_authored':True,'linux_confirmed_work_deletion_resume_source_authored':True,'linux_recovery_rotating_cursor_source_authored':True,'linux_partial_origin_recovery_complete':False,'remaining':'actual Windows package/SDK/process/copy-debt qualification; Linux legitimate host provisioning/migration/startup factory/recovery selection, partial-origin recovery, compositor activation/whole-profile physical accounting; macOS containment/lifecycle; hardware presentation/measurement; compiled SDK/core/driver/license/device and every original product/release requirement'})
    receipt.update({'linux_work_ownership_source_schema':2,'linux_creator_bound_unacknowledged_launch_source_authored':True,'linux_missing_group_identity_recovery_complete':False})
    receipt.update({'linux_native_service_startup_source_authored':True,'linux_socket_activation_source_authored':True,'linux_broker_profile_fd_handoff_source_authored':True,'linux_broker_factory_selection_source_authored':True,'linux_empty_partial_host_tree_repair_source_authored':True,'linux_per_user_installer_source_authored':True,'linux_safe_registration_removal_source_authored':True,'linux_resumable_bootstrap_replacement_source_authored':True,'linux_quiescent_service_upgrade_source_authored':True,'linux_host_provisioning_source_complete':False,'linux_default_broker_factory_selected':False,'product_enabled':False,'remaining':'actual Windows package/SDK/process/copy-debt qualification; Linux active-tree policy migration, optional program purge and actual systemd/socket/cgroup/broker qualification, compositor activation/whole-profile physical accounting; macOS containment/lifecycle; compiled SDK/core/driver/license/device and every original product/release requirement plus final verification'})
    receipt.update({'macos_closed_sandbox_policy_source_authored':True,'macos_authenticated_xpc_endpoint_source_authored':True,'macos_bounded_session_staging_source_authored':True,'macos_session_launch_source_authored':False,'macos_product_enabled':False})
    receipt.update({'windows_creation_time_job_association_source_authored':True,'windows_serialized_container_lifecycle_source_authored':True,'windows_external_confirmed_retirement_source_authored':True,'windows_recovery_rotating_cursor_source_authored':True,'windows_verified':False})
    receipt.update({'windows_background_recovery_owner_source_authored':True,'windows_settings_recovery_source_authored':True,'windows_document_owned_recovery_jobs_source_authored':True})
    receipt.update({'windows_creation_owner_lease_handoff_source_authored':True,'windows_prelaunch_protocol_source':1,'windows_ledger_only_recovery_source_authored':True,'windows_legacy_prelaunch_promotion':False,'windows_prelaunch_recovery_verified':False})
    receipt.update({'windows_default_filesystem_storage_sealing_source_authored':True,'windows_actual_actor_token_admission_source_authored':True,'windows_storage_probe_source_authored':True,'windows_default_storage_verified':False,'windows_physical_disk_quota_complete':False})
    receipt.update({'windows_private_registry_sealing_source_authored':True,'windows_single_use_actor_registry_lookup_source_authored':True,'windows_registry_exact_sid_grant_source_authored':True,'windows_pre_resume_registry_then_filesystem_source_authored':True,'windows_storage_probe_source_protocol':2,'windows_private_registry_verified':False})
    receipt.update({'native_memory_files_protocol_source':1,'native_memory_files_quota_source_authored':True,'native_memory_save_capture_replace_source_authored':True,'windows_md_memory_files_source_authored':True,'windows_md_actor_os_save_scratch_write_grants':False,'native_memory_files_verified':False,'all_native_core_writes_bounded':False})
    receipt.update({'windows_ps1_memory_cards_source_authored':True,'windows_ps1_card_swap_writer_drain_source_authored':True,'windows_ps1_read_failure_retention_source_authored':True,'windows_ps1_actor_os_save_scratch_write_grants':False,'windows_ps1_memory_cards_verified':False})
    receipt.update({'n64_memory_stdio_stream_gzip_source_authored':True,'n64_memory_io_verified':False,'n64_core_memory_io_integration_complete':False,'n64_package_admission_changed':False,'n64_actor_os_write_grants_removed':False})
    receipt.update({'n64_configuration_database_shader_source_integrated':True,'n64_complete_pre_gl_cache_admission_source_authored':True,'n64_owned_windows_fiber_retirement_source_authored':True,'n64_configuration_shader_verified':False})
    receipt.update({'n64_texture_gzip_file_cache_source_integrated':True,'n64_texture_cache_verified':False,'n64_texture_dump_and_peripheral_closure_complete':False})
    receipt.update({'n64_bounded_texture_pool_source_integrated':True,'n64_truthful_filter_initialization_source_authored':True,'native_actor_unconfirmed_uninstall_process_exit_source_authored':True,'n64_texture_pool_verified':False})
    receipt.update({'n64_scratch_png_texture_dump_source_integrated':True,'n64_bounded_diagnostics_source_integrated':True,'n64_png_codec_allocation_limit_bytes':8388608,'n64_png_dump_verified':False})
    receipt.update({'n64_persistent_file_storage_transferpak_source_integrated':True,'n64_persistent_storage_verified':False})
    receipt.update({'n64_dd_storage_format_geometry_ipl_source_integrated':True,'n64_dd_storage_verified':False})
    receipt.update({'n64_owner_prepare_pause_snapshot_checked_flush_source_integrated':True,'native_restore_callback_detach_acknowledgement_source_authored':True,'n64_owner_capture_verified':False})
    receipt.update({'n64_snapshot_queue_pif_pointer_preflight_dd_id_source_integrated':True,'n64_snapshot_semantic_validation_complete':False,'n64_snapshot_bounds_verified':False})
    receipt.update({'n64_tlb_snapshot_fields_lut_finite_page_walk_source_integrated':True,'n64_tlb_random_index_guard_source_integrated':True,'n64_tlb_snapshot_verified':False})
    receipt.update({'n64_transferpak_snapshot_selectors_gb_storage_camera_source_integrated':True,'n64_full_width_gb_ram_and_rumble_bank_mask_source_integrated':True,'n64_gb_transferpak_verified':False})
    receipt.update({'n64_mbc3_rtc_bounded_time_halt_carry_latched_reads_source_integrated':True,'n64_m64p_snapshot_write_version':'1.10','n64_m64p_snapshot_read_versions':'1.0-1.10','n64_mbc3_rtc_verified':False})
    receipt.update({'n64_rtc_battery_staged_boot_offline_time_checked_ram_retirement_source_integrated':True,'n64_rtc_battery_record_bytes':32,'n64_rtc_battery_verified':False})
    receipt.update({'n64_flash_si_c2_memory_snapshot_device_selectors_source_integrated':True,'n64_device_memory_verified':False})
    receipt.update({'n64_pi_full_dram_framebuffer_ranges_fixed_graphics_cache_source_integrated':True,'n64_pi_framebuffer_verified':False})
    receipt.update({'n64_sram_cart_rom_dd_direct_dma_complete_ranges_source_integrated':True,'n64_cartridge_dma_verified':False})
    receipt.update({'n64_fixed_color_dirty_bitmap_bounded_pixel_copy_source_integrated':True,'n64_color_memory_verified':False})
    receipt.update({'n64_rom_image_admission_unaligned_conversion_source_integrated':True,'n64_rom_image_verified':False})
    receipt.update({'n64_frontend_content_rom_receipt_failed_header_close_source_integrated':True,'n64_content_load_verified':False})
    receipt.update({'n64_audio_checked_partial_allocation_frontend_readiness_source_integrated':True,'n64_audio_lifecycle_verified':False})
    receipt.update({'n64_finite_audio_output_queue_root_backpressure_source_integrated':True,'n64_audio_output_verified':False})
    receipt.update({'n64_audio_callback_borrow_root_reentry_retirement_source_integrated':True,'n64_audio_callback_retirement_verified':False})
    receipt.update({'n64_ai_complete_ram_publication_immutable_backend_copy_source_integrated':True,'n64_audio_source_verified':False})
    receipt.update({'n64_ai_checked_duration_fifo_delayed_carry_source_integrated':True,'n64_ai_timing_verified':False})
    receipt.update({'n64_sinc_phase_prewrite_output_capacity_source_integrated':True,'n64_sinc_capacity_verified':False})
    receipt.update({'n64_sinc_bounded_filter_allocation_instance_dispatch_source_integrated':True,'n64_actual_sinc_verified':False})
    receipt.update({'n64_ai_staged_fifo_publication_source_integrated':True,'n64_ai_fifo_verified':False})
    receipt.update({'n64_ai_event_pool_insertion_acknowledgement_source_integrated':True,'n64_ai_event_verified':False})
    receipt.update({'n64_failed_stopped_guest_peripheral_flush_recovery_source_integrated':True,'n64_stopped_save_recovery_verified':False})
    receipt.update({'native_graphics_callback_release_retention_source_authored':True,'native_graphics_callback_release_verified':False})
    receipt.update({'native_graphics_reset_callback_lifetime_source_authored':True,'native_graphics_reset_callback_verified':False})
    receipt.update({'native_audio_actual_enqueue_receipt_n64_batch_cap_source_authored':True,'native_audio_enqueue_receipt_verified':False})
    receipt.update({'native_audio_sink_commit_and_fault_stop_source_authored':True,'presentation_atomic_audio_batch_and_media_generation_source_authored':True,'native_audio_device_control_receipts_source_authored':True,'native_audio_sink_commit_verified':False})
    receipt.update({'native_checked_guest_pause_source_protocol':1,'native_pause_and_retirement_unknown_lifetime_source_authored':True,'windows_owned_actor_no_dll_detach_source_authored':True,'native_lifecycle_actual_worker_fixture_source_authored':True,'native_checked_pause_and_retirement_verified':False})
    receipt.update({'native_core_api_unknown_exception_guard_source_authored':True,'native_public_ready_phase_admission_source_authored':True})
    receipt.update({'native_control_header_before_binary_source_authored':True,'native_control_single_outstanding_payload_source_authored':True,'native_broker_closed_response_and_channel_invalidation_source_authored':True,'native_core_exception_before_host_buffer_unwind_source_authored':True,'native_control_boundary_verified':False})
    receipt.update({'native_control_worker_source_slot_limit':16,'native_control_worker_retained_handle_and_prelaunch_reservation_source_authored':True,'windows_control_thread_os_completion_before_join_source_authored':True,'native_control_worker_retirement_verified':False})
    receipt.update({'native_recovery_worker_process_wide_source_limit':1,'native_recovery_worker_retained_across_broker_replacement_source_authored':True,'native_recovery_worker_retirement_verified':False,'windows_item2_source_complete':False})
    receipt.update({'windows_work_retention_reasons_source_authored':True,'windows_work_retention_reasons_verified':False,'assigned_test_executor':'antigravity','result_review_and_fix_owner':'human_and_codex'})
    receipt.update({'windows_recovery_diagnostic_preview_and_granted_export_source_authored':True,'windows_recovery_diagnostic_export_verified':False})
    (project/'preparation-receipt.json').write_bytes(encoded(receipt));(project/'recipe.json').write_bytes(encoded({'schema_version':4,'source_only':True,'configuration':'not_started','source':'source/presentation','sdl_source':'source/SDL','build_directory':'build','targets':['omniplay-presentation-host','omniplay-presentation-client'],'native_frontend':{'source':'source/native','presentation_source':'source/presentation','json_source':'source/json','libretro_include':'source/interface/libretro','core_binary_identity':'explicit compiled core receipt and its runtime identity/filename required','default_mode':'direct','brokered_mode':'private inherited pipes and session under --presentation-client-v2'},'linux_bootstrap':{'source':'source/isolation-linux','separate_cmake_project':True,'target':'omniplay-linux-bootstrap','linkage':'static-pie','toolchain':'qualified Linux static libc/runtime/sysroot required','trusted_ui':'dynamic PIE without RPATH; qualified desktop/driver dependencies required','registry_integration':False,'image_package_source':'source/broker/platform/native-runtime/src/linux_package.rs','shared_cache_source':'source/broker/platform/native-runtime/src/linux_image_cache.rs','input_staging_source':'source/broker/platform/native-runtime/src/linux_staging.rs'},'broker_source_selection':{'source':'source/broker','complete_workspace':False,'requires':'the complete product Cargo workspace and qualified dependencies/SDK for actual broker builds','authored_target':'windows-x86_64','linux_lower_level_owned_pair_source_authored':True,'database_source_schema':15,'catalog':'optional signed presentation protocol 2; source alone enables no runtime'},'generator':'Ninja','compiler_and_sdk':'explicit qualified target toolchain required after all product authoring','tests':'not_run'}))
    print('Froze presentation and selected SDL source. No build, helper, OS API or test was started.')

if __name__=='__main__':main()
