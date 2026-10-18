"""Verify that a package-only rerun reuses matching, successful components."""
import json
import os
import subprocess
import urllib.request


def verify():
    repo = os.environ['GITHUB_REPOSITORY']
    run = os.environ['CUTFLOW_COMPONENT_RUN']
    if not run.isdecimal():
        raise RuntimeError('Component run must be a numeric workflow run ID')
    base = os.environ.get('GITHUB_API_URL', 'https://api.github.com').rstrip('/')
    token = os.environ['GH_TOKEN']

    def api(path):
        request = urllib.request.Request(
            f'{base}/repos/{repo}/{path}',
            headers={'Authorization': f'Bearer {token}', 'Accept': 'application/vnd.github+json'},
        )
        with urllib.request.urlopen(request, timeout=45) as response:
            return json.load(response)

    source = api(f'actions/runs/{run}')
    if source['status'] != 'completed' or source['path'] != '.github/workflows/windows.yml':
        raise RuntimeError('Source must be a completed CutFlow Windows workflow')
    jobs = api(f'actions/runs/{run}/jobs?per_page=100')['jobs']
    for name in ('Build hub', 'Build editor'):
        if not any(job['name'] == name and job['conclusion'] == 'success' for job in jobs):
            raise RuntimeError(f'Source component did not pass: {name}')
    artifacts = api(f'actions/runs/{run}/artifacts?per_page=100')['artifacts']
    for name in ('CutFlow-component-hub', 'CutFlow-component-editor'):
        if not any(item['name'] == name and not item['expired'] for item in artifacts):
            raise RuntimeError(f'Missing source artifact: {name}')

    sha = source['head_sha']
    subprocess.run(['git', 'fetch', '--no-tags', '--depth=1', 'origin', sha], check=True)
    changed = subprocess.check_output(['git', 'diff', '--name-only', sha, 'HEAD'], text=True).splitlines()
    packaging_files = {
        '.github/workflows/windows.yml', 'scripts/build_windows.py',
        'scripts/CutFlow.iss', 'scripts/verify_component_run.py',
        'docs/创作与外部AI.md', 'docs/验收与已知限制.md', 'docs/creation-projects.md',
    }
    unsafe = sorted(set(changed) - packaging_files)
    if unsafe:
        raise RuntimeError('Binary source changed; rebuild components: ' + ', '.join(unsafe))
    print(f'Verified successful hub/editor from run {run}, source {sha}; only packaging changed')


if __name__ == '__main__':
    verify()
