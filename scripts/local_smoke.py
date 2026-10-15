import os
from pathlib import Path
import shutil
import subprocess
import sys


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[1]
    stage = root / '.build/local-smoke'
    stage.mkdir(parents=True, exist_ok=True)
    (stage / 'tools').mkdir(exist_ok=True)
    shutil.copy2(root / '.build/target/debug/CutFlow.exe', stage)
    shutil.copy2(root / '.build/target/debug/cutflow-agent.exe', stage / 'tools')
    source = Path(sys.argv[1]).resolve(strict=True)
    for name in ['ffmpeg.exe', 'ffprobe.exe']:
        shutil.copy2(source / name, stage / 'tools' / name)
    env = os.environ.copy()
    env['PYTHONDONTWRITEBYTECODE'] = '1'
    env['WEBVIEW2_BROWSER_EXECUTABLE_FOLDER'] = str(root / '.build/webview-runtime')
    for name in ['TEMP', 'TMP']:
        folder = root / '.build/tmp'
        folder.mkdir(exist_ok=True)
        env[name] = str(folder)
    subprocess.run([sys.executable, '-B', 'tests/check_windows_startup.py', '--exe', str(stage / 'CutFlow.exe'), '--state-dir', str(root / '.build/local-main-startup')], cwd=root, env=env, check=True)
    subprocess.run([sys.executable, '-B', 'tests/check_hub.py', '--stage', str(stage)], cwd=root, env=env, check=True)
