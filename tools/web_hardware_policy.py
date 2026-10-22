"""Finite manifest admission for the dedicated Worker WebGL2 frontend."""
def admit(core):
    if 'hardware_video' not in core:
        if 'hardware_video_protocol' in core:
            raise ValueError('Hardware video protocol needs an explicit target')
        return
    value = core['hardware_video']
    if core.get('hardware_video_protocol') != 1 or core.get('wasm_threads') is not False or core.get('libretro_result_protocol') != 2:
        raise ValueError('Hardware video requires the unthreaded completion protocol')
    if not isinstance(value, dict) or set(value) != {'schema_version', 'api', 'maximum_width', 'maximum_height'} or value.get('schema_version') != 1 or value.get('api') != 'webgl2':
        raise ValueError('Hardware video target is invalid')
    width, height = value['maximum_width'], value['maximum_height']
    if type(width) is not int or type(height) is not int or not 1 <= width <= 1920 or not 1 <= height <= 1080 or width * height > 1920 * 1080:
        raise ValueError('Hardware video target exceeds its dimension bound')
