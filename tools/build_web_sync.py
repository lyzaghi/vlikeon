"""Freeze browser crypto source, or explicitly build that prepared source later.

Preparation copies source and writes provenance only. It does not invoke Cargo,
rustc, target installation, a browser, a WASM engine, an application or a test.
The separate --build-prepared path is reserved for after all product authoring.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib
from web_sync_artifact import inspect

ROOT=Path(__file__).resolve().parents[1]
SOURCE_LIMIT=256*1024*1024
def sha(path):
    digest=hashlib.sha256()
    with path.open('rb')as stream:
        for part in iter(lambda:stream.read(256*1024),b''):digest.update(part)
    return digest.hexdigest()
def regular(path,base):
    if path.is_symlink() or getattr(path,'is_junction',lambda:False)() or not path.is_file() or not path.resolve().is_relative_to(base.resolve()):raise ValueError('Crypto sources must be regular confined files')
    return path
def encoded(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode('utf-8')
def source_inventory(directory):
    if directory.is_symlink() or getattr(directory,'is_junction',lambda:False)() or not directory.is_dir():raise ValueError('Prepared crypto source root must be an ordinary directory')
    result=[];total=0
    for path in sorted(directory.rglob('*')):
        if path.is_symlink() or getattr(path,'is_junction',lambda:False)():raise ValueError('Prepared crypto source contains a link')
        if not path.is_file():continue
        regular(path,directory);size=path.stat().st_size;total+=size
        if size>32*1024*1024 or total>SOURCE_LIMIT or len(result)>=20000:raise ValueError('Prepared crypto source inventory exceeds its budget')
        result.append({'path':path.relative_to(directory).as_posix(),'bytes':size,'sha256':sha(path)})
    return result
def prepare(project):
    if project.exists():raise ValueError('Use a fresh browser crypto source project')
    if not project.resolve().is_relative_to((ROOT/'out').resolve()):raise ValueError('Browser crypto preparation must stay under the workspace out directory')
    workspace=tomllib.loads((ROOT/'Cargo.toml').read_text(encoding='utf-8'));paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock'];members=workspace['workspace']['members']
    for member in members:
        base=(ROOT/member).resolve()
        if not base.is_relative_to(ROOT) or not (base/'Cargo.toml').is_file():raise ValueError('Workspace member is outside the browser crypto source inventory')
        for path in sorted(base.rglob('*')):
            if path.is_symlink() or getattr(path,'is_junction',lambda:False)():raise ValueError('Workspace crypto source contains a link')
            if path.is_file():
                if any(part in {'target','.git','node_modules','__pycache__'} for part in path.relative_to(base).parts):continue
                if path.suffix.lower() not in {'.rs','.toml','.sql','.json','.h','.hpp','.c','.cc','.cpp','.md','.txt'} and path.name not in {'LICENSE','COPYING','NOTICE'}:raise ValueError('Unexpected workspace crypto source file: '+str(path))
                paths.append(path)
    extras=['security/web-sync-toolchain.json','security/web-sync-channel.json','security/web-sync-services.json',
            'tools/build_web_sync.py','tools/web_sync_artifact.py','tools/web_sync_resources.py','tools/web_sync_network.py',
            'apps/web/platform/sync-wasm-policy.js','apps/web/platform/sync-crypto-worker.js','apps/web/platform/sync-crypto.js',
            'apps/web/platform/sync-vault.js','apps/web/platform/sync-credentials.js','apps/web/platform/sync-crypto.json',
            'apps/web/platform/sync-network-policy.js','apps/web/platform/sync-http.js','apps/web/platform/sync-account.js',
            'apps/web/platform/sync-lease.js','apps/web/platform/sync-storage.js','apps/web/platform/sync-consent.js','apps/web/platform/sync-save-identity.js',
            'apps/web/platform/sync-hash-worker.js','apps/web/platform/sync-hash.js','apps/web/platform/sync-journal.js','apps/web/platform/sync-resumable.js',
            'apps/web/platform/sync-recovery-state.js','apps/web/platform/sync-recovery.js','apps/web/platform/sync-upload-recovery.js','apps/web/platform/sync-reconcile.js','apps/web/platform/sync-run.js',
            'apps/web/platform/sync-services.json','apps/web/platform/errors.js','apps/web/platform/platform.js','apps/web/platform/idb.js','apps/web/platform/files.js',
            'apps/web/platform/saves.js','apps/web/platform/preferences.js','apps/web/platform/storage-inventory.js','apps/web/platform/unused-assets.js','apps/web/platform/sha256.js','apps/web/platform/hash-client.js','apps/web/platform/hash-worker.js',
            'engines/web/sha256.js','engines/web/media-manifest.js','engines/web/core-options.js','engines/web/core-resources.js',
            'apps/game-hub/input-profile.js','apps/game-hub/touch-profile.js','apps/game-hub/save-origin.js',
            'apps/web/bootstrap.js','apps/game-hub/app.js','apps/game-hub/sync-settings.js','apps/game-hub/save-history.js','docs/WEB_SYNC_PENDING_EXPORT.md','docs/WEB_STORAGE_INVENTORY.md','docs/WEB_UNUSED_ASSETS.md','docs/LINUX_HOST_PROVISIONING.md','docs/MACOS_NATIVE_CONTAINMENT.md','packaging/linux/systemd/user/omniplay-native-host.service','packaging/linux/systemd/user/omniplay-native-host.socket','tools/install_user_release_posix.py','tools/unregister_user_release_posix.py','tools/replace_user_bootstrap_posix.py','tools/prepare_macos_containment.py','tools/author_macos_runtime_catalog.py','engines/native/macos-actor.entitlements','engines/presentation/macos-presentation.entitlements','engines/macos-owner/CMakeLists.txt','engines/macos-owner/Info.plist.in','engines/macos-owner/macos-owner.entitlements','engines/macos-owner/src/main.mm','engines/macos-owner/src/session.hpp','engines/macos-owner/src/session.mm','engines/macos-owner/src/catalog.hpp','engines/macos-owner/src/catalog.mm','engines/macos-owner/src/process.hpp','engines/macos-owner/src/process.mm']
    extras+=['apps/game-hub/native-work-maintenance.js','docs/WINDOWS_NATIVE_MAINTENANCE.md','docs/WINDOWS_NATIVE_COMPLETION.md','docs/WINDOWS_PRELAUNCH_RECOVERY.md','docs/WINDOWS_DEFAULT_STORAGE.md','docs/WINDOWS_REGISTRY_STORAGE.md','qa/windows/storage-probe.cpp','qa/windows/STORAGE_PROBE.md']
    extras+=['docs/NATIVE_MEMORY_FILES.md','qa/native-memory/CMakeLists.txt','qa/native-memory/memory_files_cases.cpp','qa/native-memory/ACCEPTANCE.md','engines/native/CMakeLists.txt','engines/native/src/writable_files_v1.h','engines/native/src/memory_files.hpp','engines/native/src/memory_files.cpp','engines/native/src/saved_files.hpp','engines/native/src/saved_files.cpp','engines/native/src/engine.hpp','engines/native/src/engine.cpp','engines/ares-md/adapter.cpp','engines/ares-md/web-files.hpp','engines/ares-md/save-memory.inc','tools/build_ares_md.py','tools/build_ares_md_web.py','tools/build_native_runtime.py']
    extras+=['docs/PS1_MEMORY_CARDS.md','docs/NATIVE_PS1_SAVES.md','qa/ps1-memory/CMakeLists.txt','qa/ps1-memory/ps1_memory_cases.cpp','qa/ps1-memory/ACCEPTANCE.md','engines/native/adapters/beetle-save-io.c.inc','engines/native/adapters/beetle-memory-files.c','engines/native/adapters/beetle-memory-files.h','engines/native/adapters/beetle-memory-frontio.c.inc','tools/build_beetle_psx.py','tools/build_beetle_psx_web.py','tools/psx_source_adaptations.py','engines/policies/beetle-options.json']
    extras+=['docs/N64_MEMORY_IO.md','qa/n64-memory/CMakeLists.txt','qa/n64-memory/n64_memory_cases.cpp','qa/n64-memory/ACCEPTANCE.md','engines/native/adapters/n64-memory-io.h','engines/native/adapters/n64-memory-io.cpp','engines/native/adapters/n64-memory-streams.hpp','engines/native/adapters/n64-memory-gzip.h','engines/native/adapters/n64-memory-gzip.cpp','tools/prepare_n64_memory_io.py','tools/pinned_core_source.py']
    extras+=['engines/native/adapters/n64-memory-text.hpp','engines/native/adapters/n64-shader-cache.hpp','engines/native/adapters/n64-shader-storage.cpp.inc','engines/native/adapters/n64-fiber-win.c.inc','qa/n64-memory/fiber_fixture.c','qa/n64-memory/fiber_cases.cpp','tools/prepare_n64_memory_core.py','tools/n64_source_adaptations.py','tools/build_mupen64plus.py']
    extras+=['engines/native/adapters/n64-memory-path.hpp','engines/native/adapters/n64-texture-cache.hpp','engines/native/adapters/n64-texture-memory.cpp.inc','engines/native/adapters/n64-texture-storage.cpp.inc','qa/n64-memory/texture_core_cases.cpp','qa/n64-memory/texture_fixture.cpp']
    extras+=['engines/native/adapters/n64-texture-pool.hpp','engines/native/adapters/n64-texture-pool.cpp','engines/native/adapters/n64-texture-pool-core.cpp.inc','qa/n64-memory/texture_pool_core_fixture.cpp','qa/n64-memory/texture_pool_cases.cpp']
    extras+=['engines/native/adapters/n64-memory-png.h','engines/native/adapters/n64-memory-png.c','engines/native/adapters/n64-texture-dump.cpp.inc','engines/native/adapters/n64-texture-debug.cpp.inc','engines/native/adapters/n64-native-log.cpp.inc','qa/n64-memory/png_cases.cpp','qa/n64-memory/diagnostic_cases.cpp']
    extras+=['engines/native/adapters/n64-save-files.h','engines/native/adapters/n64-save-files.cpp','engines/native/adapters/n64-file-storage.c.inc','engines/native/adapters/n64-util-save.c.inc','engines/native/adapters/n64-transferpak-ram.c.inc','qa/n64-memory/save_storage_cases.cpp']
    extras+=['engines/native/adapters/n64-dd-storage.h','engines/native/adapters/n64-dd-storage.c','engines/native/adapters/n64-dd-format.c.inc','engines/native/adapters/n64-dd-geometry.c.inc','engines/native/adapters/n64-dd-main.c.inc','qa/n64-memory/dd_format_fixture.c','qa/n64-memory/dd_storage_cases.cpp']
    extras+=['engines/native/adapters/n64-core-owner.h','engines/native/adapters/n64-core-owner.cpp','engines/native/adapters/n64-core-barrier.c.inc','engines/native/adapters/n64-main-storage-owner.c.inc','engines/native/adapters/n64-gb-storage-owner.h','engines/native/adapters/n64-snapshot-lifecycle.c.inc','qa/n64-memory/owner_cases.cpp','qa/n64-memory/main_owner_fixture.c','qa/n64-memory/owner_fiber_cases.cpp','engines/native/adapters/n64-snapshot-bounds.h','engines/native/adapters/n64-event-queue.c.inc','qa/n64-memory/event_queue_fixture.c','qa/n64-memory/snapshot_bounds_cases.cpp','engines/native/adapters/n64-tlb-memory.c.inc','qa/n64-memory/tlb_fixture.c','qa/n64-memory/tlb_snapshot_cases.cpp']
    extras+=['engines/native/adapters/n64-gb-storage-access.c.inc','qa/n64-memory/gb_transferpak_cases.cpp']
    extras+=['engines/native/adapters/n64-mbc3-rtc.c.inc','qa/n64-memory/mbc3_rtc_cases.cpp','docs/N64_SNAPSHOT_FORMAT.md']
    extras+=['engines/native/adapters/n64-rtc-storage.h','engines/native/adapters/n64-rtc-storage.c','engines/native/adapters/n64-gb-rtc-bind.c.inc','qa/n64-memory/rtc_storage_cases.cpp','docs/N64_RTC_BATTERY.md']
    extras+=['engines/native/adapters/n64-device-memory.h','engines/native/adapters/n64-flash-memory.c.inc','engines/native/adapters/n64-si-copy.c.inc','engines/native/adapters/n64-dd-c2.c.inc','qa/n64-memory/peripheral_memory_cases.cpp','qa/n64-memory/si_memory_fixture.c','qa/n64-memory/dd_c2_fixture.c','docs/N64_DEVICE_MEMORY.md']
    extras+=['engines/native/adapters/n64-pi-memory.c.inc','engines/native/adapters/n64-framebuffer-memory.c.inc','engines/native/adapters/n64-framebuffer-mapping.c.inc','engines/native/adapters/n64-framebuffer-cache.hpp','qa/n64-memory/pi_framebuffer_fixture.h','qa/n64-memory/pi_framebuffer_fixture.c','qa/n64-memory/pi_framebuffer_cases.cpp']
    extras+=['engines/native/adapters/n64-dma-memory.h','engines/native/adapters/n64-sram-memory.c.inc','engines/native/adapters/n64-cart-rom-memory.c.inc','engines/native/adapters/n64-dd-dma-memory.c.inc','qa/n64-memory/cartridge_dma_fixture.h','qa/n64-memory/cartridge_dma_fixture.c','qa/n64-memory/cartridge_dma_cases.cpp']
    extras+=['engines/native/adapters/n64-color-memory.hpp','engines/native/adapters/n64-color-buffer.cpp.inc','engines/native/adapters/n64-color-upload.cpp.inc','qa/n64-memory/color_memory_cases.cpp']
    extras+=['engines/native/adapters/n64-rom-image.h','qa/n64-memory/rom_image_cases.cpp']
    extras+=['engines/native/adapters/n64-content-load.c.inc','qa/n64-memory/content_load_cases.cpp']
    extras+=['engines/native/adapters/n64-audio-lifecycle.c.inc','qa/n64-memory/audio_lifecycle_cases.cpp']
    extras+=['engines/native/adapters/n64-audio-output.h','engines/native/adapters/n64-audio-output.c.inc','qa/n64-memory/audio_output_cases.cpp']
    extras+=['engines/native/adapters/n64-ai-memory.c.inc','engines/native/adapters/n64-audio-source.c.inc','qa/n64-memory/audio_source_cases.cpp']
    extras+=['engines/native/adapters/n64-ai-timing.h','engines/native/adapters/n64-ai-dma.c.inc','qa/n64-memory/ai_timing_cases.cpp']
    extras+=['engines/native/adapters/n64-sinc-capacity.h','engines/native/adapters/n64-sinc-checked.c.inc','engines/native/adapters/n64-audio-sinc.cpp.inc','qa/n64-memory/sinc_capacity_cases.cpp']
    extras+=['qa/n64-memory/sinc_actual_cases.cpp']
    extras+=['engines/native/adapters/n64-ai-fifo.c.inc']
    extras+=['engines/native/adapters/n64-event-insert.c.inc','qa/n64-memory/event_insert_fixture.c','qa/n64-memory/event_insert_cases.cpp']
    extras+=['engines/native/src/gl_video.hpp','engines/native/src/gl_video.cpp','qa/windows/NATIVE_ACCEPTANCE.md']
    extras+=['engines/native/src/audio_ring.hpp','qa/native-audio/CMakeLists.txt','qa/native-audio/audio_ring_cases.cpp','qa/native-audio/ACCEPTANCE.md']
    extras+=['engines/native/src/owned_actor_exit.h','engines/native/src/main.cpp','docs/NATIVE_GUEST_PAUSE.md','qa/native-lifecycle/CMakeLists.txt','qa/native-lifecycle/fixture_core.cpp','qa/native-lifecycle/run_cases.py','qa/native-lifecycle/ACCEPTANCE.md','tools/build_ppsspp.py']
    extras+=['engines/native/src/protocol.hpp','engines/native/src/protocol.cpp','docs/NATIVE_CONTROL_BOUNDARY.md','qa/native-lifecycle/wire_cases.py']
    extras+=['docs/NATIVE_CONTROL_WORKERS.md','qa/native-lifecycle/CONTROL_WORKERS.md']
    extras+=['docs/WINDOWS_ITEM2_CLOSURE.md']
    paths.extend(ROOT/name for name in extras);total=0
    for path in paths:
        regular(path,ROOT);total+=path.stat().st_size
        if path.stat().st_size>32*1024*1024 or total>SOURCE_LIMIT or len(paths)>20000:raise ValueError('Workspace crypto source exceeds its preparation budget')
    source=project/'source';source.mkdir(parents=True)
    for path in paths:
        destination=source/path.relative_to(ROOT);destination.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,destination)
    receipt={'schema_version':1,'source_only':True,'target':'wasm32-wasip1','protocol':1,'source_inventory':source_inventory(source),'cargo_lock_sha256':sha(source/'Cargo.lock'),'toolchain':json.loads((source/'security/web-sync-toolchain.json').read_text(encoding='utf-8')),'configuration':'not_started','compiler':'not_started','worker':'not_started','verification':'not_run','distribution_approved':False}
    (project/'preparation-receipt.json').write_bytes(encoded(receipt));print('Prepared immutable browser crypto source. No compiler, WASM, browser or test was started.')
def build(project,cargo,rustc,cargo_home):
    regular(project/'preparation-receipt.json',project);receipt=json.loads((project/'preparation-receipt.json').read_text(encoding='utf-8'));source=project/'source'
    if receipt.get('source_only')is not True or source_inventory(source)!=receipt['source_inventory'] or sha(source/'Cargo.lock')!=receipt['cargo_lock_sha256']:raise ValueError('Prepared browser crypto source changed')
    if sha(Path(__file__).resolve())!=sha(source/'tools/build_web_sync.py') or sha(ROOT/'tools/web_sync_artifact.py')!=sha(source/'tools/web_sync_artifact.py'):raise ValueError('Browser crypto build/admission recipe differs from its prepared source')
    toolchain=receipt['toolchain'];regular(cargo,cargo.parent);regular(rustc,rustc.parent)
    version=subprocess.run([str(rustc),'--version','--verbose'],check=True,capture_output=True,text=True).stdout
    if 'release: '+toolchain['rust_release']+'\n' not in version or 'commit-hash: '+toolchain['rust_commit_prefix'] not in version:raise ValueError('Browser crypto requires its explicit pinned Rust compiler')
    sysroot=Path(subprocess.run([str(rustc),'--print','sysroot'],check=True,capture_output=True,text=True).stdout.strip()).resolve();standard=sysroot/'lib/rustlib/wasm32-wasip1/lib'
    if not standard.is_dir() or standard.is_symlink() or getattr(standard,'is_junction',lambda:False)():raise ValueError('Install and qualify the explicit Rust WASIp1 standard library separately')
    standard_files=[]
    for path in sorted(standard.iterdir()):
        regular(path,standard)
        if len(standard_files)>=128 or path.stat().st_size>128*1024*1024:raise ValueError('Unexpected browser crypto standard library inventory')
        standard_files.append({'path':path.name,'bytes':path.stat().st_size,'sha256':sha(path)})
    if not standard_files:raise ValueError('The browser crypto standard library is empty')
    if (project/'build').exists() or (project/'package').exists():raise ValueError('Use a fresh prepared browser crypto build output')
    env=os.environ.copy()
    for key in list(env):
        if key.startswith(('CARGO_','RUSTC_','RUSTDOC','RUSTFLAGS','RUSTUP_TOOLCHAIN','LD_','DYLD_')) or key in {'RUSTC','CC','CXX','AR','RUSTC_BOOTSTRAP'}:env.pop(key,None)
    flags=['-C','link-arg=--no-entry','-C','link-arg=--export-memory','-C','link-arg=--initial-memory=33554432','-C','link-arg=--max-memory=1073741824','-C','link-arg=-zstack-size=2097152','--remap-path-prefix',str(source)+'=/omniplay/src']
    env.update({'CARGO_HOME':str(cargo_home),'RUSTC':str(rustc),'CARGO_TARGET_DIR':str(project/'build'),'CARGO_ENCODED_RUSTFLAGS':'\x1f'.join(flags)})
    subprocess.run([str(cargo),'build','--frozen','--offline','--release','--package','omniplay-web-sync','--target','wasm32-wasip1'],cwd=source,env=env,check=True)
    artifact=regular(project/'build/wasm32-wasip1/release/omniplay_web_sync.wasm',project)
    if artifact.stat().st_size>16*1024*1024:raise ValueError('Browser crypto artifact exceeds its size budget')
    data=artifact.read_bytes();policy=inspect(data)
    package=project/'package';package.mkdir();shutil.copy2(artifact,package/'sync-crypto.wasm')
    value={'schema_version':1,'source_only':False,'target':'wasm32-wasip1','protocol':1,'wasm_bytes':len(data),'wasm_sha256':hashlib.sha256(data).hexdigest(),'module_policy':policy,'cargo_lock_sha256':receipt['cargo_lock_sha256'],'prepared_source_inventory_sha256':hashlib.sha256(encoded(receipt['source_inventory'])).hexdigest(),'preparation_receipt_sha256':sha(project/'preparation-receipt.json'),'rustc_sha256':sha(rustc),'cargo_sha256':sha(cargo),'rustc_version':version,'standard_library_inventory':standard_files,'distribution_approved':False,'verification':'not_run','release_requires':'explicit artifact approval, compiled dependency and SDK provenance, complete final cross-platform wire/credential/crypto/storage/security verification'}
    (package/'build-receipt.json').write_bytes(encoded(value));print('Built a browser crypto candidate. It was not executed, tested, approved or enabled.')
def main():
    parser=argparse.ArgumentParser(description=__doc__);modes=parser.add_mutually_exclusive_group(required=True);modes.add_argument('--prepare-only',action='store_true');modes.add_argument('--build-prepared',action='store_true');parser.add_argument('--project',type=Path,required=True);parser.add_argument('--cargo',type=Path);parser.add_argument('--rustc',type=Path);parser.add_argument('--cargo-home',type=Path);args=parser.parse_args();project=args.project.resolve()
    output=ROOT/'out'
    if output.is_symlink() or getattr(output,'is_junction',lambda:False)() or not output.resolve().is_relative_to(ROOT.resolve()) or not project.is_relative_to(output.resolve()):raise ValueError('Browser crypto projects must remain in the ordinary workspace output directory')
    for path in [project,*project.parents]:
        if path==ROOT:break
        if path.is_symlink() or getattr(path,'is_junction',lambda:False)():raise ValueError('Browser crypto project ancestry cannot contain a link')
    if args.prepare_only:prepare(project)
    else:
        if not all([args.cargo,args.rustc,args.cargo_home]):raise ValueError('The prepared build needs explicit Cargo, rustc and offline registry cache paths')
        build(project,args.cargo.resolve(),args.rustc.resolve(),args.cargo_home.resolve())
if __name__=='__main__':main()
