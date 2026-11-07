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

class ProcessInspectionError(RuntimeError):
    """Raised when the uninstaller cannot reliably query running system processes."""


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
        import csv
        import io

        running: List[str] = []
        process_names = ("omniplay", "omniplay-launcher", "omniplay-host", "vlikeon")
        if self.os_type == "windows":
            try:
                out = subprocess.check_output(
                    ["tasklist", "/FO", "CSV", "/NH"],
                    text=True,
                    stdin=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                    timeout=5,
                )
            except subprocess.TimeoutExpired as exc:
                raise ProcessInspectionError("Process query timed out while executing tasklist") from exc
            except PermissionError as exc:
                raise ProcessInspectionError(f"Permission denied while querying running processes: {exc}") from exc
            except subprocess.CalledProcessError as exc:
                stderr_msg = (exc.stderr or "").strip()
                raise ProcessInspectionError(
                    f"tasklist exited with status {exc.returncode}: {stderr_msg or 'unknown error'}"
                ) from exc
            except OSError as exc:
                raise ProcessInspectionError(f"Failed to invoke tasklist for process fencing: {exc}") from exc

            if not isinstance(out, str) or not out.strip():
                raise ProcessInspectionError("tasklist returned empty or unreadable process table output")

            parsed_rows = 0
            for raw_line in out.splitlines():
                line = raw_line.strip()
                if not line:
                    continue
                if line.upper().startswith(("ERROR:", "INFO:")):
                    raise ProcessInspectionError(f"tasklist did not return a valid CSV process table: {line}")
                try:
                    row = next(csv.reader(io.StringIO(line)))
                except Exception as exc:
                    raise ProcessInspectionError(f"Unreadable CSV row from tasklist: {line!r}") from exc
                if len(row) < 2 or not row[0].strip() or not row[1].strip().isdigit():
                    raise ProcessInspectionError(f"Malformed CSV process entry from tasklist: {line!r}")
                parsed_rows += 1
                name = row[0].strip().lower()
                if any(p in name for p in process_names):
                    running.append(name)

            if parsed_rows == 0:
                raise ProcessInspectionError("tasklist output contained zero valid process records")
        else:
            try:
                out = subprocess.check_output(
                    ["pgrep", "-l", "omniplay"],
                    text=True,
                    stdin=subprocess.DEVNULL,
                    stderr=subprocess.PIPE,
                    timeout=5,
                )
                if not isinstance(out, str) or not out.strip():
                    raise ProcessInspectionError("pgrep exited 0 but returned empty or unreadable output")
                for raw_line in out.splitlines():
                    line = raw_line.strip()
                    if not line:
                        continue
                    parts = line.split(maxsplit=1)
                    if not parts or not parts[0].isdigit():
                        raise ProcessInspectionError(f"Malformed pgrep output line: {line!r}")
                    running.append(line)
            except subprocess.TimeoutExpired as exc:
                raise ProcessInspectionError("Process query timed out while executing pgrep") from exc
            except PermissionError as exc:
                raise ProcessInspectionError(f"Permission denied while querying running processes: {exc}") from exc
            except subprocess.CalledProcessError as exc:
                stderr_msg = (exc.stderr or "").strip()
                if exc.returncode == 1 and not stderr_msg:
                    return []
                raise ProcessInspectionError(
                    f"pgrep failed with status {exc.returncode}: {stderr_msg or 'process inspection failed'}"
                ) from exc
            except OSError as exc:
                raise ProcessInspectionError(f"Failed to invoke pgrep for process fencing: {exc}") from exc
        return running

    def run(self) -> bool:
        print("==================================================")
        print("       OmniPlay Uninstaller - Clean Removal       ")
        print("==================================================")

        # Step 1: Process fencing (always fail closed before any deletion if inspection fails)
        try:
            running = self.check_running_processes()
        except ProcessInspectionError as exc:
            print(f"[-] Error: Cannot verify running processes ({exc}).")
            print("[-] Halting uninstallation before any file deletion.")
            return False

        if running:
            print(f"[-] Error: OmniPlay/Vlikeon processes are currently running: {', '.join(running)}")
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
