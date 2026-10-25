"""Recipe for the macOS diskless Linux ARM64 guest: PID 1, device-free worker, kernel, initramfs (Item 18).

Cross-builds with explicit toolchain files only; no host-library discovery or downloads.
The kernel is built from an explicit, pinned, already-checked-out source tree with
engines/macos-guest/kernel.config, and its provenance is recorded before authoring.
The initramfs is then written by tools/author_macos_guest.py. Nothing is signed/run.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
HEX = re.compile(r"[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(command):
    subprocess.run([str(value) for value in command], check=True)


def single(root, name):
    found = [p for p in root.rglob(name) if p.is_file() and not p.is_symlink()]
    if len(found) != 1:
        raise ValueError(f"Expected exactly one {name} in {root}")
    return found[0]


def build_kernel(args, work):
    source = args.kernel_source.resolve(strict=True)
    config = ROOT / "engines/macos-guest/kernel.config"
    revision = subprocess.check_output([args.git, "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if not COMMIT.fullmatch(revision) or revision != args.kernel_commit:
        raise ValueError("Kernel source revision differs from the pinned commit")
    if subprocess.check_output([args.git, "-C", str(source), "status", "--porcelain"], text=True).strip():
        raise ValueError("Kernel source tree must be clean")
    out = work / "kernel-build"
    out.mkdir(parents=True)
    shutil.copyfile(config, out / ".config")
    make = [args.make, "-C", source, f"O={out}", "ARCH=arm64", f"CROSS_COMPILE={args.cross_compile}"]
    run(make + ["olddefconfig"])
    # A silent olddefconfig rewrite must not weaken the reviewed configuration.
    wanted = {line.strip() for line in config.read_text().splitlines() if line.startswith("CONFIG_") and "=" in line}
    final = {line.strip() for line in (out / ".config").read_text().splitlines()}
    missing = sorted(wanted - final)
    if missing:
        raise ValueError(f"Kernel configuration lost reviewed options: {missing[:8]}")
    run(make + [f"-j{args.parallel}", "Image"])
    image = out / "arch/arm64/boot/Image"
    return image, {"source_sha256": args.kernel_source_sha256, "config_sha256": sha(out / ".config"),
                   "source_url": args.kernel_url, "source_commit": revision, "license": "GPL-2.0-only"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", required=True)
    parser.add_argument("--package-root", required=True, type=Path, help="Provisional ARM64 guest worker/core inventory root")
    parser.add_argument("--catalog", required=True, type=Path, help="Provisional native catalog")
    parser.add_argument("--dependencies", required=True, type=Path)
    parser.add_argument("--aarch64-toolchain-file", required=True, type=Path)
    parser.add_argument("--kernel-source", required=True, type=Path)
    parser.add_argument("--kernel-commit", required=True)
    parser.add_argument("--kernel-source-sha256", required=True)
    parser.add_argument("--kernel-url", required=True)
    parser.add_argument("--cross-compile", required=True, help="e.g. aarch64-linux-gnu-")
    parser.add_argument("--cmake", default="cmake", type=Path)
    parser.add_argument("--make", default="make")
    parser.add_argument("--git", default="git")
    parser.add_argument("--parallel", type=int, default=8)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if not HEX.fullmatch(args.kernel_source_sha256) or not args.kernel_url.startswith("https://") or not 1 <= args.parallel <= 32:
        raise ValueError("Kernel provenance arguments are invalid")
    output = args.output.resolve()
    if output.exists():
        raise ValueError("Use a fresh guest image output directory")
    work = ROOT / "out/macos-guest-build" / args.runtime
    if work.exists():
        raise ValueError("Use a fresh guest build directory")
    work.mkdir(parents=True)
    init_build = work / "init"
    run([args.cmake, "-S", ROOT / "engines/macos-guest", "-B", init_build, "-G", "Ninja",
         f"-DCMAKE_TOOLCHAIN_FILE={args.aarch64_toolchain_file.resolve(strict=True)}", "-DCMAKE_BUILD_TYPE=Release"])
    run([args.cmake, "--build", init_build, "--parallel", args.parallel])
    init = single(init_build, "vlikeon-guest-init")
    kernel, provenance = build_kernel(args, work)
    provenance_path = work / "kernel-provenance.json"
    provenance_path.write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    run(["python", ROOT / "tools/author_macos_guest.py", "--package-root", args.package_root, "--catalog", args.catalog,
         "--runtime", args.runtime, "--init", init, "--kernel", kernel, "--dependencies", args.dependencies,
         "--kernel-provenance", provenance_path, "--output", output])
    receipt = json.loads((output / "guest-receipt.json").read_text(encoding="utf-8"))
    (output / "build-recipe-receipt.json").write_text(json.dumps({
        "schema_version": 1, "runtime": args.runtime, "init_sha256": sha(init),
        "kernel_sha256": sha(output / "guest-kernel"), "initrd_sha256": sha(output / "guest-initrd.cpio"),
        "guest_receipt_sha256": sha(output / "guest-receipt.json"), "kernel_provenance": provenance,
        "recipe_sha256": sha(Path(__file__)), "verification": "not_run", "guest_protocol": receipt["guest_protocol"],
    }, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    print("Guest image authored from explicit pinned inputs. Boot and qualification remain pending.")


if __name__ == "__main__":
    main()
