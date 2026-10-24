#!/bin/sh
# Read-only prerequisite check for the pinned Chromium Apple module-map inputs.
# This does not qualify GN, compilation, runtime, signing or distribution.
set -eu
export LC_ALL=C
if [ "$#" -ne 1 ] || [ ! -d "$1" ]; then
  printf '%s\n' 'Usage: sdk-modulemap-preflight.sh /absolute/path/to/MacOSX.sdk' >&2
  exit 2
fi
case "$1" in
  /*) ;;
  *) printf '%s\n' 'An absolute SDK path is required' >&2; exit 2 ;;
esac
task_sdk_root=$1
task_missing=0
for task_modulemap in DarwinFoundation1.modulemap DarwinFoundation2.modulemap DarwinFoundation3.modulemap libunwind.modulemap; do
  if [ ! -f "$task_sdk_root/usr/include/$task_modulemap" ] || [ ! -r "$task_sdk_root/usr/include/$task_modulemap" ]; then
    printf 'MISSING %s\n' "$task_modulemap" >&2
    task_missing=1
  else
    printf 'PRESENT %s\n' "$task_modulemap"
  fi
done
if [ "$task_missing" -ne 0 ]; then
  printf '%s\n' 'SDK module-map prerequisites are incomplete; do not start this pinned Chromium build.' >&2
  exit 1
fi
printf '%s\n' 'SDK module-map prerequisites present; GN and full build remain unverified.'
