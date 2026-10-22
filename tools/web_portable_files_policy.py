"""Explicit portable persistent-file admission, never inferred from core IDs."""
FORMAT='ares-md-canonical-files-v1'
PIN='9408cb43d4948fc3ea6e152a307a34348df3fe04'
VERSION='0.3.0+ares.9408cb43d494.omweb1'


def admit(core):
    if 'portable_in_game_format' not in core and 'portable_files_protocol' not in core:
        return
    if core.get('portable_in_game_format')!=FORMAT or type(core.get('portable_files_protocol')) is not int or core['portable_files_protocol']!=1 or core.get('in_game_format_version')!=FORMAT or core.get('id')!='ares-md-web' or core.get('core_version')!=VERSION or core.get('runtime_version_string')!=VERSION or core.get('source_revision')!=PIN or core.get('systems')!=['mega_drive'] or core.get('saved_files_protocol')!=1 or core.get('saved_files_io')!='single_threaded_barrier_v1' or core.get('wasm_threads') is not False or core.get('libretro_result_protocol')!=1 or 'save_files' not in core.get('capabilities',[]):
        raise ValueError('Portable Web progress requires its exact admitted MD file/descriptor profile')
