"""Materialize pinned native N64 configuration/shader/texture integration source.

No configure, build, compiler, core, helper, OS product API, browser or test runs.
Other core/peripheral write closure, decoder bounds and package admission are unfinished;
there is deliberately no build or distribution entry point in this preparer.
"""
import argparse
import json
import shutil
from pathlib import Path
from pinned_core_source import ROOT,file_hash,materialize,owned,source_inventory
from build_mupen64plus import PIN,GL_VERSION,prepare
from n64_source_adaptations import integrate,replace

VERSION='3.0.0+omniplay.gl3.12edd2c74a51'
SELECTED=('LICENSE','Makefile','Makefile.common','README.md','GLideN64','custom',
          'libretro-common','libretro','mupen64plus-core','mupen64plus-rsp-hle','xxHash')
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
RECIPES=('prepare_n64_memory_core.py','n64_source_adaptations.py','build_mupen64plus.py','pinned_core_source.py')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project',type=Path,required=True);parser.add_argument('--git',default='git')
    args=parser.parse_args();project=owned(args.project.absolute())
    if not project.resolve().is_relative_to((ROOT/'out').resolve()):raise ValueError('Use a fresh ordinary workspace output project')
    source,archive=materialize('mupen64plus-next',PIN,project,args.git,SELECTED)
    changes=prepare(source,source,'opengl',materialized=True)
    replace(source/'libretro/libretro.c','info->library_version = "'+GL_VERSION+'";','info->library_version = "'+VERSION+'";')
    adapters=ROOT/'engines/native/adapters'
    for name in ADAPTERS:shutil.copy2(owned(adapters/name),source/name)
    shutil.copy2(owned(ROOT/'engines/native/src/writable_files_v1.h'),source/'writable_files_v1.h')
    changes+=integrate(source,adapters)
    adaptations=source/'OMNIPLAY_ADAPTATIONS.json';metadata=json.loads(adaptations.read_text(encoding='utf-8'))
    metadata.update({'core_version':VERSION,'changes':changes,'memory_io_integration_complete':False,'product_enabled':False})
    adaptations.write_text(json.dumps(metadata,sort_keys=True,indent=2)+'\n',encoding='utf-8')
    recipe_hashes={}
    for name in RECIPES:
        incoming=owned(ROOT/'tools'/name);shutil.copy2(incoming,project/name);recipe_hashes[name]=file_hash(incoming)
    for name in ['N64_MEMORY_IO.md','N64_SNAPSHOT_FORMAT.md','N64_RTC_BATTERY.md','N64_DEVICE_MEMORY.md']:
        shutil.copy2(owned(ROOT/'docs'/name),project/name)
    inventory=source_inventory(source)
    receipt={'schema_version':1,'source_only':True,'core':'mupen64plus-gl','core_version':VERSION,
        'upstream_revision':PIN,'selected_upstream_paths':list(SELECTED),'upstream_source_sha256':file_hash(archive),
        'source_inventory':inventory,'source_files':len(inventory),
        'source_bytes':sum((source/name).stat().st_size for name in inventory),
        'recipe_inputs_sha256':recipe_hashes,'adapter_inputs_sha256':{name:file_hash(adapters/name)for name in ADAPTERS},
        'writable_files_header_sha256':file_hash(ROOT/'engines/native/src/writable_files_v1.h'),
        'configuration_database_and_shader_source_integrated':True,'texture_gzip_and_file_storage_source_integrated':True,'bounded_texture_pool_source_integrated':True,'target_writable_files_protocol':1,
        'scratch_texture_png_and_diagnostics_source_integrated':True,'n64_png_dump_verified':False,
        'persistent_file_storage_and_transferpak_ram_source_integrated':True,'n64_persistent_storage_verified':False,
        'dd_storage_format_geometry_and_ipl_source_integrated':True,'n64_dd_storage_verified':False,
        'owner_prepare_pause_snapshot_and_checked_peripheral_flush_source_integrated':True,'n64_owner_capture_verified':False,
        'snapshot_queue_pif_pointer_preflight_and_dd_id_source_integrated':True,'snapshot_semantic_validation_complete':False,'n64_snapshot_bounds_verified':False,
        'tlb_snapshot_fields_lut_and_finite_page_walk_source_integrated':True,'interpreter_tlb_random_index_guard_source_integrated':True,'n64_tlb_snapshot_verified':False,
        'transferpak_snapshot_selectors_and_bounded_gb_storage_camera_source_integrated':True,'gb_full_width_ram_offsets_and_rumble_bank_mask_source_integrated':True,'n64_gb_transferpak_verified':False,
        'mbc3_rtc_bounded_time_halt_carry_latched_reads_source_integrated':True,'m64p_snapshot_write_version':'1.10','m64p_snapshot_read_versions':'1.0-1.10','n64_mbc3_rtc_verified':False,
        'rtc_battery_staged_boot_offline_time_and_checked_ram_retirement_source_integrated':True,'rtc_battery_record_bytes':32,'n64_rtc_battery_verified':False,
        'flash_si_c2_memory_and_snapshot_device_selectors_source_integrated':True,'n64_device_memory_verified':False,
        'pi_full_dram_framebuffer_ranges_and_fixed_graphics_cache_source_integrated':True,'n64_pi_framebuffer_verified':False,
        'sram_cart_rom_dd_direct_dma_complete_ranges_source_integrated':True,'n64_cartridge_dma_verified':False,
        'fixed_color_dirty_bitmap_and_bounded_pixel_copy_source_integrated':True,'n64_color_memory_verified':False,
        'rom_image_admission_and_unaligned_conversion_source_integrated':True,'n64_rom_image_verified':False,
        'frontend_content_rom_receipt_and_failed_header_close_source_integrated':True,'n64_content_load_verified':False,
        'audio_checked_partial_allocation_frontend_readiness_source_integrated':True,'n64_audio_lifecycle_verified':False,
        'finite_audio_output_queue_and_root_backpressure_source_integrated':True,'n64_audio_output_verified':False,
        'audio_callback_borrow_root_reentry_and_retirement_source_integrated':True,'n64_audio_callback_retirement_verified':False,
        'ai_complete_ram_publication_immutable_backend_copy_source_integrated':True,'n64_audio_source_verified':False,
        'ai_checked_duration_fifo_and_delayed_carry_source_integrated':True,'n64_ai_timing_verified':False,
        'sinc_phase_prewrite_output_capacity_source_integrated':True,'n64_sinc_capacity_verified':False,
        'sinc_bounded_filter_allocation_instance_dispatch_source_integrated':True,'n64_actual_sinc_verified':False,
        'ai_staged_fifo_publication_source_integrated':True,'n64_ai_fifo_verified':False,
        'ai_event_pool_insertion_acknowledgement_source_integrated':True,'n64_ai_event_verified':False,
        'failed_stopped_guest_peripheral_flush_recovery_source_integrated':True,'n64_stopped_save_recovery_verified':False,
        'native_sized_audio_batch_offers_source_integrated':True,'n64_native_audio_batch_verified':False,
        'n64_core_memory_io_integration_complete':False,'n64_texture_cache_verified':False,'n64_package_admission_changed':False,
        'n64_actor_os_write_grants_removed':False,'all_native_core_writes_bounded':False,
        'configuration':'not_started','compiler':'not_started','core_execution':'not_started',
        'verification':'not_run','product_enabled':False,'distribution_approved':False,
        'remaining':['other core and peripheral write closure, remaining decoder/temporary bounds',
          'actual full core/cache/producer lifecycle and SRAM/snapshot qualification',
          'exact core/worker/package/broker admission and final dependency security/license selection',
          'real OS/game/device/driver/performance/fault and all original final product verification']}
    (project/'preparation-receipt.json').write_text(json.dumps(receipt,sort_keys=True,indent=2)+'\n',encoding='utf-8')
    print('Materialized pinned N64 configuration/shader/texture/PNG source. No build, core or test started; remaining core/peripheral/package admission is due.')


if __name__=='__main__':main()
