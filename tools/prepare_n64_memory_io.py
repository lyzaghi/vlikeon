"""Freeze N64 memory I/O adapters and exact vendor codec source; never build.

This is source copying/materialization and byte inventory only. It starts no
configuration/compiler, core, helper, OS product API, browser, server or test.
"""
import argparse
import json
from pathlib import Path
import shutil
from pinned_core_source import ROOT, file_hash, materialize, owned, source_inventory

PIN='12edd2c74a517ff86dfa8cfc71ad75e4c10486d5'
ADAPTERS=('n64-memory-io.h','n64-memory-io.cpp','n64-memory-streams.hpp',
          'n64-memory-gzip.h','n64-memory-gzip.cpp','n64-memory-text.hpp',
          'n64-shader-cache.hpp','n64-shader-storage.cpp.inc','n64-fiber-win.c.inc',
          'n64-memory-path.hpp','n64-texture-cache.hpp','n64-texture-memory.cpp.inc','n64-texture-storage.cpp.inc',
          'n64-texture-pool.hpp','n64-texture-pool.cpp','n64-texture-pool-core.cpp.inc',
          'n64-memory-png.h','n64-memory-png.c','n64-texture-dump.cpp.inc','n64-texture-debug.cpp.inc','n64-native-log.cpp.inc',
          'n64-save-files.h','n64-save-files.cpp','n64-file-storage.c.inc','n64-util-save.c.inc','n64-transferpak-ram.c.inc',
          'n64-dd-storage.h','n64-dd-storage.c','n64-dd-format.c.inc','n64-dd-geometry.c.inc','n64-dd-main.c.inc',
          'n64-core-owner.h','n64-core-owner.cpp','n64-core-barrier.c.inc','n64-main-storage-owner.c.inc','n64-gb-storage-owner.h','n64-snapshot-lifecycle.c.inc',
          'n64-snapshot-bounds.h','n64-event-queue.c.inc','n64-tlb-memory.c.inc','n64-gb-storage-access.c.inc','n64-mbc3-rtc.c.inc',
          'n64-rtc-storage.h','n64-rtc-storage.c','n64-gb-rtc-bind.c.inc',
          'n64-device-memory.h','n64-flash-memory.c.inc','n64-si-copy.c.inc','n64-dd-c2.c.inc',
          'n64-pi-memory.c.inc','n64-framebuffer-memory.c.inc','n64-framebuffer-mapping.c.inc','n64-framebuffer-cache.hpp',
          'n64-dma-memory.h','n64-sram-memory.c.inc','n64-cart-rom-memory.c.inc','n64-dd-dma-memory.c.inc',
          'n64-color-memory.hpp','n64-color-buffer.cpp.inc','n64-color-upload.cpp.inc','n64-rom-image.h','n64-content-load.c.inc','n64-audio-lifecycle.c.inc','n64-audio-output.h','n64-audio-output.c.inc','n64-ai-memory.c.inc','n64-audio-source.c.inc','n64-ai-timing.h','n64-ai-dma.c.inc','n64-sinc-capacity.h','n64-sinc-checked.c.inc','n64-audio-sinc.cpp.inc','n64-ai-fifo.c.inc','n64-event-insert.c.inc')
SELECTED=('custom/dependencies/libzlib','custom/dependencies/libpng','libretro-common/include/libco.h','libretro-common/include/retro_common_api.h')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project',type=Path,required=True)
    parser.add_argument('--git',default='git')
    args=parser.parse_args();project=owned(args.project.absolute())
    if not project.resolve().is_relative_to((ROOT/'out').resolve()):
        raise ValueError('Use a fresh ordinary workspace output project')
    source,archive=materialize('mupen64plus-next',PIN,project,args.git,SELECTED)
    names=['engines/native/adapters/'+name for name in ADAPTERS]
    names+=['engines/native/src/writable_files_v1.h','engines/native/src/memory_files.hpp',
            'engines/native/src/memory_files.cpp','qa/n64-memory/CMakeLists.txt',
            'qa/n64-memory/n64_memory_cases.cpp','qa/n64-memory/fiber_fixture.c','qa/n64-memory/fiber_cases.cpp','qa/n64-memory/ACCEPTANCE.md',
            'qa/n64-memory/texture_core_cases.cpp','qa/n64-memory/texture_fixture.cpp',
            'qa/n64-memory/texture_pool_core_fixture.cpp','qa/n64-memory/texture_pool_cases.cpp',
            'qa/n64-memory/png_cases.cpp',
            'qa/n64-memory/diagnostic_cases.cpp',
            'qa/n64-memory/save_storage_cases.cpp',
            'qa/n64-memory/dd_format_fixture.c','qa/n64-memory/dd_storage_cases.cpp',
            'qa/n64-memory/owner_cases.cpp','qa/n64-memory/main_owner_fixture.c','qa/n64-memory/owner_fiber_cases.cpp',
            'qa/n64-memory/event_queue_fixture.c','qa/n64-memory/snapshot_bounds_cases.cpp',
            'qa/n64-memory/tlb_fixture.c','qa/n64-memory/tlb_snapshot_cases.cpp',
            'qa/n64-memory/gb_transferpak_cases.cpp',
            'qa/n64-memory/mbc3_rtc_cases.cpp','docs/N64_SNAPSHOT_FORMAT.md',
            'qa/n64-memory/rtc_storage_cases.cpp','docs/N64_RTC_BATTERY.md',
            'qa/n64-memory/peripheral_memory_cases.cpp','qa/n64-memory/si_memory_fixture.c','qa/n64-memory/dd_c2_fixture.c','docs/N64_DEVICE_MEMORY.md',
            'qa/n64-memory/pi_framebuffer_fixture.h','qa/n64-memory/pi_framebuffer_fixture.c','qa/n64-memory/pi_framebuffer_cases.cpp',
            'qa/n64-memory/cartridge_dma_fixture.h','qa/n64-memory/cartridge_dma_fixture.c','qa/n64-memory/cartridge_dma_cases.cpp',
            'qa/n64-memory/color_memory_cases.cpp','qa/n64-memory/rom_image_cases.cpp','qa/n64-memory/content_load_cases.cpp','qa/n64-memory/audio_lifecycle_cases.cpp','qa/n64-memory/audio_output_cases.cpp','qa/n64-memory/audio_source_cases.cpp','qa/n64-memory/ai_timing_cases.cpp','qa/n64-memory/sinc_capacity_cases.cpp','qa/n64-memory/sinc_actual_cases.cpp','qa/n64-memory/event_insert_fixture.c','qa/n64-memory/event_insert_cases.cpp',
            'docs/N64_MEMORY_IO.md','tools/prepare_n64_memory_io.py','tools/pinned_core_source.py',
            'tools/prepare_n64_memory_core.py','tools/n64_source_adaptations.py','tools/build_mupen64plus.py']
    for name in names:
        incoming=owned(ROOT/name)
        if not incoming.is_file():raise ValueError('A required N64 I/O source is missing')
        outgoing=source/name;outgoing.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(incoming,outgoing)
    inventory=source_inventory(source)
    receipt={'schema_version':1,'source_only':True,'purpose':'N64 owned scratch/persistent I/O foundation',
        'upstream_revision':PIN,'selected_upstream_paths':list(SELECTED),
        'upstream_source_sha256':file_hash(archive),'source_inventory':inventory,
        'source_files':len(inventory),'source_bytes':sum((source/name).stat().st_size for name in inventory),
        'adapter_inputs_sha256':{name:file_hash(ROOT/'engines/native/adapters'/name)for name in ADAPTERS},
        'writable_files_header_sha256':file_hash(ROOT/'engines/native/src/writable_files_v1.h'),
        'target_writable_files_protocol':1,'n64_core_integration_complete':False,
        'n64_package_admission_changed':False,'n64_actor_os_write_grants_removed':False,
        'all_native_core_writes_bounded':False,'configuration':'not_started','compiler':'not_started',
        'verification':'not_run','product_enabled':False,'distribution_approved':False,
        'remaining':['other core/peripheral write integration and remaining decoder/temporary bounds',
          'quiescent ABI lifecycle and exact core/package admission',
          'final dependency security/license selection and artifact identity',
          'actual core/worker/broker/OS/game/device/fault qualification']}
    (project/'preparation-receipt.json').write_text(json.dumps(receipt,sort_keys=True,indent=2)+'\n',encoding='utf-8')
    print('Froze N64 memory I/O, actual store/QA and pinned codec source only. No configure, build, core or test started.')


if __name__=='__main__':main()
