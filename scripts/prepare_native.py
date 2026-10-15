import hashlib
from pathlib import Path
import subprocess
import shutil
import sys
import tarfile
import urllib.request
import zipfile


def prepare():
    root = Path(__file__).resolve().parents[1]
    destination = root / '.build' / 'native'
    destination.mkdir(parents=True, exist_ok=True)
    entries = {}
    for line in (root / 'editor/scripts/ci/deps.lock').read_text().splitlines():
        if line.strip() and not line.startswith('#'):
            name, url, digest = line.split()
            entries[name] = (url, digest)
    for name in ['ffmpeg-win64', 'onnxruntime-win-x64',
                 'sherpa-onnx-win-x64-shared-MT-Release-lib']:
        target = destination / name
        marker = target / '.complete'
        if marker.exists():
            continue
        url, digest = entries[name]
        archive = destination / url.rsplit('/', 1)[1]
        if not archive.exists() or hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() != digest:
            print('Downloading', name, flush=True)
            size = int(urllib.request.urlopen(urllib.request.Request(url, method='HEAD'), timeout=60).headers['Content-Length'])
            offset = archive.stat().st_size if archive.exists() else 0
            if offset >= size:
                offset = 0
            while offset < size:
                request = urllib.request.Request(url, headers={'Range': f'bytes={offset}-{min(offset + 8 * 1024 * 1024 - 1, size - 1)}'})
                with urllib.request.urlopen(request, timeout=60) as response:
                    if offset and response.status != 206:
                        raise RuntimeError('Server refused ranged download')
                    with archive.open('ab' if offset else 'wb') as output:
                        while chunk := response.read(1024 * 1024):
                            output.write(chunk)
                offset = archive.stat().st_size
                print(name, offset, '/', size, flush=True)
        if hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() != digest:
            raise RuntimeError('SHA256 mismatch: ' + name)
        target.mkdir(exist_ok=True)
        if zipfile.is_zipfile(archive):
            with zipfile.ZipFile(archive) as package:
                for member in package.infolist():
                    if not (target / member.filename).resolve().is_relative_to(target.resolve()):
                        raise RuntimeError('Unsafe archive member')
                package.extractall(target)
        else:
            with tarfile.open(archive) as package:
                package.extractall(target, filter='data')
        marker.touch()
    imports=destination / 'sherpa-imports'
    imports.mkdir(exist_ok=True)
    for library in (destination / 'sherpa-onnx-win-x64-shared-MT-Release-lib').glob('*/lib/*.lib'):
        shutil.copy2(library,imports / library.name)
    clang = root / '.build' / 'python'
    if not (clang / 'clang/native/libclang.dll').exists():
        subprocess.run([sys.executable, '-B', '-m', 'pip', 'install', '--no-cache-dir',
                        '--target', str(clang), 'libclang==18.1.1'], check=True)
    print('Native libraries prepared', flush=True)


if __name__ == '__main__':
    prepare()
