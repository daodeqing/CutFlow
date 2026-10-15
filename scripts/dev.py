import argparse
import os
from pathlib import Path
import subprocess
import shutil
import sys


def environment():
    root = Path(__file__).resolve().parents[1]
    build = root / '.build'
    result = os.environ.copy()
    for name, folder in [('CARGO_HOME', 'cargo'), ('RUSTUP_HOME', 'rustup'),
                         ('TEMP', 'tmp'), ('TMP', 'tmp')]:
        target = build / folder
        target.mkdir(parents=True, exist_ok=True)
        result[name] = str(target)
    result['PYTHONUTF8'] = '1'
    result['PYTHONDONTWRITEBYTECODE'] = '1'
    result['CARGO_TARGET_DIR'] = str(build / 'target')
    proxies=build / 'cargo' / 'bin'
    proxies.mkdir(exist_ok=True)
    rustup=shutil.which('rustup.exe')
    if rustup:
        for name in ['rustup.exe','cargo.exe','rustc.exe','rustfmt.exe']:
            target=proxies / name
            if not target.exists():
                shutil.copy2(rustup,target)
        result['PATH']=str(proxies)+os.pathsep+result['PATH']
    native = build / 'native'
    libraries = [('FFMPEG_DIR', 'ffmpeg-win64', 'bin/ffmpeg.exe'),
                 ('ORT_DIR', 'onnxruntime-win-x64', 'lib/onnxruntime.lib'),
                 ('SHERPA_DLL_DIR', 'sherpa-onnx-win-x64-shared-MT-Release-lib', 'lib/sherpa-onnx-c-api.lib')]
    for name, folder, marker in libraries:
        candidates = list((native / folder).glob('*/' + marker))
        if candidates:
            result[name] = str(candidates[0].parents[1]) if name != 'SHERPA_DLL_DIR' else str(candidates[0].parent)
    if 'FFMPEG_DIR' in result:
        result['PATH'] = str(Path(result['FFMPEG_DIR']) / 'bin') + os.pathsep + result['PATH']
    if 'ORT_DIR' in result:
        result['ORT_LIB_LOCATION'] = str(Path(result['ORT_DIR']) / 'lib')
        result['ORT_PREFER_DYNAMIC_LINK'] = '1'
    if 'SHERPA_DLL_DIR' in result:
        result['SHERPA_ONNX_LIB_DIR'] = str(native / 'sherpa-imports')
    clang = build / 'python' / 'clang' / 'native'
    if (clang / 'libclang.dll').exists():
        result['LIBCLANG_PATH'] = str(clang)
    for name in ['GGML_NATIVE', 'GGML_AVX2', 'GGML_FMA', 'GGML_F16C', 'GGML_BMI2', 'GGML_AVX512']:
        result[name] = 'OFF'
    locator = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    if locator.is_file():
        installation = subprocess.check_output([str(locator), '-latest', '-products', '*',
                                                '-property', 'installationPath'], text=True, encoding='utf-8').strip()
        setup = Path(installation) / 'VC/Auxiliary/Build/vcvars64.bat'
        if setup.is_file():
            command = f'call "{setup}" >nul && set'
            output = subprocess.check_output('cmd.exe /d /s /c "' + command + '"', env=result)
            for line in output.decode('mbcs', errors='replace').splitlines():
                name, separator, value = line.partition('=')
                if separator and name:
                    result[name] = value
    return root, result


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--cwd', default='.')
    parser.add_argument('command', nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    root, process_environment = environment()
    command = arguments.command
    if command and command[0] == '--':
        command = command[1:]
    if not command:
        parser.error('Provide a command after --')
    sys.exit(subprocess.run(command, cwd=root / arguments.cwd,
                            env=process_environment).returncode)
