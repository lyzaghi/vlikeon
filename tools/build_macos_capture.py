"""Build only the ScreenCaptureKit adapter library using an existing Mac SDK.

No Linux build, dependency install, signing, permission prompt, capture or test.
The resulting library is not admitted into the product by this recipe.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "engines/capture-macos"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cmake", required=True, type=Path)
    parser.add_argument("--architecture", choices=("arm64", "x86_64"), required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("A real Mac with an existing Xcode macOS SDK is required")
    if not args.cmake.is_absolute() or not args.cmake.is_file():
        parser.error("--cmake must name an existing absolute executable")
    output = ROOT / "out/macos-capture" / args.architecture
    if output.exists() or output.is_symlink():
        parser.error("Use a fresh capture build output; existing results are not overwritten")
    sdk = Path(subprocess.check_output(
        ["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"],
        encoding="utf-8", timeout=15).strip())
    if not sdk.is_absolute() or not sdk.is_dir():
        raise ValueError("xcrun did not resolve an existing macOS SDK")
    # Source byte hashes are recorded before build; no downloaded headers/stubs.
    files = sorted(path for path in SOURCE.rglob("*") if path.is_file())
    if any(path.is_symlink() or not path.resolve().is_relative_to(SOURCE.resolve()) for path in files):
        raise ValueError("Capture source must be ordinary files in its source tree")
    hashes = {path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
              for path in files}
    output.mkdir(parents=True)
    build = output / "build"
    configure = [str(args.cmake), "-S", str(SOURCE), "-B", str(build),
                 "-DCMAKE_BUILD_TYPE=Release", "-DCMAKE_OSX_DEPLOYMENT_TARGET=13.0",
                 "-DCMAKE_OSX_SYSROOT=" + str(sdk),
                 "-DCMAKE_OSX_ARCHITECTURES=" + args.architecture]
    compile_command = [str(args.cmake), "--build", str(build), "--config", "Release", "--parallel", "2"]
    subprocess.run(configure, check=True, timeout=120)
    subprocess.run(compile_command, check=True, timeout=300)
    for name, expected in hashes.items():
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != expected:
            raise ValueError("Source changed during build: " + name)
    candidates = [path for path in build.rglob("libvlikeon-macos-capture.a") if path.is_file()]
    if len(candidates) != 1 or candidates[0].is_symlink() or not candidates[0].resolve().is_relative_to(build.resolve()):
        raise ValueError("Expected exactly one ordinary adapter static library")
    library = candidates[0]
    if not 0 < library.stat().st_size <= 100 * 1024**2:
        raise ValueError("Adapter library is outside its explicit size bound")
    receipt = {"schema_version": 1, "item": 54, "architecture": args.architecture,
               "deployment_target": "13.0", "sdk": str(sdk), "source_sha256": hashes,
               "configure": configure, "build": compile_command,
               "library": library.relative_to(ROOT).as_posix(),
               "library_bytes": library.stat().st_size,
               "library_sha256": hashlib.sha256(library.read_bytes()).hexdigest(),
               "capture_executed": False, "signing_executed": False,
               "product_integrated": False, "runtime_verification": "not_run"}
    (output / "build-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    print("Built the macOS capture library. Product integration and runtime qualification remain open.")


if __name__ == "__main__":
    main()
