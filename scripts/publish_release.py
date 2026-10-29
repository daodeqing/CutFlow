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
                    'tools/cutflow-agent.exe', 'tools/cutflow-transcribe.exe', 'tools/bdl.exe', 'tools/ffmpeg.exe',
                    'tools/ffprobe.exe', 'WebView2/msedgewebview2.exe', 'build-info.json']
        if not all(prefix + name in archive.namelist() for name in required):
            raise RuntimeError('Portable package is missing a required executable or build record')
        provenance = json.loads(archive.read(prefix + 'build-info.json'))
        if (provenance['commit'] != source['head_sha'] or provenance['version'] != version
                or str(provenance['cloud_run']) != run_id):
            raise RuntimeError('Portable package does not match the verified source build')
        component_names = {'CutFlow.exe', 'CutFlow-editor.exe', 'tools/concat.exe',
                           'tools/cutflow-agent.exe', 'tools/cutflow-transcribe.exe', 'tools/bdl.exe'}
        if not component_names.issubset(provenance.get('files', {})):
            raise RuntimeError('Build record is missing a required component checksum')
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
        '从 B 站素材到英文成片：这次把下载、翻译、剪辑和发布审核串进同一个工作台。\n\n'
        '- 新增英文制作：复用视频字幕或按需语音识别，生成英文字幕、精选英文弹幕、视频简介与 X 推文。\n'
        '- 默认模型和各步骤模型可分别选择或手动输入；可输入要求单独重新生成简介与推文，无需重做视频。\n'
        '- 用可选深色字幕条替代底部整片插值，避免出现严重画面拉花；字幕条会遮住底部一部分画面。\n'
        '- 支持监控指定收藏夹或 UP 主的新视频；程序须保持运行或驻留托盘。\n'
        '- 审核后可在浏览器打开带文案的 X 发帖页，或把上传指令交给 Cindy；上传与最终发布仍取决于登录、权限和用户确认。\n'
        '- 更新项目式 AI 创作的任务状态、下载恢复与媒体保存；具体能力取决于模型、账号和服务。\n\n'
        '使用边界：不自动读取 X 实时热榜；AI 接口可能将内容发送到云端并产生费用。真实账号的 B 站监控、X 上传以及 AI 新图、连续改图、4K 和视频生成仍需按实际账号验收。\n\n'
        f'- 安装版：`{package}-setup.exe`；便携版：`{package}.zip`，选择一种即可。\n'
        '- 包内包含 WebView2、FFmpeg、编辑器、语音识别组件及运行库；升级前备份工程和 portable 数据。\n'
        '- 可选 Whisper 模型首次使用时按需下载；需使用有权下载、改编及发布的视频与音乐。\n\n'
        f'Windows 完整构建：{source["html_url"]}\n源代码：`{source["head_sha"]}`\n'
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
