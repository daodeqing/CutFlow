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
from prepare_webview import prepare as prepare_webview
from recycle import recycle

VERSION = json.loads((Path(__file__).resolve().parents[1] / 'reference/bdl/apps/desktop/src-tauri/tauri.conf.json').read_text(encoding='utf-8'))['version']


def build(installer, skip_smoke, component):
    root, variables = environment()
    for name in ['TEMP', 'TMP', 'CARGO_HOME', 'RUSTUP_HOME', 'PYTHONDONTWRITEBYTECODE']:
        os.environ[name] = variables[name]
    target = Path(variables['CARGO_TARGET_DIR'])
    logs = root / '.build/logs'
    logs.mkdir(exist_ok=True)

    def run(arguments, cwd=root, env=None):
        print('Running', ' '.join(map(str, arguments)), flush=True)
        log = logs / f'build-{len(list(logs.glob("build-*.log"))) + 1:03d}.log'
        with log.open('wb') as output:
            process = subprocess.Popen(list(map(str, arguments)), cwd=cwd, env=env or variables,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                       creationflags=subprocess.CREATE_NO_WINDOW)
            for line in process.stdout:
                output.write(line)
                print(line.decode('utf-8', errors='replace'), end='', flush=True)
            process.wait()
        if process.returncode:
            raise RuntimeError(f'Build step failed: {log}')

    def cargo(arguments, folder='.'):
        run(['cargo', '+1.93.1', *arguments], root / folder)

    def copy(source, destination):
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    def fresh(folder):
        if folder.exists():
            raise RuntimeError('Build stage exists; recycle it before rebuilding: ' + str(folder))
        folder.mkdir(parents=True)

    if component in ['hub', 'editor', 'all'] and not (Path(variables['RUSTUP_HOME']) / 'toolchains/1.93.1-x86_64-pc-windows-msvc').is_dir():
        run(['rustup', 'toolchain', 'install', '1.93.1', '--profile', 'minimal', '--no-self-update'])
    artifacts = root / '.build/artifacts'
    if component in ['hub', 'all']:
        run([sys.executable, '-B', '-X', 'utf8', 'scripts/frontend.py', 'install', '--frozen-lockfile', '--reporter', 'append-only'])
        run([sys.executable, '-B', '-X', 'utf8', 'scripts/frontend.py', 'test'])
        run([sys.executable, '-B', '-X', 'utf8', 'scripts/frontend.py', 'build'])
        cargo(['test', '--locked', '-p', 'bdl-core'], 'reference/bdl')
        cargo(['test', '--locked', '-p', 'bdl-tauri', '--lib'], 'reference/bdl')
        cargo(['test', '--locked', '-p', 'cutflow-desktop', '--lib', 'editor_launch_tests'], 'reference/bdl')
        cargo(['build', '--locked', '--release', '-p', 'cutflow-desktop', '--bin', 'CutFlow'], 'reference/bdl')
        hub = artifacts / 'hub'
        fresh(hub)
        copy(target / 'release/CutFlow.exe', hub / 'CutFlow.exe')
        prepare_webview(hub / 'WebView2')
        (hub / 'portable').mkdir()
        if not skip_smoke:
            run([sys.executable, '-B', 'tests/check_windows_startup.py', '--exe', hub / 'CutFlow.exe', '--state-dir', root / '.build/hub-startup'])
            recycle(hub / 'portable')
            (hub / 'portable').mkdir()
    if component in ['editor', 'all']:
        prepare()
        root, variables = environment()
        for manifest in ['downloader', 'workbench']:
            cargo(['test', '--locked', '--manifest-path', f'{manifest}/Cargo.toml'])
        for manifest in ['tests/ui-compile', 'tests/native-ui']:
            cargo(['check', '--locked', '--manifest-path', f'{manifest}/Cargo.toml'])
        run([sys.executable, '-B', 'tests/check_callback_wiring.py'])
        cargo(['test', '--locked', '-p', 'concat-project'], 'editor/src')
        cargo(['build', '--locked', '--release', '--manifest-path', 'workbench/Cargo.toml', '--bin', 'cutflow-agent'])
        cargo(['build', '--locked', '--release', '-p', 'bdl-cli'], 'reference/bdl')
        cargo(['build', '--locked', '--profile', 'app', '-p', 'concat', '--config', 'profile.app.lto=false',
               '--config', 'profile.app.package.concat.opt-level=1', '--config', 'profile.app.package.concat.codegen-units=16'], 'editor/src')
        editor = artifacts / 'editor'
        fresh(editor)
        tools = editor / 'tools'
        copy(target / 'app/concat.exe', editor / 'CutFlow-editor.exe')
        copy(target / 'app/concat.exe', tools / 'concat.exe')
        copy(target / 'release/bdl.exe', tools / 'bdl.exe')
        copy(target / 'release/cutflow-agent.exe', tools / 'cutflow-agent.exe')
        ffmpeg = Path(variables['FFMPEG_DIR'])
        for name in ['ffmpeg.exe', 'ffprobe.exe']:
            copy(ffmpeg / 'bin' / name, tools / name)
        libraries = list((ffmpeg / 'bin').glob('*.dll')) + list(Path(variables['SHERPA_DLL_DIR']).glob('*.dll')) + list((Path(variables['ORT_DIR']) / 'lib').glob('*.dll'))
        for library in libraries:
            copy(library, editor / library.name)
            copy(library, tools / library.name)
        locator = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
        installation = Path(subprocess.check_output([str(locator), '-latest', '-products', '*', '-property', 'installationPath'], text=True).strip())
        runtimes = sorted(installation.glob('VC/Redist/MSVC/*/x64/Microsoft.VC14*.CRT'))
        if not runtimes:
            raise RuntimeError('Visual C++ runtime not found')
        for library in runtimes[-1].glob('*.dll'):
            copy(library, editor / library.name)
            copy(library, tools / library.name)
        licenses = editor / 'licenses'
        for path in ['editor/LICENSE', 'editor/LICENSE-EXCEPTIONS.md', 'editor/THIRD_PARTY_NOTICES.md', 'reference/bdl/LICENSE', 'THIRD_PARTY_NOTICES.md', 'upstreams.lock.json', 'docs/bdl-source-snapshot.json']:
            copy(root / path, licenses / path.replace('/', '-'))
        for folder in [ffmpeg, Path(variables['ORT_DIR']), Path(variables['SHERPA_DLL_DIR']).parent]:
            for source in folder.rglob('*'):
                if source.is_file() and any(word in source.name.upper() for word in ['LICENSE', 'NOTICE', 'COPYING']):
                    copy(source, licenses / (folder.name + '-' + source.name))
    if component not in ['package', 'all']:
        return
    stage = root / f'dist/CutFlow-{VERSION}-windows-x64'
    fresh(stage)
    for name in ['hub', 'editor']:
        source = artifacts / name
        if not source.is_dir():
            raise RuntimeError('Missing build component: ' + name)
        shutil.copytree(source, stage, dirs_exist_ok=True)
    for source, destination in [('README.md', '使用说明.md'), ('docs/创作与外部AI.md', '创作与外部AI.md'),
                                ('docs/验收与已知限制.md', '验收与已知限制.md'), ('docs/SOURCE_OFFER.md', 'SOURCE_OFFER.md'),
                                ('scripts/启动CutFlow.vbs', '启动CutFlow.vbs'), ('examples/draft.json', 'examples/draft.json')]:
        copy(root / source, stage / destination)
    copy(root / 'reference/bdl/apps/desktop/src-tauri/icons/icon.ico', stage / 'CutFlow.ico')
    (stage / 'portable').mkdir(exist_ok=True)
    if not (stage / 'WebView2/msedgewebview2.exe').is_file():
        raise RuntimeError('Fixed WebView2 runtime missing')
    if not skip_smoke:
        clean = variables.copy()
        clean['PATH'] = os.environ.get('SystemRoot', 'C:/Windows') + '/System32'
        clean.pop('CUTFLOW_RENDERER', None)
        for executable, arguments in [(stage / 'CutFlow.exe', ['--version']), (stage / 'CutFlow-editor.exe', ['--version']),
                                      (stage / 'CutFlow-editor.exe', ['--smoke']), (stage / 'tools/bdl.exe', ['--version']),
                                      (stage / 'tools/ffmpeg.exe', ['-version']), (stage / 'tools/ffprobe.exe', ['-version']),
                                      (stage / 'tools/cutflow-agent.exe', ['schema'])]:
            run([executable, *arguments], env=clean)
        for executable in ['CutFlow.exe', 'CutFlow-editor.exe']:
            run([sys.executable, '-B', 'tests/check_windows_startup.py', '--exe', stage / executable, '--state-dir', root / '.build' / (executable + '-startup')], env=clean)
        software = clean.copy()
        software['CUTFLOW_RENDERER'] = 'software'
        run([stage / 'CutFlow-editor.exe', '--smoke'], env=software)
        run([sys.executable, '-B', 'tests/check_windows_startup.py', '--exe', stage / 'CutFlow-editor.exe', '--state-dir', root / '.build/editor-software-startup'], env=software)
        run([sys.executable, '-B', 'tests/check_hub.py', '--stage', stage], env=clean)
        recycle(stage / 'portable')
        (stage / 'portable').mkdir()
    component_files = ['CutFlow.exe', 'CutFlow-editor.exe', 'tools/concat.exe', 'tools/cutflow-agent.exe', 'tools/bdl.exe']
    provenance = {'version':VERSION, 'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
                  'cloud_run':os.environ.get('GITHUB_RUN_ID'), 'files':{}}
    for name in component_files:
        path = stage / name
        provenance['files'][name] = {'bytes':path.stat().st_size, 'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
    assert provenance['files']['CutFlow-editor.exe']['sha256'] == provenance['files']['tools/concat.exe']['sha256']
    (stage / 'build-info.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
    archive = Path(str(stage) + '.zip')
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as package:
        for source in sorted(stage.rglob('*')):
            package.write(source, source.relative_to(stage.parent))
    deliverables = [archive]
    if installer:
        compiler = shutil.which('ISCC.exe') or str(Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Inno Setup 6/ISCC.exe')
        run([compiler, f'/DAppVersion={VERSION}', f'/DStage={stage}', f'/DOutput={stage.parent}', root / 'scripts/CutFlow.iss'])
        deliverables.append(stage.parent / f'CutFlow-{VERSION}-windows-x64-setup.exe')
    for path in deliverables:
        with path.open('rb') as source:
            digest = hashlib.file_digest(source, 'sha256').hexdigest()
        Path(str(path) + '.sha256').write_text(digest + '  ' + path.name + '\n', encoding='utf-8')
    print('Built', archive, flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--installer', action='store_true')
    parser.add_argument('--skip-smoke', action='store_true')
    parser.add_argument('--component', choices=['hub', 'editor', 'package', 'all'], default='all')
    options = parser.parse_args()
    build(options.installer, options.skip_smoke, options.component)
