#!/usr/bin/env bash
set -euo pipefail

# Tauri 2.11.4 downloads AppRun with mode 770, which linuxdeploy preserves in
# AppRun.wrapped. SquashFS normalizes ownership, so unrelated users cannot run it.
# Prepare the same upstream launcher with public read/execute bits before bundling
# and signing, including when the tool already exists in the cache.
arch="${TAURI_ENV_ARCH:-$(uname -m)}"
case "$arch" in
  x86_64|aarch64|i686|armhf) ;;
  arm) arch=armhf ;;
  *) echo "Unsupported AppImage launcher architecture: $arch" >&2; exit 1 ;;
esac
tools_dir="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
mkdir -p "$tools_dir"
launcher="$tools_dir/AppRun-$arch"
if [[ ! -s "$launcher" ]]; then
  download="$(mktemp "$tools_dir/.AppRun-download.XXXXXX")"
  trap 'rm -f "$download"' EXIT
  curl --fail --location --retry 3 --connect-timeout 15 --max-time 120 \
    "https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-$arch" \
    --output "$download"
  file "$download" | grep -q ELF
  chmod 755 "$download"
  mv "$download" "$launcher"
fi
chmod 755 "$launcher"
echo "AppImage launcher ready: $(stat -c '%a %n' "$launcher")"
