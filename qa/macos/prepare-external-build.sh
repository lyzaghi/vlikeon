#!/bin/bash
# Prepare task-owned storage only. Does not erase disks, install tools or build.
set -euo pipefail
export LC_ALL=C

fail() {
  printf 'Dung: %s\n' "$*" >&2
  exit 1
}

[ "$(uname -s)" = Darwin ] || fail 'Lenh nay chi danh cho macOS.'
if [ "$#" -gt 1 ]; then
  fail 'Chi truyen mot duong dan o ngoai.'
elif [ "$#" -eq 1 ]; then
  task_external_input=$1
else
  printf '\nCac o da gan:\n'
  /bin/ls -1 /Volumes
  printf '\nNhap duong dan o 4 TB, vi du /Volumes/My Passport (khong them dau nhay): '
  IFS= read -r task_external_input < /dev/tty
fi
[ -d "$task_external_input" ] || fail 'Khong tim thay o da gan.'
task_external=$(cd "$task_external_input" && pwd -P)
case "$task_external" in
  /Volumes/*) ;;
  *) fail 'Hay chon o ngoai da gan trong /Volumes.' ;;
esac
task_disk_info=$(/usr/sbin/diskutil info -plist "$task_external")
task_mount=$(printf '%s\n' "$task_disk_info" | /usr/bin/plutil -extract MountPoint raw -o - -)
task_internal=$(printf '%s\n' "$task_disk_info" | /usr/bin/plutil -extract Internal raw -o - -)
[ "$task_mount" = "$task_external" ] || fail 'Chon thu muc goc cua o, khong chon thu muc con.'
[ "$task_internal" = false ] || fail 'O duoc chon khong duoc xac nhan la o ngoai.'
[ -w "$task_external" ] || fail 'O ngoai chua co quyen ghi.'
task_free_kb=$(/bin/df -kP "$task_external" | /usr/bin/awk 'NR == 2 {print $4}')
case "$task_free_kb" in ''|*[!0-9]*) fail 'Khong doc duoc dung luong trong.' ;; esac
[ "$task_free_kb" -ge 244140625 ] || fail 'Can it nhat 250 GB trong tren o ngoai.'

task_storage="$task_external/VlikeonBuildHost"
task_mountpoint=/Volumes/VlikeonBuild
[ ! -e "$task_storage" ] && [ ! -L "$task_storage" ] || fail 'VlikeonBuildHost da ton tai; khong ghi de.'
[ ! -e "$task_mountpoint" ] && [ ! -L "$task_mountpoint" ] || fail '/Volumes/VlikeonBuild da ton tai; khong gan chong.'
umask 077
/bin/mkdir "$task_storage"
/bin/mkdir "$task_storage/tmp"
export TMPDIR="$task_storage/tmp/"
export TMP="$TMPDIR" TEMP="$TMPDIR"
task_image="$task_storage/VlikeonBuild.sparsebundle"
printf 'Tao anh dia APFS toi da 350 GiB tren: %s\n' "$task_external"
/usr/bin/hdiutil create -size 350g -type SPARSEBUNDLE -fs APFS -volname VlikeonBuild "$task_image"
/usr/bin/hdiutil attach -nobrowse "$task_image"
[ -d "$task_mountpoint" ] || fail 'Khong tim thay volume APFS vua tao.'
task_fs=$(/usr/sbin/diskutil info -plist "$task_mountpoint" | /usr/bin/plutil -extract FilesystemType raw -o - -)
[ "$task_fs" = apfs ] || fail 'Volume build khong phai APFS.'

task_root="$task_mountpoint/work"
/bin/mkdir -p "$task_root/source" "$task_root/projects" "$task_root/cache" "$task_root/tmp" "$task_root/logs"
{
  printf 'export VLIKE_BUILD_ROOT=%q\n' "$task_root"
  printf 'export VLIKE_BACKING_IMAGE=%q\n' "$task_image"
  cat <<'ENV'
test -d "$VLIKE_BUILD_ROOT" || { printf '%s\n' 'Chua gan volume VlikeonBuild' >&2; return 1; }
export TMPDIR="$VLIKE_BUILD_ROOT/tmp/"
export TMP="$TMPDIR" TEMP="$TMPDIR"
export XDG_CACHE_HOME="$VLIKE_BUILD_ROOT/cache"
export CIPD_CACHE_DIR="$VLIKE_BUILD_ROOT/cache/cipd"
export VPYTHON_VIRTUALENV_ROOT="$VLIKE_BUILD_ROOT/cache/vpython"
export PIP_CACHE_DIR="$VLIKE_BUILD_ROOT/cache/pip"
export UV_CACHE_DIR="$VLIKE_BUILD_ROOT/cache/uv"
export UV_PYTHON_INSTALL_DIR="$VLIKE_BUILD_ROOT/cache/uv-python"
export npm_config_cache="$VLIKE_BUILD_ROOT/cache/npm"
export CARGO_HOME="$VLIKE_BUILD_ROOT/cache/cargo"
export RUSTUP_HOME="$VLIKE_BUILD_ROOT/cache/rustup"
export GOCACHE="$VLIKE_BUILD_ROOT/cache/go-build"
export GOMODCACHE="$VLIKE_BUILD_ROOT/cache/go-mod"
export CCACHE_DIR="$VLIKE_BUILD_ROOT/cache/ccache"
export SCCACHE_DIR="$VLIKE_BUILD_ROOT/cache/sccache"
export CLANG_MODULE_CACHE_PATH="$VLIKE_BUILD_ROOT/cache/clang"
export PYTHONDONTWRITEBYTECODE=1
export DEPOT_TOOLS_UPDATE=0
export PATH="$VLIKE_BUILD_ROOT/depot_tools:$PATH"
ENV
} > "$task_root/env.sh"
printf '%s\n' 'Vlikeon task-5 external build workspace; no existing disk data erased.' > "$task_storage/OWNER.txt"
{
  printf '#!/bin/bash\nset -eu\n'
  printf 'exec /usr/bin/hdiutil attach -nobrowse %q\n' "$task_image"
} > "$task_storage/mount-vlikeon.sh"
printf '\nDa chuan bi. Chay trong Terminal hien tai:\nsource %q\ncd %q\n' "$task_root/env.sh" "$task_root"
printf '\nSau khi khoi dong lai Mac, gan lai bang:\nbash %q\n' "$task_storage/mount-vlikeon.sh"
printf '\nBuild/cache o o ngoai; macOS, Xcode va swap he thong khong duoc di chuyen.\n'
/bin/df -h "$task_external" "$task_mountpoint"
