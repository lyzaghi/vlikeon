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


SYSTEM32_CORE_ALLOWLIST = frozenset({
    'kernel32.dll', 'kernelbase.dll', 'ntdll.dll', 'msvcrt.dll',
    'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll',
    'ucrtbase.dll', 'advapi32.dll', 'bcryptprimitives.dll', 'bcrypt.dll',
})

HELPER_SYSTEM32_ALLOWLIST = SYSTEM32_CORE_ALLOWLIST | DESKTOP_DLLS | frozenset({
    'cfgmgr32.dll', 'version.dll', 'dwmapi.dll', 'uxtheme.dll',
    'hid.dll', 'avrt.dll', 'mmdevapi.dll', 'powrprof.dll', 'wtsapi32.dll',
})


def verify_windows_isolated_deployment(
    worker_path: Path,
    core_dll_path: Path,
    helper_path: Path | None = None,
    support_dlls: list[Path] | None = None,
) -> dict[str, object]:
    """Verify dependency closure and isolation between the AppContainer core worker and presentation host."""
    support_list = list(support_dlls or [])
    packaged_dll_names = {core_dll_path.name.lower()} | {p.name.lower() for p in support_list}

    worker_imports = require_device_free_imports(worker_path) if helper_path is not None else pe_imports(worker_path)
    core_imports = require_device_free_imports(core_dll_path) if helper_path is not None else pe_imports(core_dll_path)

    unresolved_core_deps = [
        dll for dll in core_imports
        if dll not in SYSTEM32_CORE_ALLOWLIST
        and not dll.startswith('api-ms-win-')
        and dll not in packaged_dll_names
        and (helper_path is not None or dll not in DESKTOP_DLLS)
    ]
    if unresolved_core_deps:
        raise ValueError(f'Core DLL imports unpackaged non-System32 dependencies: {unresolved_core_deps}')

    support_imports: dict[str, list[str]] = {}
    for support_dll in support_list:
        imp = require_device_free_imports(support_dll) if helper_path is not None else pe_imports(support_dll)
        support_imports[support_dll.name.lower()] = imp

    helper_imports: list[str] = []
    if helper_path is not None:
        helper_imports = pe_imports(helper_path)
        if core_dll_path.name.lower() in helper_imports:
            raise ValueError('Presentation helper must never import the emulator core DLL')
        disallowed_helper = [
            dll for dll in helper_imports
            if dll not in HELPER_SYSTEM32_ALLOWLIST and not dll.startswith('api-ms-win-')
        ]
        if disallowed_helper:
            raise ValueError(f'Presentation helper imports unapproved DLLs: {disallowed_helper}')

    return {
        'mode': 'brokered_rgba' if helper_path is not None else 'direct',
        'worker_imports': worker_imports,
        'core_imports': core_imports,
        'support_imports': support_imports,
        'helper_imports': helper_imports,
    }
