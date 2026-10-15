import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import urllib.request


def prepare(destination, resolve=False):
    root = Path(__file__).resolve().parents[1]
    lock = root / 'scripts/webview2.lock.json'
    if resolve:
        with urllib.request.urlopen('https://developer.microsoft.com/en-us/microsoft-edge/webview2/', timeout=60) as response:
            html = response.read().decode().replace('\\u002F', '/')
        urls = re.findall(r'https://msedge\.sf\.dl\.delivery\.mp\.microsoft\.com/filestreamingservice/files/[a-z0-9-]+/Microsoft\.WebView2\.FixedVersionRuntime\.([\d.]+)\.x64\.cab', html)
        if not urls:
            raise RuntimeError('Official WebView2 fixed runtime URL not found')
        version = max(urls, key=lambda version: tuple(map(int, version.split('.'))))
        url = re.search(r'https://msedge\.sf\.dl\.delivery\.mp\.microsoft\.com/filestreamingservice/files/[a-z0-9-]+/Microsoft\.WebView2\.FixedVersionRuntime\.' + re.escape(version) + r'\.x64\.cab', html).group()
        entry = {'version': version, 'url': url}
    else:
        entry = json.loads(lock.read_text(encoding='utf-8'))
    native = root / '.build/native/webview2'
    native.mkdir(parents=True, exist_ok=True)
    archive = native / ('WebView2-' + entry['version'] + '.cab')
    if not archive.exists():
        print('Downloading WebView2', entry['version'], flush=True)
        with urllib.request.urlopen(entry['url'], timeout=120) as response, archive.open('wb') as output:
            shutil.copyfileobj(response, output, 1024 * 1024)
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    if resolve:
        entry['sha256'] = digest
        lock.write_text(json.dumps(entry, indent=2) + '\n', encoding='utf-8')
    elif digest != entry['sha256']:
        raise RuntimeError('WebView2 archive SHA256 mismatch')
    unpacked = native / entry['version']
    if not unpacked.is_dir():
        unpacked.mkdir()
        subprocess.run(['expand.exe', str(archive), '-F:*', str(unpacked)], check=True,
                       stdout=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
    candidates = list(unpacked.rglob('msedgewebview2.exe'))
    if len(candidates) != 1:
        raise RuntimeError('Expected one WebView2 runtime executable')
    if not destination.exists():
        shutil.copytree(candidates[0].parent, destination)
    shutil.copy2(lock, destination / 'cutflow-runtime.json')
    return entry


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--resolve', action='store_true')
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    print(prepare(root / '.build/webview-runtime', arguments.resolve))
