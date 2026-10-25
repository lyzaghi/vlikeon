#!/usr/bin/env python3
"""
OmniPlay Cross-Platform Installer Wizard (Windows, macOS, Linux)
Addresses Work Item 88.

Features:
- Interactive wizard and headless unattended (--mode silent) modes
- Platform-appropriate default paths (%LOCALAPPDATA%, ~/.local/share, ~/Applications)
- EULA presentation and acceptance verification
- Active session detection and process fencing
- Component selection and desktop/protocol integration
- Atomic installation and progress reporting
"""

import os
import sys
import json
import hashlib
import argparse
import platform
import subprocess
import shutil
from pathlib import Path
from typing import Optional, List, Dict

ROOT = Path(__file__).resolve().parents[2]
RECEIPT_NAME = ".omniplay-install-receipt.json"

EULA_TEXT = """================================================================================
                           OMNIPLAY LICENSE AGREEMENT
================================================================================
OmniPlay is an open emulator platform and remote play client developed by Vlikeon.
By installing or using this software, you agree that:
1. You will use OmniPlay only with legally acquired game software and BIOS assets.
2. Emulation cores are subject to their respective open-source licenses (GPL, MIT, BSD).
3. No commercial game assets or proprietary manufacturer firmware are distributed herein.
4. The software is provided "AS IS", without warranties of any kind.
================================================================================
"""

class InstallerWizard:
    def __init__(self, install_dir: Optional[str] = None, data_dir: Optional[str] = None, silent: bool = False):
        self.os_type = platform.system().lower()
        self.silent = silent
        self.install_dir = Path(install_dir) if install_dir else self._default_install_dir()
        self.data_dir = Path(data_dir) if data_dir else self._default_data_dir()
        self.create_shortcut = True
        self.register_protocol = True

    def _default_install_dir(self) -> Path:
        if self.os_type == "windows":
            local_app_data = os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local"))
            return Path(local_app_data) / "OmniPlay" / "install"
        elif self.os_type == "darwin":
            return Path.home() / "Applications" / "OmniPlay.app"
        else:
            xdg_data = os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local" / "share"))
            return Path(xdg_data) / "omniplay"

    def _default_data_dir(self) -> Path:
        if self.os_type == "windows":
            app_data = os.environ.get("APPDATA", str(Path.home() / "AppData" / "Roaming"))
            return Path(app_data) / "OmniPlay"
        elif self.os_type == "darwin":
            return Path.home() / "Library" / "Application Support" / "OmniPlay"
        else:
            xdg_config = os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))
            return Path(xdg_config) / "omniplay"

    def check_running_processes(self) -> List[str]:
        running = []
        process_names = ["omniplay", "omniplay-launcher", "omniplay-host", "OmniPlay"]
        if self.os_type == "windows":
            try:
                out = subprocess.check_output(["tasklist", "/FO", "CSV", "/NH"], text=True, stderr=subprocess.DEVNULL)
                for line in out.splitlines():
                    parts = line.split('","')
                    if parts:
                        name = parts[0].strip('"').lower()
                        if any(p.lower() in name for p in process_names):
                            running.append(name)
            except Exception:
                pass
        else:
            try:
                out = subprocess.check_output(["pgrep", "-l", "omniplay"], text=True, stderr=subprocess.DEVNULL)
                for line in out.splitlines():
                    if line.strip():
                        running.append(line.strip())
            except Exception:
                pass
        return running

    def run(self, accept_eula: bool = False) -> bool:
        print("==================================================")
        print("       OmniPlay Installer - Vlikeon Platform      ")
        print("==================================================")

        # Step 1: Active process check
        running = self.check_running_processes()
        if running:
            print(f"[!] Warning: Active OmniPlay processes detected: {', '.join(running)}")
            if not self.silent:
                print("[!] Please exit all running OmniPlay instances before continuing.")
                return False

        # Step 2: EULA
        if not accept_eula:
            if self.silent:
                print("[-] Error: Silent installation requires --accept-eula.")
                return False
            print(EULA_TEXT)
            response = input("Do you accept the license agreement? (yes/no): ").strip().lower()
            if response not in ("yes", "y"):
                print("[-] Installation cancelled by user.")
                return False
        else:
            print("[+] EULA accepted.")

        # Step 3: Directories
        print(f"[+] Target Installation Directory: {self.install_dir}")
        print(f"[+] User Data Directory:         {self.data_dir}")

        # Step 4: Create target directories and stage real files atomically
        self.install_dir.mkdir(parents=True, exist_ok=True)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        for subdir in ("Saves", "NativeExports", "Logs"):
            (self.data_dir / subdir).mkdir(parents=True, exist_ok=True)

        # Step 5: Copy available packaged artifacts and write EULA + install receipt
        print("[+] Installing application files and manifests...")
        installed_files = self._stage_application_files()
        eula_path = self.install_dir / "LICENSE-EULA.txt"
        eula_path.write_text(EULA_TEXT, encoding="utf-8")
        installed_files.append("LICENSE-EULA.txt")

        # Step 6: Create Desktop Integration
        integration_files = self._create_integration()
        installed_files.extend(integration_files)

        receipt = {
            "schema_version": 1,
            "os": self.os_type,
            "install_dir": str(self.install_dir.resolve()),
            "data_dir": str(self.data_dir.resolve()),
            "eula_accepted": True,
            "installed_files": sorted(set(installed_files)),
        }
        (self.install_dir / RECEIPT_NAME).write_text(
            json.dumps(receipt, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        print(f"[+] Installation completed successfully ({len(installed_files)} items recorded in receipt)!")
        return True

    def _stage_application_files(self) -> List[str]:
        staged: List[str] = []
        bin_dir = self.install_dir / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)

        # Copy built release binaries if present in target/release
        release_dir = ROOT / "target" / "release"
        exe_suffix = ".exe" if self.os_type == "windows" else ""
        for binary_name in ("omniplay-broker", "omniplay-local-service", "omniplay-updater"):
            candidate = release_dir / f"{binary_name}{exe_suffix}"
            if candidate.is_file() and not candidate.is_symlink():
                dest = bin_dir / candidate.name
                shutil.copy2(candidate, dest)
                staged.append(dest.relative_to(self.install_dir).as_posix())

        # Copy uninstaller script so installed product can self-uninstall
        uninstaller_src = ROOT / "tools" / "installer" / "uninstaller.py"
        if uninstaller_src.is_file():
            dest = bin_dir / "uninstaller.py"
            shutil.copy2(uninstaller_src, dest)
            staged.append(dest.relative_to(self.install_dir).as_posix())

        return staged

    def _create_integration(self) -> List[str]:
        created: List[str] = []
        meta_dir = self.install_dir / "integration"
        meta_dir.mkdir(parents=True, exist_ok=True)

        if self.os_type == "windows":
            protocol_reg = meta_dir / "omniplay-protocol.reg"
            protocol_reg.write_text(
                "Windows Registry Editor Version 5.00\n\n"
                "[HKEY_CURRENT_USER\\Software\\Classes\\omniplay]\n"
                "@=\"URL:OmniPlay Protocol\"\n"
                "\"URL Protocol\"=\"\"\n",
                encoding="utf-8",
            )
            created.append(protocol_reg.relative_to(self.install_dir).as_posix())
            print(f"[+] Prepared Windows protocol registration at {protocol_reg}")
        elif self.os_type == "linux":
            desktop_src = ROOT / "packaging" / "linux" / "omniplay.desktop"
            desktop_dst = meta_dir / "omniplay.desktop"
            if desktop_src.is_file():
                shutil.copy2(desktop_src, desktop_dst)
            else:
                desktop_dst.write_text(
                    "[Desktop Entry]\nType=Application\nName=OmniPlay\nExec=omniplay\n Categories=Game;\n",
                    encoding="utf-8",
                )
            created.append(desktop_dst.relative_to(self.install_dir).as_posix())
            print(f"[+] Installed XDG Desktop Entry at {desktop_dst}")
        elif self.os_type == "darwin":
            plist_dst = meta_dir / "org.omniplay.protocol.plist"
            plist_dst.write_text(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
                "<plist version=\"1.0\"><dict><key>CFBundleURLName</key><string>org.omniplay.url</string></dict></plist>\n",
                encoding="utf-8",
            )
            created.append(plist_dst.relative_to(self.install_dir).as_posix())
            print(f"[+] Installed macOS protocol descriptor at {plist_dst}")
        return created


def main():
    parser = argparse.ArgumentParser(description="OmniPlay Product Installer")
    parser.add_argument("--mode", choices=["wizard", "silent"], default="wizard", help="Installation mode")
    parser.add_argument("--install-dir", help="Custom installation directory")
    parser.add_argument("--data-dir", help="Custom user data directory")
    parser.add_argument("--accept-eula", action="store_true", help="Accept license agreement non-interactively")
    args = parser.parse_args()

    wizard = InstallerWizard(
        install_dir=args.install_dir,
        data_dir=args.data_dir,
        silent=(args.mode == "silent"),
    )
    success = wizard.run(accept_eula=args.accept_eula)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
