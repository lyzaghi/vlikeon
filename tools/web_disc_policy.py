"""Admission of explicitly declared WASM disc/snapshot protocol metadata."""

def admit(core):
    enabled='disc_control'in core.get('capabilities',[])
    if enabled:
        if type(core.get('disc_state_protocol'))is not int or core['disc_state_protocol']!=1:raise ValueError('Web disc controls require the explicit snapshot envelope protocol')
        version=core.get('state_format_version')
        if not isinstance(version,str) or not version or len(version.encode('utf-8'))>500:raise ValueError('Web disc snapshot base version is missing or too long')
        if core.get('media_filesystem_protocol')!=1:raise ValueError('Web disc controls require a bounded media filesystem')
    elif 'disc_state_protocol'in core:raise ValueError('A disc snapshot protocol cannot be declared without disc controls')
    if 'm3u'in core.get('content_formats',[]) and not enabled:raise ValueError('Playlist support requires disc controls and its snapshot protocol')
