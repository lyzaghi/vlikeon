#!/usr/bin/env python3
"""
Android 16 KiB Page Size ELF & APK Alignment Verification Tool.
Validates that:
1. All 64-bit ELF shared libraries (.so) have PT_LOAD segments aligned to at least 16384 bytes (0x4000).
2. p_vaddr % 0x4000 == p_offset % 0x4000 for every PT_LOAD segment.
3. In APK archives, uncompressed shared libraries are aligned to 16 KiB boundaries.
"""

import os
import struct
import sys
import zipfile
from pathlib import Path

ELF_MAGIC = b"\x7fELF"
EI_CLASS_64 = 2
PT_LOAD = 1
PAGE_SIZE_16K = 16384  # 0x4000


def check_elf_alignment(file_path: Path) -> tuple[bool, str]:
    """Inspects a 64-bit ELF binary for 16 KiB segment alignment."""
    with open(file_path, "rb") as f:
        data = f.read(64)
        if len(data) < 64:
            return False, f"File too small ({len(data)} bytes)"
        if not data.startswith(ELF_MAGIC):
            return False, "Not an ELF file (magic mismatch)"
        if data[4] != EI_CLASS_64:
            return True, "32-bit ELF (16 KiB alignment check applies to 64-bit arm64/x86_64)"

        # 64-bit ELF header
        # e_phoff is at offset 32 (8 bytes), e_phentsize at 54 (2 bytes), e_phnum at 56 (2 bytes)
        e_phoff = struct.unpack_from("<Q", data, 32)[0]
        e_phentsize = struct.unpack_from("<H", data, 54)[0]
        e_phnum = struct.unpack_from("<H", data, 56)[0]

        f.seek(e_phoff)
        ph_data = f.read(e_phentsize * e_phnum)

        load_segments = 0
        for i in range(e_phnum):
            ph = ph_data[i * e_phentsize : (i + 1) * e_phentsize]
            p_type = struct.unpack_from("<I", ph, 0)[0]
            if p_type == PT_LOAD:
                load_segments += 1
                # In Elf64_Phdr:
                # p_type (4), p_flags (4), p_offset (8), p_vaddr (8), p_paddr (8), p_filesz (8), p_memsz (8), p_align (8)
                p_offset = struct.unpack_from("<Q", ph, 8)[0]
                p_vaddr = struct.unpack_from("<Q", ph, 16)[0]
                p_align = struct.unpack_from("<Q", ph, 48)[0]

                if p_align < PAGE_SIZE_16K:
                    return False, (
                        f"PT_LOAD segment {i} alignment {p_align} < 16384 (0x4000) "
                        f"[offset=0x{p_offset:x}, vaddr=0x{p_vaddr:x}]"
                    )
                if (p_vaddr % PAGE_SIZE_16K) != (p_offset % PAGE_SIZE_16K):
                    return False, (
                        f"PT_LOAD segment {i} congruence violation: "
                        f"(vaddr 0x{p_vaddr:x} % 0x4000 = {p_vaddr % PAGE_SIZE_16K}) != "
                        f"(offset 0x{p_offset:x} % 0x4000 = {p_offset % PAGE_SIZE_16K})"
                    )

        if load_segments == 0:
            return False, "No PT_LOAD segments found in ELF"

    return True, f"OK ({load_segments} PT_LOAD segments aligned to >= 16 KiB)"


def check_apk_zip_alignment(apk_path: Path) -> tuple[bool, list[str]]:
    """Checks that uncompressed .so files inside the APK start at 16 KiB aligned offsets."""
    errors = []
    with open(apk_path, "rb") as f:
        with zipfile.ZipFile(f, "r") as z:
            for info in z.infolist():
                if info.filename.endswith(".so"):
                    # Check compression
                    if info.compress_type != zipfile.ZIP_STORED:
                        errors.append(
                            f"{info.filename}: Shared library must be uncompressed (ZIP_STORED), "
                            f"found compress_type={info.compress_type}"
                        )
                        continue
                    # Find offset of data in zip file
                    header_offset = info.header_offset
                    f.seek(header_offset)
                    local_header = f.read(30)
                    if len(local_header) < 30 or not local_header.startswith(b"PK\x03\x04"):
                        errors.append(f"{info.filename}: Invalid local zip header")
                        continue
                    n_len = struct.unpack_from("<H", local_header, 26)[0]
                    e_len = struct.unpack_from("<H", local_header, 28)[0]
                    data_offset = header_offset + 30 + n_len + e_len
                    if data_offset % PAGE_SIZE_16K != 0:
                        errors.append(
                            f"{info.filename}: Data offset 0x{data_offset:x} is not 16 KiB aligned "
                            f"(offset % 16384 = {data_offset % PAGE_SIZE_16K})"
                        )

    return len(errors) == 0, errors


def main():
    if len(sys.argv) < 2:
        print("Usage: verify_android_16k_alignment.py <path-to-elf-so-or-apk> [...]")
        sys.exit(1)

    all_pass = True
    for arg in sys.argv[1:]:
        p = Path(arg)
        if not p.exists():
            print(f"[SKIP] File not found: {p}")
            continue

        if p.suffix == ".apk":
            print(f"Checking APK zip alignment: {p}")
            ok, errors = check_apk_zip_alignment(p)
            if ok:
                print(f"  [PASS] All uncompressed .so libraries in APK are 16 KiB aligned")
            else:
                all_pass = False
                print(f"  [FAIL] APK zip alignment issues:")
                for err in errors:
                    print(f"    - {err}")
        elif p.suffix == ".so" or p.name.startswith("lib"):
            print(f"Checking ELF binary: {p}")
            ok, msg = check_elf_alignment(p)
            if ok:
                print(f"  [PASS] {msg}")
            else:
                all_pass = False
                print(f"  [FAIL] {msg}")
        else:
            print(f"[SKIP] Unrecognized file type: {p}")

    sys.exit(0 if all_pass else 1)


if __name__ == "__main__":
    main()
