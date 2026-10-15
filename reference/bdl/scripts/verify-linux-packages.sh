#!/usr/bin/env bash
set -euo pipefail
bundle_dir="${1:?Usage: verify-linux-packages.sh path/to/bundle}"
shopt -s nullglob
deb_files=("$bundle_dir"/deb/*.deb)
appimages=("$bundle_dir"/appimage/*.AppImage)
if ((${#deb_files[@]} != 1 || ${#appimages[@]} != 1)); then
  echo 'Expected exactly one deb and one AppImage in a clean Linux build.' >&2
  exit 1
fi
[[ "$(dpkg-deb -f "${deb_files[0]}" Architecture)" == amd64 ]]
depends="$(dpkg-deb -f "${deb_files[0]}" Depends)"
[[ "$depends" == *ffmpeg* && "$depends" == *xdg-utils* && "$depends" == *libwebkit2gtk-4.1* ]]
contents="$(dpkg-deb --contents "${deb_files[0]}")"
[[ "$contents" == *usr/bin/bdl-desktop* && "$contents" == *share/applications/* ]]
[[ -s "${appimages[0]}" ]]
file "${appimages[0]}" | grep -q 'ELF 64-bit.*x86-64'
appimage="$(realpath "${appimages[0]}")"
extracted="$(mktemp -d)"
trap 'rm -rf "$extracted"' EXIT
# Inspect the finished image, not the AppDir that happened to produce it.
(cd "$extracted" && "$appimage" --appimage-extract > /dev/null)
for entry in AppRun AppRun.wrapped usr/bin/bdl-desktop; do
  file_path="$extracted/squashfs-root/$entry"
  [[ -f "$file_path" ]]
  mode="$(stat -Lc '%a' "$file_path")"
  # test -x alone passes for the build owner even with the broken 770 mode.
  if (( (8#$mode & 0005) != 0005 )); then
    echo "AppImage $entry is not readable/executable by other users (mode $mode)." >&2
    exit 1
  fi
done
echo 'Linux package architecture, dependencies, desktop entry and launcher permissions verified.'
