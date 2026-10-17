"""核对发行版本入口，防止软件窗口、CLI 与构建产物标错版本。"""
import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / 'reference/bdl/apps/desktop'


def check_versions():
    version = json.loads((DESKTOP / 'src-tauri/tauri.conf.json').read_text(encoding='utf-8'))['version']
    assert re.fullmatch(r'\d+\.\d+\.\d+', version), version
    files = {
        'frontend': json.loads((DESKTOP / 'package.json').read_text(encoding='utf-8'))['version'],
        'desktop': tomllib.loads((DESKTOP / 'src-tauri/Cargo.toml').read_text(encoding='utf-8'))['package']['version'],
        'workbench': tomllib.loads((ROOT / 'workbench/Cargo.toml').read_text(encoding='utf-8'))['package']['version'],
    }
    for label, actual in files.items():
        assert actual == version, f'{label}: {actual} != {version}'
    for relative, names in [('reference/bdl/Cargo.lock', ['cutflow-desktop', 'cutflow-workbench']),
                            ('workbench/Cargo.lock', ['cutflow-workbench'])]:
        packages = tomllib.loads((ROOT / relative).read_text(encoding='utf-8'))['package']
        for name in names:
            actual = next(package['version'] for package in packages if package['name'] == name)
            assert actual == version, f'{relative}: {name} {actual} != {version}'
    expected = {
        '.github/workflows/windows.yml': f'name: CutFlow-{version}-windows-x64',
        '.github/workflows/release.yml': f"default: 'v{version}'",
        'reference/bdl/apps/desktop/src/shared/ui/WabiVisuals.vue': f'CutFlow {version}',
        'reference/bdl/apps/desktop/tests/visual/creation-studio.mock.ts': f"return '{version}';",
        'README.md': f'CutFlow-{version}-windows-x64.zip',
    }
    for relative, marker in expected.items():
        assert marker in (ROOT / relative).read_text(encoding='utf-8'), f'{relative} lacks {marker}'
    agent = (ROOT / 'workbench/src/agent.rs').read_text(encoding='utf-8')
    assert agent.count('env!("CARGO_PKG_VERSION")') >= 2, 'CLI and MCP must use the package version'
    assert f'CutFlow {version}' in (ROOT / 'README.md').read_text(encoding='utf-8')
    return {'version': version, 'status': 'passed', 'manifests': 4, 'lockEntries': 3, 'labels': len(expected)}


if __name__ == '__main__':
    print(json.dumps(check_versions(), ensure_ascii=False))
