#!/bin/sh
# Read-only host inventory. Run on the proposed Mac before task-5 provisioning.
set -eu
export LC_ALL=C
task_build_parent=${1:-"$PWD"}
if [ ! -d "$task_build_parent" ]; then
  printf '%s\n' 'Select an existing build parent directory' >&2
  exit 1
fi
printf '%s\n' '=== host ==='
uname -srm
sw_vers
sysctl -n hw.memsize
sysctl -n hw.logicalcpu
printf '%s\n' '=== build filesystem ==='
df -kP "$task_build_parent"
diskutil info "$task_build_parent" | sed -n '/File System Personality:/p; /Type (Bundle):/p; /Volume Free Space:/p; /Container Free Space:/p'
printf '%s\n' '=== developer tools ==='
if xcode-select -p; then
  xcodebuild -version || true
  xcrun --sdk macosx --show-sdk-version || true
  xcrun --sdk macosx --show-sdk-build-version || true
  xcrun --sdk macosx --show-sdk-path || true
  git --version || true
  python3 --version || true
else
  printf '%s\n' 'Developer directory is not configured; no installation was attempted'
fi
