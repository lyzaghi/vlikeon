"""Admission of a core's explicit post-load system-memory observation floor."""
def admit(core):
    value = core.get('minimum_system_ram_bytes', 0)
    if type(value) is not int or not 0 <= value <= 64 * 1024 * 1024:
        raise ValueError('Core system-memory admission floor is invalid')
    if 'file_backed_content_required' in core:
        if type(core['file_backed_content_required']) is not bool or core['file_backed_content_required'] and core.get('media_filesystem_protocol') != 1:
            raise ValueError('A core requiring file-backed content needs the media filesystem protocol')
