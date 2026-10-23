"""Explicit later Windows video helper build/staging recipe. Do not run while
the owner defers build/test. Never launches media/helper binaries or signs/releases.
"""
import argparse
import hashlib
import json
import platform
from pathlib import Path
import re
import shutil
import subprocess
from windows_video_artifact import ENTRY, admit_video_artifact

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'engines/encode-windows'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_inventory():
    files = [SOURCE / 'CMakeLists.txt', *sorted((SOURCE / 'include').glob('*.hpp')), *sorted((SOURCE / 'tests').glob('*.cpp')),
             *sorted((SOURCE / 'src').glob('*.cpp')),
             ROOT / 'tools/build_stream_video.py', ROOT / 'tools/windows_video_artifact.py']
    result = {}
    for path in files:
        if not path.is_file() or path.is_symlink() or path.stat().st_size > 1024 * 1024:
            raise ValueError('The explicit video source inventory is missing, linked or oversized')
        result[path.relative_to(ROOT).as_posix()] = digest(path)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cmake', required=True, type=Path)
    parser.add_argument('--generator', required=True, choices=['Visual Studio 17 2022', 'Visual Studio 18 2026'])
    parser.add_argument('--architecture', required=True, choices=['x64', 'ARM64'])
    parser.add_argument('--sdk-version', required=True)
    parser.add_argument('--parallel', type=int, default=2)
    args = parser.parse_args()
    if platform.system() != 'Windows' or not args.cmake.is_absolute() or not args.cmake.is_file():
        raise ValueError('An existing explicit Windows CMake toolchain is required')
    if not re.fullmatch(r'10\.0\.[0-9]{4,6}\.0', args.sdk_version) or not 1 <= args.parallel <= 8:
        raise ValueError('Choose an explicit SDK and finite build parallelism')
    output = ROOT / 'out/stream-video' / args.architecture.lower()
    if output.exists():
        raise ValueError('Use a fresh video staging output; this recipe never resets one')
    before = source_inventory()
    output.mkdir(parents=True)
    # Snapshot reviewed sources: CMake never reads a moving working tree during
    # compilation, and no download/FetchContent/install action is supplied.
    snapshot = output / 'source'
    snapshot.mkdir()
    for relative in before:
        path = Path(relative)
        if path.parts[:2] != ('engines', 'encode-windows'):
            continue
        destination = snapshot / path.relative_to('engines/encode-windows')
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / path, destination)
        if digest(destination) != before[relative]:
            raise ValueError('Video source changed during snapshot creation')
    configure = [str(args.cmake), '-S', str(snapshot), '-B', str(output / 'build'),
                 '-G', args.generator, '-A', args.architecture,
                 '-DCMAKE_SYSTEM_VERSION=' + args.sdk_version,
                 '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',
                 '-DFETCHCONTENT_FULLY_DISCONNECTED=ON']
    build = [str(args.cmake), '--build', str(output / 'build'), '--config', 'Release',
             '--target', 'omniplay-video-worker', '--parallel', str(args.parallel)]
    subprocess.run(configure, cwd=ROOT, check=True)
    subprocess.run(build, cwd=ROOT, check=True)
    binary = output / 'build/Release/omniplay-video-worker.exe'
    target = 'windows-x64' if args.architecture == 'x64' else 'windows-arm64'
    artifact = admit_video_artifact(binary, target)
    staged = output / 'package' / ENTRY
    staged.parent.mkdir(parents=True)
    shutil.copy2(binary, staged)
    staged_record = admit_video_artifact(staged, target)
    if staged_record != artifact or source_inventory() != before:
        raise ValueError('Video source or binary changed while staging')
    receipt = {'schema_version': 1, 'source_sha256': before,
               'cmake_sha256': digest(args.cmake), 'generator': args.generator,
               'architecture': args.architecture, 'sdk': args.sdk_version,
               'commands': [configure, build], 'artifact': artifact,
               'private_protocols': [1, 2, 3], 'helper_executed': False, 'tests_executed': False,
               'release_requires': 'installed_inventory_provenance_signature_and_full_platform_media_qualification'}
    (output / 'build-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    (output / 'executables.json').write_text(json.dumps([ENTRY], indent=2) + '\n', encoding='utf-8')
    print('Built/staged video helper; not executed, signed, released or runtime-qualified.')


if __name__ == '__main__':
    main()
