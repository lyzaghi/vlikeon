"""Shared process guard for the installer and the staged uninstaller."""
import csv
from pathlib import Path
import subprocess


class ProcessInspectionError(RuntimeError):
    """The process table or a relevant process could not be inspected."""


def _windows_image_path(pid: int) -> Path | None:
    import ctypes
    from ctypes import wintypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.QueryFullProcessImageNameW.argtypes = (
        wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD),
    )
    kernel.QueryFullProcessImageNameW.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = (wintypes.HANDLE,)
    kernel.CloseHandle.restype = wintypes.BOOL
    process = kernel.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not process:
        error = ctypes.get_last_error()
        if error == 87:  # PID disappeared between tasklist and OpenProcess.
            return None
        raise ctypes.WinError(error)
    try:
        path = ctypes.create_unicode_buffer(32768)
        length = wintypes.DWORD(len(path))
        if not kernel.QueryFullProcessImageNameW(process, 0, path, ctypes.byref(length)):
            raise ctypes.WinError(ctypes.get_last_error())
        image = Path(path.value)
        if not image.is_absolute():
            raise OSError("Windows did not return an absolute executable path")
        return image
    finally:
        kernel.CloseHandle(process)


def _windows_processes(output: str, install_dir: Path) -> list[str]:
    rows = list(csv.reader(output.splitlines(), strict=True))
    if not rows:
        raise ProcessInspectionError("tasklist returned no process records")
    running = []
    install_root = install_dir.resolve()
    for row in rows:
        if len(row) != 5 or not all(field.strip() for field in row):
            raise ProcessInspectionError(f"Malformed tasklist CSV record: {row!r}")
        name, pid, _, session, _ = (field.strip() for field in row)
        if not (pid.isascii() and pid.isdecimal() and session.isascii() and session.isdecimal()):
            raise ProcessInspectionError(f"Malformed tasklist PID or session: {row!r}")
        name = name.casefold()
        if "omniplay" in name or "vlikeon" in name:
            running.append(name)
        elif name == "chrome.exe":
            image = _windows_image_path(int(pid))
            if image is not None and image.resolve().is_relative_to(install_root):
                running.append(name)
    return running


def check_running_processes(os_type: str, install_dir: Path) -> list[str]:
    windows = os_type == "windows"
    command = ["tasklist", "/FO", "CSV", "/NH"] if windows else ["pgrep", "-il", "omniplay|vlikeon"]
    try:
        output = subprocess.check_output(
            command, text=True, stdin=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=5,
        )
        if not output.strip():
            raise ProcessInspectionError(f"{command[0]} returned no readable process records")
        if windows:
            return _windows_processes(output, install_dir)

        running = []
        for line in output.splitlines():
            pid, _, name = line.partition(" ")
            if not (pid.isascii() and pid.isdecimal() and name.strip()):
                raise ProcessInspectionError(f"Malformed pgrep process record: {line!r}")
            running.append(line)
        return running
    except subprocess.CalledProcessError as exc:
        if not windows and exc.returncode == 1 and not (exc.stderr or "").strip() and not (exc.stdout or "").strip():
            return []
        raise ProcessInspectionError(f"{command[0]} failed with status {exc.returncode}") from exc
    except (subprocess.TimeoutExpired, OSError, UnicodeError, csv.Error, ValueError) as exc:
        raise ProcessInspectionError(f"Cannot inspect {command[0]} process table: {exc}") from exc
