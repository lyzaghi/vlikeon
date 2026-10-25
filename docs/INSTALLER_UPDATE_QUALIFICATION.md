# OmniPlay Installer, Repair, Update & Maintenance Qualification (Group K, Items 88–95)

This document establishes the qualification matrix, operational procedures, and deferred verification queue for OmniPlay desktop installation, repair, uninstallation, bootstrap maintenance, and packaging.

---

## 1. Group K Architecture Overview

| Item ID | Component | Implementation File(s) | Role & Invariants |
|:---|:---|:---|:---|
| **88** | Installer UI / Wizard | `tools/installer/installer_wizard.py` | Cross-platform wizard; supports interactive and silent unattended installs; EULA verification; platform paths. |
| **89** | Exact Signed Repair | `platform/updater/src/repair.rs` | Verifies release file hashes; selectively unpacks only corrupted files from signed package; atomic replacement. |
| **90** | Uninstaller & Retention | `tools/installer/uninstaller.py` | Active process fencing; choice of `--keep-user-data` vs `--remove-all`; clean registry and desktop shortcut removal. |
| **91** | Windows Bootstrap Maintenance | `tools/windows_bootstrap_maintenance.ps1` | Authenticode signature validation; per-user registry management for `omniplay://` and Add/Remove Programs. |
| **92** | Linux Package Manager | `packaging/linux/debian/`, `rpm/`, `flatpak/` | Debian (`.deb`), RPM (`.spec`), Flatpak manifests with systemd user unit activation and XDG desktop integration. |
| **93** | macOS Signed/Notarized Updates | `packaging/macos/Info.plist`, `Entitlements.plist`, `tools/package_macos_bundle.py` | Hardened Runtime bundle structure, App Store / Developer ID codesigning, notarytool submission instructions. |
| **94** | Quota, Staging & Release Pruning | `platform/updater/src/cleanup.rs` | Sweeps stale staging directories older than 24h; prunes retired releases beyond keep count (preserving active + rollback). |
| **95** | Full Qualification Matrix | `qa/installer_qualification.py` | Automated qualification suite verifying unattended install, repair, uninstall, staging sweep, and package manifests. |

---

## 2. Platform Directory Layout

| Platform | Default Binary Install Path | Default User Data Path |
|:---|:---|:---|
| **Windows** | `%LOCALAPPDATA%\OmniPlay\install` | `%APPDATA%\OmniPlay` |
| **macOS** | `~/Applications/OmniPlay.app` | `~/Library/Application Support/OmniPlay` |
| **Linux** | `~/.local/share/omniplay` | `~/.config/omniplay` |

---

## 3. Automated Qualification Suite

Run the automated test suite with:
```bash
py qa/installer_qualification.py
```

### Verified Test Cases:
1. `test_installer_wizard_unattended`: Confirms silent install correctly enforces `--accept-eula` and establishes target directories.
2. `test_uninstaller_retention_and_fencing`: Verifies process fencing and confirms user saves are preserved under `--keep-user-data` and wiped under `--remove-all`.
3. `test_repair_file_corruption`: Verifies corrupted file detection and exact byte-for-byte repair from authentic package.
4. `test_staging_sweep_and_pruning`: Verifies 24-hour TTL sweep on staging directories and old version pruning.
5. `test_linux_packaging_specs`: Confirms Debian control, postinst, prerm, RPM spec, and Flatpak manifests are structurally valid.
6. `test_macos_packaging_specs`: Confirms `Info.plist` and hardened runtime `Entitlements.plist` enforce strict JIT restrictions (`allow-jit: false`).
7. `test_windows_bootstrap_script`: Confirms `windows_bootstrap_maintenance.ps1` correctly provisions and cleans registry keys.

---

## 4. Acceptance Queue & Deferred Execution

In accordance with project policy, physical hardware package building (dpkg-buildpackage, rpmbuild, flatpak-builder), live codesigning (Authenticode, Apple Developer ID), and live macOS notarytool submissions are deferred to the final consolidated release pass.

| Item ID | Status | Deferred Dependency |
|:---|:---|:---|
| **88** | `SOURCE_READY_UNVERIFIED` | GUI frontend integration on physical displays |
| **89** | `SOURCE_READY_UNVERIFIED` | Production signed package server |
| **90** | `SOURCE_READY_UNVERIFIED` | End-to-end OS installer unregister hooks |
| **91** | `SOURCE_READY_UNVERIFIED` | Live Authenticode production certificate |
| **92** | `SOURCE_READY_UNVERIFIED` | Linux packaging build farms (Debian, Fedora, Flatpak) |
| **93** | `SOURCE_READY_UNVERIFIED` | Apple Developer ID certificate & Notarization credentials |
| **94** | `SOURCE_READY_UNVERIFIED` | Multi-gigabyte disk volume qualification |
| **95** | `WAITING_PERMISSION_OR_RESOURCE` | Live OS clean-install VMs (Windows 11, macOS Sonoma, Ubuntu 24.04) |
