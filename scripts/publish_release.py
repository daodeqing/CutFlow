import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import urllib.error
import urllib.request
import zipfile


def github(path, payload=None, method='GET', missing_ok=False):
    base = os.environ.get('GITHUB_API_URL', 'https://api.github.com').rstrip('/')
    request = urllib.request.Request(
        f'{base}/repos/{os.environ["GITHUB_REPOSITORY"]}/{path}',
        data=json.dumps(payload).encode('utf-8') if payload is not None else None,
        headers={'Authorization': 'Bearer ' + os.environ['GH_TOKEN'],
                 'Accept': 'application/vnd.github+json',
                 'Content-Type': 'application/json'},
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        if missing_ok and error.code == 404:
            return None
        raise RuntimeError(f'GitHub {method} {path} failed: HTTP {error.code}') from None


def source_build(run_id, tag):
    if not run_id.isdecimal() or not re.fullmatch(r'v\d+\.\d+\.\d+', tag):
        raise RuntimeError('Expected a numeric build run ID and a vX.Y.Z release tag')
    source = github(f'actions/runs/{run_id}')
    if (source['status'] != 'completed' or source['conclusion'] != 'success'
            or source['path'] != '.github/workflows/windows.yml'
            or source['head_branch'] != 'main'
            or source['head_repository']['full_name'] != os.environ['GITHUB_REPOSITORY']):
        raise RuntimeError('Release source must be a successful CutFlow Windows build on main')
    jobs = github(f'actions/runs/{run_id}/jobs?per_page=100')['jobs']
    if not any(job['name'] == 'package' and job['conclusion'] == 'success' for job in jobs):
        raise RuntimeError('The Windows package verification job did not pass')
    artifact_name = f'CutFlow-{tag[1:]}-windows-x64'
    artifacts = github(f'actions/runs/{run_id}/artifacts?per_page=100')['artifacts']
    matches = [item for item in artifacts if item['name'] == artifact_name and not item['expired']]
    if len(matches) != 1:
        raise RuntimeError('Expected one unexpired final package: ' + artifact_name)
    print(f'Verified {artifact_name}: run {run_id}, commit {source["head_sha"]}', flush=True)
    return source, artifact_name


def verify_files(directory, package, source, run_id, version):
    names = [package + '.zip', package + '-setup.exe']
    expected = set(names + [name + '.sha256' for name in names])
    actual = {path.name for path in directory.iterdir()}
    if actual != expected or any(not (directory / name).is_file() for name in expected):
        raise RuntimeError('Release artifact must contain exactly the installer, ZIP and two checksums')
    digests = {}
    for name in sorted(expected):
        with (directory / name).open('rb') as stream:
            digests[name] = hashlib.file_digest(stream, 'sha256').hexdigest()
    for name in names:
        checksum = (directory / (name + '.sha256')).read_text(encoding='utf-8').strip().split(maxsplit=1)
        if len(checksum) != 2 or checksum[0] != digests[name] or checksum[1].lstrip('*') != name:
            raise RuntimeError('SHA256 verification failed: ' + name)
    with zipfile.ZipFile(directory / names[0]) as archive:
        prefix = package + '/'
        required = ['CutFlow.exe', 'CutFlow-editor.exe', 'tools/concat.exe',
                    'tools/cutflow-agent.exe', 'tools/bdl.exe', 'tools/ffmpeg.exe',
                    'tools/ffprobe.exe', 'WebView2/msedgewebview2.exe', 'build-info.json']
        if not all(prefix + name in archive.namelist() for name in required):
            raise RuntimeError('Portable package is missing a required executable or build record')
        provenance = json.loads(archive.read(prefix + 'build-info.json'))
        if (provenance['commit'] != source['head_sha'] or provenance['version'] != version
                or str(provenance['cloud_run']) != run_id):
            raise RuntimeError('Portable package does not match the verified source build')
        for name, record in provenance['files'].items():
            with archive.open(prefix + name) as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            if digest != record['sha256'] or archive.getinfo(prefix + name).file_size != record['bytes']:
                raise RuntimeError('Packaged executable does not match its build record: ' + name)
    print('Verified both download checksums and portable executable provenance', flush=True)
    return digests


def tag_commit(tag):
    reference = github(f'git/ref/tags/{tag}', missing_ok=True)
    if reference is None:
        return None
    target = reference['object']
    for depth in range(5):
        if target['type'] == 'commit':
            return target['sha']
        if target['type'] != 'tag':
            break
        target = github('git/tags/' + target['sha'])['object']
    raise RuntimeError('Release tag does not resolve to a commit')


def verify_uploaded(release, directory, digests):
    assets = {asset['name']: asset for asset in release['assets']}
    for name, digest in digests.items():
        asset = assets.get(name)
        if (asset is None or asset['state'] != 'uploaded'
                or asset['size'] != (directory / name).stat().st_size
                or asset.get('digest') != 'sha256:' + digest):
            raise RuntimeError('Uploaded release asset failed verification: ' + name)


def publish(directory, tag, run_id, source, package, digests):
    commit = tag_commit(tag)
    if commit is not None and commit != source['head_sha']:
        raise RuntimeError('Existing release tag points to another source commit')
    notes = (
        f'CutFlow {tag[1:]} · Windows x64\n\n'
        'B站素材、独立 Concat 剪辑窗口，以及兼容 OpenAI Chat Completions 的图片和视频创作。'
        '本版修复创作图片卡片模糊、模型菜单挤压布局、项目能力刷新，以及生成状态缺少动态进度的问题；移除顶栏 Wabi 方案按钮。\n\n'
        '- 创作记录直接显示原图，模型选择使用默认收起的悬浮菜单。\n'
        '- 支持动画和阶段百分比；按处理阶段更新，保存成功才到 100%，并非耗时或像素完成率。\n'
        '- 4K 选项固定保留，模型或账号未开放时禁用；当前账号能力以刷新结果为准。\n'
        '- 请同时更新 GFlow 0.1.15，修复 2K/4K 缓存回传失败并启用结构化阶段进度。插件安装在代理服务中。\n\n'
        f'- 安装版：`{package}-setup.exe`，运行安装程序。\n'
        f'- 便携版：`{package}.zip`，完整解压后运行 `CutFlow.exe`。\n'
        '- 两种版本选择一种即可，旁边的 `.sha256` 文件用于校验下载。\n'
        '- 包内包含 WebView2、FFmpeg、编辑器及所需运行库。升级前备份工程和 `portable` 数据。\n\n'
        f'复用已通过 Windows 构建、双窗口启动和联动检查的[构建产物]({source["html_url"]})。'
        'B站会员画质下载和真实 AI 生成仍需使用自己的账号及 API 服务验证。\n\n'
        f'源代码：`{source["head_sha"]}`\n'
    )
    release = github(f'releases/tags/{tag}', missing_ok=True)
    if release is None:
        release = github('releases', {'tag_name': tag, 'target_commitish': source['head_sha'],
                                     'name': 'CutFlow ' + tag[1:], 'body': notes,
                                     'draft': True, 'prerelease': False}, method='POST')
    assets = {asset['name']: asset for asset in release['assets']}
    missing = []
    for name, digest in digests.items():
        if name not in assets:
            missing.append(str(directory / name))
        elif (assets[name]['state'] != 'uploaded'
                or assets[name]['size'] != (directory / name).stat().st_size
                or assets[name].get('digest') != 'sha256:' + digest):
            raise RuntimeError('Existing release asset differs; refusing to replace it: ' + name)
    if missing:
        subprocess.run(['gh', 'release', 'upload', tag, *missing,
                        '--repo', os.environ['GITHUB_REPOSITORY']], check=True, timeout=900)
    verify_uploaded(github(f'releases/{release["id"]}'), directory, digests)
    release = github(f'releases/{release["id"]}',
                     {'draft': False, 'prerelease': False, 'make_latest': 'true'}, method='PATCH')
    verify_uploaded(release, directory, digests)
    if tag_commit(tag) != source['head_sha'] or github('releases/latest')['tag_name'] != tag:
        raise RuntimeError('Published tag or latest release does not match the verified build')
    print(json.dumps({'url': release['html_url'], 'tag': tag, 'sourceRun': run_id,
                      'assets': [{'name': asset['name'], 'bytes': asset['size'],
                                  'sha256': asset.get('digest')} for asset in release['assets']]},
                     ensure_ascii=False, indent=2), flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--assets-dir', type=Path)
    options = parser.parse_args()
    run_id = os.environ['CUTFLOW_RELEASE_RUN']
    tag = os.environ['CUTFLOW_RELEASE_TAG']
    source, package = source_build(run_id, tag)
    if options.prepare:
        if os.environ.get('GITHUB_OUTPUT'):
            with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as output:
                output.write(f'artifact_name={package}\n')
        return
    if options.assets_dir is None:
        parser.error('--assets-dir is required for publishing')
    digests = verify_files(options.assets_dir, package, source, run_id, tag[1:])
    publish(options.assets_dir, tag, run_id, source, package, digests)


if __name__ == '__main__':
    main()
