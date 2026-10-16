import argparse
import ctypes
import os
from pathlib import Path
import subprocess
import sys
import time
from ctypes import wintypes
import psutil


def capture_webview_children(process):
    try:
        return [child for child in psutil.Process(process.pid).children(recursive=True)
                if child.name().lower() == "msedgewebview2.exe"]
    except psutil.Error:
        return []


def close_webview_children(children):
    # Only processes created by this exact test application are eligible.
    _, alive = psutil.wait_procs(children, timeout=3)
    for child in alive:
        try:
            child.terminate()
        except psutil.Error:
            pass
    _, alive = psutil.wait_procs(alive, timeout=5)
    for child in alive:
        try:
            child.kill()
        except psutil.Error:
            pass
    psutil.wait_procs(alive, timeout=5)


def check_startup(executable, state_dir, timeout_seconds, settle_seconds):
    state_dir.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    for variable in ('APPDATA', 'LOCALAPPDATA', 'TEMP', 'TMP'):
        folder = state_dir / variable.lower()
        folder.mkdir(exist_ok=True)
        environment[variable] = str(folder)
    user32 = ctypes.WinDLL('user32', use_last_error=True)
    user32.IsWindowVisible.argtypes = [wintypes.HWND]
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.IsHungAppWindow.argtypes = [wintypes.HWND]
    user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    log_path = state_dir / 'gui-startup.log'
    with log_path.open('wb') as log:
        process = subprocess.Popen([str(executable)], cwd=executable.parent, env=environment,
                                   stdout=log, stderr=log, creationflags=subprocess.CREATE_NO_WINDOW)
        windows = []
        webviews = []

        @callback_type
        def collect_window(window, _parameter):
            owner = wintypes.DWORD()
            user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
            if owner.value == process.pid and user32.IsWindowVisible(window):
                windows.append(window)
            return True

        try:
            deadline = time.monotonic() + timeout_seconds
            settle_deadline = None
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(f'GUI exited during startup: 0x{process.returncode & 0xffffffff:08X}')
                windows.clear()
                user32.EnumWindows(collect_window, 0)
                if windows:
                    if settle_deadline is None:
                        settle_deadline = time.monotonic() + settle_seconds
                    elif time.monotonic() >= settle_deadline:
                        break
                time.sleep(0.2)
            if not windows:
                raise RuntimeError(f'No visible application window after {timeout_seconds:g}s.')
            if any(user32.IsHungAppWindow(window) for window in windows):
                raise RuntimeError('Application window is not responding.')
            webviews = capture_webview_children(process)
            for window in windows:
                title = ctypes.create_unicode_buffer(256)
                user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
                user32.GetWindowTextW(window, title, len(title))
                posted = user32.PostMessageW(window, 0x0010, 0, 0)
                print(f'Window: {title.value!r}; close posted={posted}; last_error={ctypes.get_last_error()}')
            forced_shutdown = False
            try:
                exit_code = process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                forced_shutdown = True
                process.terminate()
                process.wait(timeout=10)
                exit_code = 0
            if exit_code != 0:
                raise RuntimeError(f'GUI shutdown failed: 0x{exit_code & 0xffffffff:08X}')
        finally:
            if not webviews:
                webviews = capture_webview_children(process)
            if process.poll() is None:
                process.kill()
                process.wait(timeout=10)
            close_webview_children(webviews)
    print(f'PASS: window visible and responsive; log: {log_path}')
    if forced_shutdown:
        print('NOTE: background process required termination after the close request.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--state-dir', type=Path, required=True)
    parser.add_argument('--seconds', type=float, default=25)
    parser.add_argument('--settle', type=float, default=3)
    arguments = parser.parse_args()
    try:
        check_startup(arguments.exe.resolve(), arguments.state_dir.resolve(), arguments.seconds, arguments.settle)
    except (OSError, RuntimeError) as error:
        print(f'FAIL: {error}', file=sys.stderr)
        sys.exit(1)
