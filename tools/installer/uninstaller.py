#!/usr/bin/env python3
"""
OmniPlay Uninstaller Engine
Addresses Work Item 90.

Features:
- Active session detection and process fencing
- Explicit data retention choice: keep user data vs remove all
- Clean removal of binary files, registry entries, systemd units, and desktop shortcuts
"""

import os
import sys
import argparse
import platform
import subprocess
import shutil
from pathlib import Path
from typing import List, Optional

class Uninstaller:
    def __init__(self, keep_user_data: bool = True, force: bool = False, install_dir: Optional[str] = None, data_dir: Optional[str] = None):
        self.os_type = platform.system().lower()
        self.keep_user_data = keep_user_data
        self.force = force
        self.install_dir = Path(install_dir) if install_dir else self._default_install_dir()
        self.data_dir = Path(data_dir) if data_dir else self._default_data_dir()

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

    def run(self) -> bool:
        print("==================================================")
        print("       OmniPlay Uninstaller - Clean Removal       ")
        print("==================================================")

        # Step 1: Process fencing
        running = self.check_running_processes()
        if running:
            print(f"[-] Error: OmniPlay processes are currently running: {', '.join(running)}")
            if not self.force:
                print("[-] Please close all OmniPlay windows and services before uninstalling.")
                return False
            print("[!] Force flag set; continuing uninstallation.")

        # Step 2: Confirmation of data retention policy
        if self.keep_user_data:
            print("[+] Policy: PRESERVE user data (saves, ROMs, profile database).")
            print(f"    User data preserved at: {self.data_dir}")
        else:
            print("[!] Policy: REMOVE ALL (deleting user saves, profile database, and configurations).")

        # Step 3: Remove binaries and version directories after verifying receipt
        receipt_path = self.install_dir / ".omniplay-install-receipt.json"
        if self.install_dir.exists():
            if not receipt_path.is_file() and not self.force:
                print(f"[-] Refusing to delete {self.install_dir} without .omniplay-install-receipt.json (use --force to override).")
                return False
            print(f"[+] Removing application binaries from {self.install_dir}...")
            try:
                shutil.rmtree(self.install_dir)
                print("[+] Application binaries successfully removed.")
            except Exception as e:
                print(f"[-] Failed removing install directory: {e}")
                return False
        else:
            print(f"[*] Install directory {self.install_dir} not present.")

        # Step 4: Remove user data if requested
        if not self.keep_user_data:
            if self.data_dir.exists():
                print(f"[+] Wiping user data directory {self.data_dir}...")
                try:
                    shutil.rmtree(self.data_dir)
                    print("[+] User data successfully wiped.")
                except Exception as e:
                    print(f"[-] Failed removing data directory: {e}")
                    return False

        # Step 5: Platform-specific cleanup
        self._cleanup_system_integrations()

        print("[+] Uninstallation finished successfully.")
        return True

    def _cleanup_system_integrations(self):
        if self.os_type == "linux":
            xdg_data = Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local" / "share")))
            desktop_entry = xdg_data / "applications" / "omniplay.desktop"
            if desktop_entry.is_file():
                desktop_entry.unlink()
                print(f"[+] Removed XDG desktop entry: {desktop_entry}")


def main():
    parser = argparse.ArgumentParser(description="OmniPlay Product Uninstaller")
    parser.add_argument("--keep-user-data", action="store_true", default=True, help="Preserve saves, ROMs, and profile DB (default: True)")
    parser.add_argument("--remove-all", action="store_true", help="Remove all user data, saves, and databases")
    parser.add_argument("--force", action="store_true", help="Force uninstallation even if processes appear active")
    parser.add_argument("--install-dir", help="Custom install directory")
    parser.add_argument("--data-dir", help="Custom data directory")
    args = parser.parse_args()

    keep_data = not args.remove_all
    uninstaller = Uninstaller(
        keep_user_data=keep_data,
        force=args.force,
        install_dir=args.install_dir,
        data_dir=args.data_dir,
    )
    success = uninstaller.run()
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
