"""Freeze the unbuilt fail-closed macOS App Sandbox source foundation."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
FILES = [
    "platform/contracts/src/input.rs", "engines/native/src/input_profile.hpp",
    "engines/native/src/presentation_input.hpp", "engines/native/src/engine.cpp",
    "docs/ADVANCED_INPUT_CUSTOMIZATION.md",
    "Cargo.toml", "Cargo.lock", "platform/sandbox/Cargo.toml",
    "platform/sandbox/src/lib.rs", "platform/native-runtime/Cargo.toml",
    "platform/native-runtime/src/lib.rs", "platform/native-runtime/src/registry.rs",
    "platform/native-runtime/src/mac_policy.rs", "engines/native/CMakeLists.txt",
    "engines/native/macos-actor.entitlements", "engines/presentation/CMakeLists.txt",
    "engines/presentation/macos-presentation.entitlements",
    "engines/macos-owner/macos-owner.entitlements",
    "engines/macos-owner/CMakeLists.txt", "engines/macos-owner/Info.plist.in",
    "engines/macos-owner/src/main.mm", "engines/macos-owner/src/session.hpp",
    "engines/macos-owner/src/session.mm", "engines/macos-owner/src/catalog.hpp",
    "engines/macos-owner/src/catalog.mm", "engines/macos-owner/src/process.hpp",
    "engines/macos-owner/src/process.mm", "tools/author_macos_runtime_catalog.py",
    "docs/MACOS_NATIVE_CONTAINMENT.md", "tools/prepare_macos_containment.py",
    "docs/MACOS_ITEM4_CLOSURE.md",
    "engines/common/macos_child.hpp", "platform/sandbox/build.rs",
    "platform/sandbox/native/macos_xpc.h", "platform/sandbox/native/macos_xpc.mm",
    "platform/sandbox/src/macos.rs", "platform/native-runtime/src/mac_package.rs",
    "platform/native-runtime/src/mac_staging.rs", "platform/native-runtime/src/mac_session.rs",
    "platform/native-runtime/src/mac_adapter.rs", "platform/native-runtime/src/channel.rs",
    "platform/broker/src/main.rs", "engines/native/src/main.cpp",
    "engines/presentation/src/main.cpp", "engines/presentation/src/owner.cpp",
    "tools/assemble_macos_broker.py",
    "engines/native/src/core_module.hpp", "engines/native/src/core_module.cpp",
    "tools/package_release.py",
    "platform/native-runtime/src/mac_work.rs", "platform/core/src/native_work_storage.rs",
    "engines/macos-owner/src/boot.hpp", "engines/macos-owner/src/boot.mm",
    "docs/MACOS_BOOT_RECOVERY.md",
    "engines/macos-owner/src/virtual_machine.hpp", "engines/macos-owner/src/virtual_machine.mm",
    "engines/macos-guest/CMakeLists.txt", "engines/macos-guest/init.c", "engines/macos-guest/kernel.config",
    "tools/author_macos_guest.py", "platform/native-runtime/src/mac_guest.rs",
    "docs/MACOS_DISKLESS_GUEST.md",
]


def ordinary(path, base):
    if not path.resolve().is_relative_to(base.resolve()):
        raise ValueError("macOS source must remain in its declared workspace root")
    for parent in [path, *path.parents]:
        if parent.is_symlink() or getattr(parent, "is_junction", lambda: False)():
            raise ValueError("macOS source ancestry cannot contain links")
        if parent == base:
            break


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    args = parser.parse_args()
    project = args.project.absolute()
    ordinary(project, ROOT)
    if not project.resolve().is_relative_to((ROOT / "out").resolve()) or project.exists():
        raise ValueError("Use a fresh workspace output project")
    source = project / "source"
    source.mkdir(parents=True)
    inventory = []
    total = 0
    for name in FILES:
        original = ROOT / name
        ordinary(original, ROOT)
        if not original.is_file():
            raise ValueError("A macOS containment source input is missing")
        size = original.stat().st_size
        total += size
        if size > 8 * 1024 * 1024 or total > 32 * 1024 * 1024:
            raise ValueError("macOS containment source exceeds its bound")
        target = source / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(original, target)
        inventory.append({"path": name, "bytes": size,
                          "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})
    receipt = {
        "schema_version": 1, "source_only": True, "target": "macos-aarch64",
        "source_files": len(inventory), "source_bytes": total,
        "source_inventory": inventory, "app_sandbox_policy_authored": True,
        "hardened_runtime_policy_authored": True,
        "actor_network_entitlement": False, "actor_jit_entitlement": False,
        "actor_unsigned_executable_memory_entitlement": False,
        "actor_disable_library_validation_entitlement": False,
        "authenticated_xpc_endpoint_source_authored": True,
        "bounded_xpc_session_staging_source_authored": True,
        "signed_bundle_runtime_admission_source_authored": True,
        "xpc_session_launch_authored": True, "sandbox_launch_authored": True,
        "xpc_session_launch_enabled": True, "profile_binding_source_authored": True,
        "bounded_save_channel_source_authored": True,
        "macos_adapter_source_authored": True,
        "protected_boot_journal_source_authored": True,
        "previous_boot_recovery_source_authored": True,
        "same_boot_independent_child_termination_authored": False,
        "suspended_pair_owner_source_authored": True,
        "staged_receipt_schema": 3, "staged_owner_lease_source_authored": True,
        "durable_launch_fence_source_authored": True,
        "product_enabled": False, "configuration": "not_started",
        "compiler": "not_started", "codesign": "not_started",
        "notarization": "not_started", "verification": "not_run",
        "diskless_guest_source_authored": True, "runtime_catalog_schema": 2,
        "activation_authorized_before_qualification": True,
        "remaining": "target build/signing/notarization/qualification; actual guest/core artifacts (item 5); unknown legacy metadata retained by policy",
    }
    (project / "preparation-receipt.json").write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("Froze macOS containment source. No Xcode, compiler, codesign, helper or test was started.")


if __name__ == "__main__":
    main()
