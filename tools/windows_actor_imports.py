"""Bounded PE import inventory for Windows device-free actor package receipts.

Static import admission is not proof against runtime loads or a sandbox test.
The actual actor's Win32k/job/token boundaries remain mandatory.
"""
import re
import struct
from pathlib import Path

DESKTOP_DLLS = frozenset({
    'user32.dll', 'gdi32.dll', 'gdi32full.dll', 'win32u.dll', 'ole32.dll',
    'oleaut32.dll', 'shell32.dll', 'shcore.dll', 'shlwapi.dll', 'imm32.dll',
    'winmm.dll', 'setupapi.dll', 'dinput8.dll', 'dsound.dll', 'opengl32.dll',
    'd3d9.dll', 'd3d11.dll', 'd3d12.dll', 'dxgi.dll', 'vulkan-1.dll',
})


def pe_imports(path: Path):
    if path.is_symlink() or not path.is_file() or not 64 <= path.stat().st_size <= 128 * 1024**2:
        raise ValueError('PE import input must be a bounded ordinary file')
    data = path.read_bytes()

    def unpack(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(data):
            raise ValueError('Truncated PE import structure')
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b'MZ':
        raise ValueError('Missing PE DOS signature')
    pe, = unpack('<I', 60)
    if pe > 1024**2 or data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Invalid PE signature')
    machine, sections = unpack('<HH', pe + 4)
    optional_size, = unpack('<H', pe + 20)
    optional = pe + 24
    magic, = unpack('<H', optional)
    if machine != 0x8664 or magic != 0x20b or not 1 <= sections <= 96 or optional_size < 240:
        raise ValueError('Expected bounded x64 PE32+ headers')
    directories, = unpack('<I', optional + 108)
    if not 14 <= directories <= 16 or 112 + directories * 8 > optional_size:
        raise ValueError('Invalid PE directory count')
    image_base, = unpack('<Q', optional + 24)
    layout = [unpack('<IIII', optional + optional_size + index * 40 + 8) for index in range(sections)]

    def rva_offset(rva, size):
        candidates = [raw + rva - start for _, start, raw_size, raw in layout
                      if start <= rva and rva - start + size <= raw_size]
        if len(candidates) != 1 or candidates[0] + size > len(data):
            raise ValueError('Unmapped or ambiguous PE import RVA')
        return candidates[0]

    def dll_name(rva):
        offset = rva_offset(rva, 1)
        end = data.find(b'\0', offset, min(offset + 129, len(data)))
        if end < 0:
            raise ValueError('Unterminated PE DLL name')
        rva_offset(rva, end - offset + 1)
        name = data[offset:end].decode('ascii').lower()
        if not re.fullmatch(r'[a-z0-9_-][a-z0-9_.-]{0,123}\.dll', name):
            raise ValueError('Invalid PE DLL basename')
        return name

    names = set()
    for directory, width in [(1, 20), (13, 32)]:
        rva, size = unpack('<II', optional + 112 + directory * 8)
        if not rva and not size:
            continue
        if not rva or not width <= size <= 65536:
            raise ValueError('Invalid PE import table extent')
        offset = rva_offset(rva, size)
        terminated = False
        for index in range(min(size // width, 512)):
            words = unpack('<' + 'I' * (width // 4), offset + index * width)
            if not any(words):
                terminated = True
                break
            if directory == 1:
                name_rva = words[3]
            else:
                if words[0] not in (0, 1):
                    raise ValueError('Unknown delayed import attributes')
                name_rva = words[1] if words[0] == 1 else words[1] - image_base
            names.add(dll_name(name_rva))
        if not terminated:
            raise ValueError('Unterminated or excessive PE import descriptors')
    return sorted(names)


def require_device_free_imports(path: Path):
    names = pe_imports(path)
    if any(name in DESKTOP_DLLS or name.startswith(('sdl', 'xinput', 'd3d', 'xaudio')) for name in names):
        raise ValueError('Device-free actor statically imports a desktop/device DLL')
    return names
