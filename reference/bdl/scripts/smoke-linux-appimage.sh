#!/usr/bin/env bash
set -euo pipefail

# Run under xvfb-run and dbus-run-session as an ordinary user.
if (( EUID == 0 )); then
  echo 'Run the AppImage smoke test as a non-root user.' >&2
  exit 1
fi
appimage="$(realpath "${1:?Usage: smoke-linux-appimage.sh path/to.AppImage}")"
test_dir="$(mktemp -d)"
app_pid=''
cleanup() {
  if [[ -n "$app_pid" ]]; then
    kill -- "-$app_pid" 2>/dev/null || true
    wait "$app_pid" 2>/dev/null || true
  fi
  rm -rf "$test_dir"
}
trap cleanup EXIT
# Isolate app settings and downloads without relying on FUSE availability.
mkdir -p "$test_dir/home" "$test_dir/config" "$test_dir/data" "$test_dir/cache"
setsid env HOME="$test_dir/home" XDG_CONFIG_HOME="$test_dir/config" \
  XDG_DATA_HOME="$test_dir/data" XDG_CACHE_HOME="$test_dir/cache" \
  "$appimage" --appimage-extract-and-run > "$test_dir/startup.log" 2>&1 &
app_pid=$!
for ((attempt=0; attempt<30; attempt++)); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    cat "$test_dir/startup.log" >&2
    echo 'AppImage exited before displaying its window.' >&2
    exit 1
  fi
  if xwininfo -root -tree | grep -q '"BDL"'; then
    sleep 3
    kill -0 "$app_pid"
    echo 'AppImage displayed its BDL window as an ordinary user.'
    exit 0
  fi
  sleep 1
done
cat "$test_dir/startup.log" >&2
echo 'AppImage did not display its BDL window within 30 seconds.' >&2
exit 1
