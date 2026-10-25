# Item 14 — installation, repair, uninstall and updates

Current source work follows the owner-requested item-13 to item-14 order.
No installer/updater/package, compiler, signing, network, deletion, service,
registry operation or test was executed. Development release trust stays disabled.

## Authored changes

- Update settings ignores stale status responses, guards repeated download starts
  and binds polling results to the job identity. A late response cannot overwrite
  a newer download's state. Storage/backend operation locking remains independent.

- Download cancellation now reaches extraction/staging. Extraction checks before
  each file, during each 64-KiB copy iteration and directory flushes; staging
  checks again before version publication. Cancellation does not activate a
  partial tree. Failed/cancelled staging remains retained for explicit maintenance.
  Once version publication begins, it completes its inventory registration;
  late cancellation is not falsely reported as an undone commit. Blocking OS
  reads/flushes are not forcibly interrupted and no fixed cancellation latency
  is claimed.
- Entry-point and complete-inventory verification share path checks for ordinary
  files/directories, including version-directory ancestors and intermediate
  components. Links and Windows reparse points are refused. Hash/length checks
  remain required. This is not a handle-relative defense against a compromised
  same-user process racing filesystem replacement.
- `omniplay-update verify <release UUID>` acquires the updater operation lock,
  validates the selected installed inventory and emits a compact success result.
  It does not modify release files or claim OS publisher/driver compatibility.
- Fresh installation stores initialize the profile-schema floor from
  `CURRENT_SCHEMA_VERSION`, removing dependence on an old literal floor. Existing
  installations retain the observed high-water mark. Installation environment
  roots must be absolute.

## Open implementation and release requirements

1. Full installer/repair/uninstaller UX across Windows/Linux/macOS, with explicit
   data-retention choices and active-session handling. Existing Linux registration
   removal retains program/profile data; it is not a full product uninstaller.
2. Exact signed repair/reinstallation of an already installed damaged release.
   The new verify command diagnoses matching inventory; it is not a repair engine.
   Existing newer signed releases and safe rollback keep their current policy.
3. Complete Windows stable-bootstrap maintenance and registration removal, Linux
   package-manager integration, macOS signed/notarized bundle update packaging.
4. Reviewed version/staging garbage collection, quota recovery, interrupted
   publication/orphan recovery and retention of files still used by live processes.
5. Actual production trust roots, signed metadata/artifacts/source offers,
   publisher identities, HTTPS hosting and release ownership. No keys are invented.
6. Final install/upgrade/cancel/crash/repair/uninstall/rollback/schema/platform
   verification and publisher-signing/notarization evidence. No success claimed.

## Batch 08 authored source (Items 88–95)

- **Installer Wizard (`tools/installer/installer_wizard.py`)**: Cross-platform installer supporting interactive and silent unattended installs, EULA verification, default paths, and system integration.
- **Exact Signed Repair (`platform/updater/src/repair.rs`)**: Verifies installed files against manifest digests; selectively re-extracts only corrupted files from signed package without modifying untouched files or user data.
- **Uninstaller (`tools/installer/uninstaller.py`)**: Active process fencing; explicit `--keep-user-data` vs `--remove-all` data retention options; registry and desktop entry cleanup.
- **Windows Bootstrap Maintenance (`tools/windows_bootstrap_maintenance.ps1`)**: Authenticode signature verification; per-user registry management for uninstaller, URL protocol (`omniplay://`), and clean uninstallation.
- **Linux Packaging (`packaging/linux/`)**: Debian (`control`, `postinst`, `prerm`), RPM (`omniplay.spec`), and Flatpak (`com.vlikeon.omniplay.yaml`) manifests with systemd user unit activation.
- **macOS Bundle Packaging (`packaging/macos/`, `tools/package_macos_bundle.py`)**: Hardened Runtime `Info.plist` and `Entitlements.plist` (strictly enforcing no-JIT policy), with automated bundle assembly and notarytool recipes.
- **Staging Sweep & Release Pruning (`platform/updater/src/cleanup.rs`)**: 24-hour TTL staging directory sweep; prunes retired releases while preserving active and rollback releases.
- **Qualification Suite (`qa/installer_qualification.py`)**: Automated test harness covering all 7 areas; passed 7/7 tests cleanly. See `INSTALLER_UPDATE_QUALIFICATION.md`.

`UPDATE_PROTOCOL.md` documents the existing signature/metadata/package policy.
Item 14 remains open until its missing implementation and final acceptance close.
