"""Explicit opt-in contract for bounded WASM persistent files and restart saves."""


def admit(core):
    enabled = 'save_files' in core.get('capabilities', [])
    if not enabled:
        if 'saved_files_protocol' in core or 'saved_files_io' in core:
            raise ValueError('Persistent-file metadata requires its explicit capability')
        return
    if type(core.get('saved_files_protocol')) is not int or core['saved_files_protocol'] != 1:
        raise ValueError('Web persistent files require their explicit envelope protocol')
    if core.get('wasm_threads') is True:
        if core.get('saved_files_io') != 'pthread_joined_barrier_v1' or core.get('thread_callbacks_protocol')!=1 or core.get('libretro_result_protocol')!=2:
            raise ValueError('Threaded persistent files need their joined I/O and typed callback contract')
    elif core.get('saved_files_io') != 'single_threaded_barrier_v1' or core.get('wasm_threads') is not False:
        raise ValueError('Persistent-file cores must declare their explicit guest I/O barrier contract')
    for field in ('in_game_format_version', 'state_format_version'):
        value = core.get(field)
        # Leave room for both the disc and persistent-file envelope suffixes.
        if not isinstance(value, str) or not value or len(value.encode('utf-8')) > 480:
            raise ValueError('Persistent-file save format versions are missing or too long')
