"""Bounded structural admission of the Windows video worker; never executes it.

PE structure/flags are not Authenticode, provenance, vulnerability or runtime
evidence. Signed outer-package admission and final release qualification remain.
"""
import hashlib
from pathlib import Path
import struct

ENTRY = 'helpers/omniplay-video-worker.exe'
MACHINES = {'windows-x64': 0x8664, 'windows-arm64': 0xaa64}
MAX_BYTES = 128 * 1024 * 1024


def admit_video_artifact(path, platform):
    path = Path(path)
    if platform not in MACHINES or not path.is_file() or path.is_symlink():
        raise ValueError('The video worker requires a regular Windows x64/ARM64 PE file')
    for parent in path.parents:
        if parent.is_symlink() or getattr(parent, 'is_junction', lambda: False)():
            raise ValueError('The video worker cannot traverse linked parents')
    if getattr(path, 'is_junction', lambda: False)():
        raise ValueError('The video worker cannot be a reparse object')
    with path.open('rb') as source:
        data = source.read(MAX_BYTES + 1)
    if not 256 <= len(data) <= MAX_BYTES or data[:2] != b'MZ':
        raise ValueError('The video worker exceeds its bound or has no DOS header')
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    if pe < 64 or pe > len(data) - 24 or data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('The video worker PE header is invalid')
    machine, sections = struct.unpack_from('<HH', data, pe + 4)
    optional_size, characteristics = struct.unpack_from('<HH', data, pe + 20)
    optional = pe + 24
    if (machine != MACHINES[platform] or not 1 <= sections <= 96
            or optional_size < 112 or optional + optional_size + sections * 40 > len(data)
            or not characteristics & 2 or characteristics & 0x2000):
        raise ValueError('The video worker architecture or executable layout is invalid')
    magic = struct.unpack_from('<H', data, optional)[0]
    subsystem, flags = struct.unpack_from('<HH', data, optional + 68)
    # PE32+, console helper, high-entropy ASLR, dynamic base, NX and CFG.
    if magic != 0x20b or subsystem != 3 or flags & 0x4160 != 0x4160:
        raise ValueError('The video worker PE format or required mitigation flags are missing')
    for index in range(sections):
        section = optional + optional_size + index * 40
        size, offset = struct.unpack_from('<II', data, section + 16)
        if size and (offset < optional + optional_size + sections * 40 or offset > len(data) or size > len(data) - offset):
            raise ValueError('The video worker section exceeds its file')
    return {'path': ENTRY, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(),
            'executable': True, 'platform': platform,
            'verification': 'pe_structure_and_flags_only_not_runtime_or_signature'}
