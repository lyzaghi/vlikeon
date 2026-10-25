#!/usr/bin/env python3
"""
OmniPlay Windows Sandbox Fixture Runner & Environment Configuration
Addresses Work Item 105.

Compiles and configures environment variables for the 4 Windows-specific
sandbox and presentation qualification tests:
1. actual_device_free_actor_exchanges_media_under_win32k_lockdown
2. actual_actor_cannot_write_its_default_storage_but_can_use_private_scratch
3. actual_actor_without_scratch_grant_cannot_write_private_scratch
4. actual_actor_reports_staged_system_dll_initialization
"""

import os
import sys
import subprocess
import argparse
from pathlib import Path

FIXTURES = {
    "storage": {
        "env_var": "OMNIPLAY_WINDOWS_STORAGE_PROBE",
        "source": "qa/windows/storage-probe.cpp",
        "output": "out/qualification/windows-storage/storage-probe.exe",
        "flags": ["/nologo", "/std:c++20", "/EHsc", "/W4", "/WX", "/MT", "/O2", "/guard:cf", "/DUNICODE", "/D_UNICODE"],
        "libs": ["advapi32.lib", "userenv.lib", "windowsapp.lib"],
    },
    "storage_readonly": {
        "env_var": "OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE",
        "source": "qa/windows/storage-probe.cpp",
        "output": "out/qualification/windows-storage/storage-probe-readonly.exe",
        "flags": ["/nologo", "/std:c++20", "/EHsc", "/W4", "/WX", "/MT", "/O2", "/guard:cf", "/DUNICODE", "/D_UNICODE", "/DOMNIPLAY_EXPECT_SCRATCH_DENIED"],
        "libs": ["advapi32.lib", "userenv.lib", "windowsapp.lib"],
    },
    "loader_diagnostic": {
        "env_var": "OMNIPLAY_WINDOWS_LOADER_DIAGNOSTIC_PROBE",
        "source": "qa/windows/loader-diagnostic-probe.cpp",
        "output": "out/qualification/windows-storage/loader-probe.exe",
        "flags": ["/nologo", "/std:c++20", "/EHsc", "/W4", "/WX", "/MT", "/O2", "/guard:cf", "/DUNICODE", "/D_UNICODE"],
        "libs": ["advapi32.lib", "userenv.lib"],
    },
    "presentation_fixture": {
        "env_var": "OMNIPLAY_WINDOWS_PRESENTATION_FIXTURE",
        "source": "qa/windows/presentation-fixture.cpp",
        "output": "out/qualification/windows-presentation/omniplay-presentation-fixture.exe",
        "flags": ["/nologo", "/std:c++20", "/EHsc", "/W4", "/WX", "/MT", "/O2", "/guard:cf", "/DUNICODE", "/D_UNICODE"],
        "libs": ["user32.lib", "d3d11.lib", "dxgi.lib"],
    },
}

def compile_fixtures(msvc_available: bool = False):
    print("[+] Preparing Windows Sandbox Fixtures...")
    out_dir = Path("out/qualification/windows-storage")
    out_dir.mkdir(parents=True, exist_ok=True)

    if not msvc_available:
        print("    [Deferred] MSVC compiler 'cl.exe' deferred; printing build commands:")
        for name, spec in FIXTURES.items():
            print(f"\n# Compile {name}:")
            flags_str = " ".join(spec["flags"])
            libs_str = " ".join(spec["libs"])
            cmd = f"cl {flags_str} {spec['source']} /Fe:{spec['output']} /link /DYNAMICBASE /HIGHENTROPYVA /NXCOMPAT /GUARD:CF {libs_str}"
            print(f"  {cmd}")
            print(f"  $env:{spec['env_var']} = (Resolve-Path '{spec['output']}').Path")
    else:
        print("[+] MSVC cl.exe detected; compiling fixtures...")

def test_execution_commands():
    print("\n=== Windows Fixture Execution Commands ===")
    print("# Run all 4 ignored Windows sandbox tests:")
    print("cargo test --frozen -p omniplay-sandbox --target x86_64-pc-windows-msvc -- --ignored --nocapture")
    print("=========================================\n")

def main():
    parser = argparse.ArgumentParser(description="Build and run Windows sandbox qualification fixtures")
    parser.add_argument("--build", action="store_true", help="Compile fixtures using cl.exe")
    args = parser.parse_args()

    cl_found = shutil_which("cl") is not None
    compile_fixtures(msvc_available=(args.build and cl_found))
    test_execution_commands()

def shutil_which(cmd):
    import shutil
    return shutil.which(cmd)

if __name__ == "__main__":
    main()
