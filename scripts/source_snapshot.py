import hashlib
import json
import os
from pathlib import Path
import sys
import subprocess


def snapshot(source):
    excluded = {'.git', 'node_modules', 'target', 'dist', '.bdl', 'portable', 'flightdeck',
                '.build', '.cache', '__pycache__', 'gen', 'test-results', 'playwright-report'}
    files = {}
    for directory, folders, names in os.walk(source):
        folders[:] = sorted(folder for folder in folders if folder not in excluded)
        for name in sorted(names):
            path = Path(directory) / name
            if name in {'auto-imports.d.ts', 'components.d.ts'}:
                continue
            if path.suffix in {'.dpapi', '.sqlite', '.db', '.log'} or name.startswith('.env'):
                continue
            files[path.relative_to(source).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    digest = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    return {'source': str(source), 'algorithm': 'sha256', 'treeHash': digest, 'files': files}


if __name__ == '__main__':
    root = Path(__file__).resolve().parents[1]
    source = Path(sys.argv[1]).resolve(strict=True)
    report = snapshot(source)
    baseline = subprocess.check_output(['git', 'ls-tree', '-r', 'HEAD', 'reference/bdl'], cwd=root, text=True, encoding='utf-8')
    tracked = {}
    for line in baseline.splitlines():
        metadata, path = line.split('\t', 1)
        tracked[path.removeprefix('reference/bdl/')] = metadata.split()[2]
    differences = []
    for path in report['files']:
        content = (source / path).read_bytes()
        if b'\0' not in content:
            content = content.replace(b'\r\n', b'\n')
        blob = hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest()
        if tracked.get(path) != blob:
            differences.append(path)
    report['vendoredBaselineCommit'] = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    report['vendoredBaselineDifferences'] = differences
    target = root / 'docs/bdl-source-snapshot.json'
    target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(f"Recorded {len(report['files'])} source files: {report['treeHash']}")
    print('Differences from the pre-change vendored source:', differences)
