import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys


def environment():
    root = Path(__file__).resolve().parents[1]
    build = root / '.build'
    runtime = build / 'node'
    runtime.mkdir(parents=True, exist_ok=True)
    node = runtime / 'node.exe'
    if not node.exists():
        source = shutil.which('node.exe')
        if not source:
            raise RuntimeError('Install Node.js 22 or newer before building the desktop frontend')
        shutil.copy2(source, node)
    pnpm = runtime / 'pnpm-10/bin/pnpm.cjs'
    if not pnpm.exists():
        import urllib.request
        import tarfile
        archive = runtime / 'pnpm.tgz'
        urllib.request.urlretrieve('https://registry.npmjs.org/pnpm/-/pnpm-10.28.2.tgz', archive)
        with tarfile.open(archive) as package:
            for member in package.getmembers():
                member.name = member.name.replace('package/', 'pnpm-10/', 1)
                package.extract(member, runtime, filter='data')
    result = os.environ.copy()
    for name, folder in [('TEMP', 'tmp'), ('TMP', 'tmp'), ('npm_config_cache', 'npm'),
                         ('PNPM_HOME', 'pnpm-home'), ('XDG_CACHE_HOME', 'cache')]:
        target = build / folder
        target.mkdir(parents=True, exist_ok=True)
        result[name] = str(target)
    result['PATH'] = str(runtime) + os.pathsep + result['PATH']
    result['CI'] = 'true'
    result['npm_config_store_dir'] = str(build / 'pnpm-store')
    return root, node, pnpm, result


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('command', nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    root, node, pnpm, process_environment = environment()
    command = arguments.command
    if command and command[0] == '--':
        command = command[1:]
    process = subprocess.Popen([str(node), str(pnpm), *command],
                               cwd=root / 'reference/bdl/apps/desktop', env=process_environment,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
    for line in process.stdout:
        sys.stdout.buffer.write(line)
        sys.stdout.buffer.flush()
    sys.exit(process.wait())
