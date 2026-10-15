import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile

from dev import environment
from prepare_native import prepare
from recycle import recycle


def build(installer, skip_smoke):
    initial_root, initial_environment = environment()
    for name in ['TEMP', 'TMP', 'CARGO_HOME', 'RUSTUP_HOME', 'PYTHONDONTWRITEBYTECODE']:
        os.environ[name] = initial_environment[name]
    prepare()
    root, environment_variables = environment()
    target = Path(environment_variables['CARGO_TARGET_DIR'])
    def run(arguments, cwd=root):
        print('Running', ' '.join(str(argument) for argument in arguments), flush=True)
        logs=root / '.build/logs'
        logs.mkdir(exist_ok=True)
        log=logs / f'build-{len(list(logs.glob("build-*.log"))) + 1:03d}.log'
        with log.open('wb') as output:
            process=subprocess.Popen([str(argument) for argument in arguments], cwd=cwd,
                                     env=environment_variables, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                     creationflags=subprocess.CREATE_NO_WINDOW)
            for line in process.stdout:
                output.write(line)
                print(line.decode('utf-8',errors='replace'),end='',flush=True)
            process.wait()
        if process.returncode:
            print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-70:]),flush=True)
            raise RuntimeError(f'Build step failed; see {log}')
    def cargo(arguments, cwd=root):
        run(['cargo', '+1.93.1', *arguments], cwd)
    if not (Path(environment_variables['RUSTUP_HOME']) / 'toolchains/1.93.1-x86_64-pc-windows-msvc').is_dir():
        run(['rustup', 'toolchain', 'install', '1.93.1', '--profile', 'minimal', '--no-self-update'])
    for manifest in ['downloader', 'workbench']:
        cargo(['test', '--locked', '--manifest-path', f'{manifest}/Cargo.toml'])
    for manifest in ['tests/ui-compile', 'tests/native-ui']:
        cargo(['check', '--locked', '--manifest-path', f'{manifest}/Cargo.toml'])
    run([sys.executable, '-B', 'tests/check_callback_wiring.py'])
    cargo(['test', '--locked', '-p', 'concat-project'], root / 'editor/src')
    cargo(['build', '--locked', '--release', '--manifest-path', 'workbench/Cargo.toml', '--bin', 'cutflow-agent'])
    cargo(['build', '--locked', '--release', '-p', 'bdl-cli'], root / 'reference/bdl')
    cargo(['build', '--locked', '--profile', 'app', '-p', 'concat',
           '--config', 'profile.app.lto="thin"',
           '--config', 'profile.app.package.concat.opt-level=1',
           '--config', 'profile.app.package.concat.codegen-units=16'], root / 'editor/src')
    stage = root / 'dist/CutFlow-0.3.0-windows-x64'
    if stage.exists():
        raise RuntimeError('Stage already exists; recycle or rename the old bundle first: ' + str(stage))
    tools = stage / 'tools'
    licenses = stage / 'licenses'
    for folder in [tools, licenses, stage / 'portable', stage / 'examples']:
        folder.mkdir(parents=True, exist_ok=True)
    shutil.copy2(target / 'app/concat.exe', stage / 'CutFlow.exe')
    shutil.copy2(target / 'release/bdl.exe', tools)
    shutil.copy2(target / 'release/cutflow-agent.exe', tools)
    ffmpeg = Path(environment_variables['FFMPEG_DIR'])
    for executable in ['ffmpeg.exe', 'ffprobe.exe']:
        shutil.copy2(ffmpeg / 'bin' / executable, tools)
    for library in list((ffmpeg / 'bin').glob('*.dll')) + list(Path(environment_variables['SHERPA_DLL_DIR']).glob('*.dll')) + list((Path(environment_variables['ORT_DIR']) / 'lib').glob('*.dll')):
        shutil.copy2(library, stage)
        shutil.copy2(library, tools)
    locator = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    installation = Path(subprocess.check_output([str(locator), '-latest', '-products', '*', '-property', 'installationPath'], text=True).strip())
    runtimes = sorted(installation.glob('VC/Redist/MSVC/*/x64/Microsoft.VC14*.CRT'))
    if not runtimes:
        raise RuntimeError('Visual C++ runtime not found')
    for library in runtimes[-1].glob('*.dll'):
        shutil.copy2(library, stage)
        shutil.copy2(library, tools)
    for path in ['editor/LICENSE', 'editor/LICENSE-EXCEPTIONS.md', 'editor/THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_NOTICES.md', 'upstreams.lock.json', 'reference/bdl/LICENSE']:
        source = root / path
        shutil.copy2(source, licenses / (path.replace('/', '-') if source.name in ['LICENSE', 'THIRD_PARTY_NOTICES.md'] else source.name))
    for source in ffmpeg.rglob('*'):
        if source.is_file() and any(word in source.name.upper() for word in ['LICENSE', 'COPYING']):
            shutil.copy2(source, licenses / ('FFmpeg-' + source.name))
    for name in ['onnxruntime-win-x64','sherpa-onnx-win-x64-shared-MT-Release-lib']:
        for source in (root / '.build/native' / name).rglob('*'):
            if source.is_file() and any(word in source.name.upper() for word in ['LICENSE','NOTICE','COPYING']):
                shutil.copy2(source,licenses / (name + '-' + source.name))
    for source, destination in [('README.md', '使用说明.md'), ('docs/创作与外部AI.md', '创作与外部AI.md'),
                                ('docs/验收与已知限制.md', '验收与已知限制.md'), ('docs/SOURCE_OFFER.md', 'SOURCE_OFFER.md'),
                                ('scripts/启动CutFlow.vbs', '启动CutFlow.vbs'), ('examples/draft.json', 'examples/draft.json')]:
        shutil.copy2(root / source, stage / destination)
    if not skip_smoke:
        clean = environment_variables.copy()
        clean['PATH'] = os.environ.get('SystemRoot', 'C:/Windows') + '/System32'
        for executable, arguments in [(stage / 'CutFlow.exe', ['--version']), (stage / 'CutFlow.exe', ['--smoke']),
                                      (tools / 'bdl.exe', ['--version']), (tools / 'ffmpeg.exe', ['-version']),
                                      (tools / 'ffprobe.exe', ['-version']), (tools / 'cutflow-agent.exe', ['schema'])]:
            result = subprocess.run([str(executable), *arguments], env=clean, capture_output=True, timeout=120,
                                    creationflags=subprocess.CREATE_NO_WINDOW)
            if result.returncode:
                raise RuntimeError(f'Smoke failed: {executable.name}: {result.stderr.decode(errors="replace")}')
            if executable.name == 'cutflow-agent.exe':
                json.loads(result.stdout)
        run([sys.executable, '-B', 'tests/check_windows_startup.py', '--exe', stage / 'CutFlow.exe', '--state-dir', root / '.build/startup-check'])
        recycle(stage / 'portable')
        (stage / 'portable').mkdir()
    archive = Path(str(stage) + '.zip')
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as package:
        for source in sorted(stage.rglob('*')):
            package.write(source, source.relative_to(stage.parent))
    digest = hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest()
    Path(str(archive) + '.sha256').write_text(digest + '  ' + archive.name + '\n', encoding='utf-8')
    if installer:
        compiler = shutil.which('ISCC.exe') or str(root / '.build/inno/tools/ISCC.exe')
        if not Path(compiler).is_file():
            compiler = str(Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Inno Setup 6/ISCC.exe')
        run([compiler, f'/DStage={stage}', f'/DOutput={stage.parent}', root / 'scripts/CutFlow.iss'])
    print('Built', archive, flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--installer', action='store_true')
    parser.add_argument('--skip-smoke', action='store_true')
    options = parser.parse_args()
    build(options.installer, options.skip_smoke)
