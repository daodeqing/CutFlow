import argparse
import base64
from collections import Counter
import ctypes
from ctypes import wintypes
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import threading
import time
from check_windows_startup import capture_webview_children, close_webview_children


def close_window(process):
    if process.poll() is not None:
        return
    user32 = ctypes.WinDLL('user32', use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]

    @callback_type
    def close(window, _parameter):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
        if owner.value == process.pid:
            user32.PostMessageW(window, 0x0010, 0, 0)
        return True

    webviews = capture_webview_children(process)
    user32.EnumWindows(close, 0)
    process.wait(timeout=15)
    close_webview_children(webviews)
    if process.returncode:
        raise RuntimeError(f'Window exited with code {process.returncode}')


def verify(stage):
    portable = stage / 'portable'
    fixtures = portable / 'smoke/fixtures'
    fixtures.mkdir(parents=True, exist_ok=True)
    ffmpeg = stage / 'tools/ffmpeg.exe'
    flags = subprocess.CREATE_NO_WINDOW
    subprocess.run([str(ffmpeg), '-v', 'error', '-y', '-f', 'lavfi', '-i', 'color=blue:s=64x64', '-frames:v', '1', str(fixtures / 'image.png')], check=True, capture_output=True, creationflags=flags)
    subprocess.run([str(ffmpeg), '-v', 'error', '-y', '-f', 'lavfi', '-i', 'color=blue:s=64x64:r=25:d=1', '-f', 'lavfi', '-i', 'anullsrc=r=44100:cl=mono', '-t', '1', '-c:v', 'mpeg4', '-c:a', 'aac', str(fixtures / 'video.mp4')], check=True, capture_output=True, creationflags=flags)
    subprocess.run([str(ffmpeg), '-v', 'error', '-y', '-i', str(fixtures / 'image.png'), '-frames:v', '1', str(fixtures / 'image.jpg')], check=True, capture_output=True, creationflags=flags)
    posts = Counter()
    run_id = str(time.time_ns())
    authorized_media = []
    retry_enabled = [False]

    class Provider(BaseHTTPRequestHandler):
        def log_message(self, *_arguments):
            pass

        def send(self, status, content_type, body):
            self.send_response(status)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path == '/v1/models':
                assert self.headers.get('Authorization') == 'Bearer cutflow-smoke-key'
                self.send(200, 'application/json', json.dumps({'data': [{'id': 'custom-image'}, {'id': 'veo-test-video'}]}).encode())
                return
            authorized_media.append(self.headers.get('Authorization'))
            if self.path == '/retry.png' and not retry_enabled[0]:
                self.send(503, 'text/plain', b'retry media download')
                return
            filename = 'video.mp4' if self.path == '/video.mp4' else 'image.png'
            self.send(200, 'video/mp4' if filename.endswith('mp4') else 'image/png', (fixtures / filename).read_bytes())

        def do_POST(self):
            payload = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            assert self.path in ['/v1/chat/completions', '/v1/images/generations']
            assert self.headers.get('Authorization') == 'Bearer cutflow-smoke-key'
            model = payload['model']
            posts[model] += 1
            if self.path == '/v1/images/generations':
                assert payload['n'] == 1 and payload['size'] == '1024x1024' and payload['quality'] == 'low'
                self.send(200, 'application/json', json.dumps({'data':[{'b64_json':base64.b64encode((fixtures/'image.png').read_bytes()).decode()}]}).encode())
                return
            if model == 'jpeg-image':
                data = 'data:image/jpeg;base64,' + base64.b64encode((fixtures/'image.jpg').read_bytes()).decode()
                self.send(200, 'application/json', json.dumps({'choices':[{'message':{'content':f'![result]({data})'}}]}).encode())
                return
            if model == 'quota':
                self.send(402, 'application/json', b'{"error":{"code":"insufficient_quota"}}')
                return
            if model == 'custom-image':
                assert payload['messages'][0]['content'][1]['image_url']['url'].startswith('data:image/png;base64,')
            address = f'http://127.0.0.1:{self.server.server_port}'
            filename = '/video.mp4' if model == 'veo-test-video' else '/retry.png' if model == 'retry' else '/image.png'
            content = f'![result]({address}{filename})'
            if payload.get('stream', True):
                body = 'data: ' + json.dumps({'choices': [{'delta': {'content': content}}]}) + '\n\n'
                if model != 'broken-stream':
                    body += 'data: [DONE]\n\n'
                self.send(200, 'text/event-stream', body.encode())
            else:
                self.send(200, 'application/json', json.dumps({'choices': [{'message': {'content': content}}]}).encode())

    provider = ThreadingHTTPServer(('127.0.0.1', 0), Provider)
    threading.Thread(target=provider.serve_forever, daemon=True).start()
    agent = stage / 'tools/cutflow-agent.exe'

    def call(method, params=None):
        for attempt in range(3):
            output = subprocess.run([str(agent), 'call', method, '--stdin'], input=json.dumps(params or {}).encode(), capture_output=True, timeout=130, creationflags=flags)
            if not output.returncode or method not in ['cutflow.jobs.get', 'cutflow.editor.status', 'cutflow.connection.get']:
                break
            if attempt < 2:
                time.sleep(0.25)
        if output.returncode:
            raise RuntimeError(f'Hub exit={hub.poll()}: ' + output.stderr.decode(errors='replace'))
        return json.loads(output.stdout)

    def complete(job_id, expected='completed'):
        deadline = time.monotonic() + 80
        while time.monotonic() < deadline:
            job = call('cutflow.jobs.get', {'jobId': job_id})
            if job['status'] not in ['queued', 'running']:
                assert job['status'] == expected, job
                return job
            time.sleep(0.2)
        raise RuntimeError('Generation task did not finish')

    env = os.environ.copy()
    env['CUTFLOW_RENDERER'] = 'software'
    editor = None
    with (fixtures / 'hub.log').open('wb') as log:
        hub = subprocess.Popen([str(stage / 'CutFlow.exe')], cwd=stage, env=env, stdout=log, stderr=log, creationflags=flags)
        try:
            deadline = time.monotonic() + 40
            while True:
                if hub.poll() is not None:
                    raise RuntimeError('Main window exited before control was available')
                try:
                    call('cutflow.connection.get')
                    break
                except (RuntimeError, json.JSONDecodeError):
                    if time.monotonic() > deadline:
                        raise
                    time.sleep(0.3)
            call('cutflow.connection.save', {'baseUrl': f'http://127.0.0.1:{provider.server_port}', 'apiKey': 'cutflow-smoke-key'})
            assert len(call('cutflow.models.list')['models']) == 2
            request = {'operationId': run_id + '-smoke-image', 'kind': 'image', 'model': 'custom-image', 'prompt': 'fixture', 'references': [str(fixtures / 'image.png')], 'stream': False, 'maxGenerationCalls': 1}
            submitted = call('cutflow.generate', request)
            assert call('cutflow.generate', request)['jobId'] == submitted['jobId']
            image = complete(submitted['jobId'])['result']['assets'][0]
            assert image['width'] == 64 and Path(image['path']).is_relative_to(portable / 'generated')
            jpeg_job = call('cutflow.generate', {'operationId':run_id+'-jpeg-image','kind':'image','model':'jpeg-image','prompt':'fixture','maxGenerationCalls':1,'stream':False})
            jpeg = complete(jpeg_job['jobId'])['result']['assets'][0]
            assert jpeg['stillImage'] is True and jpeg['duration'] is None and jpeg['width'] == 64
            for model, expected in [('veo-test-video', 'completed'), ('broken-stream', 'result_unknown'), ('quota', 'failed'), ('retry', 'failed')]:
                request = {'operationId': run_id + '-smoke-' + model, 'kind': 'video' if model == 'veo-test-video' else 'image', 'model': model, 'prompt': 'fixture', 'maxGenerationCalls': 1}
                submitted = call('cutflow.generate', request)
                job = complete(submitted['jobId'], expected)
                if model == 'veo-test-video':
                    assert job['result']['assets'][0]['hasAudio'] is True
                if model == 'quota':
                    assert job['error']['code'] == 'insufficient_credit'
                if model == 'retry':
                    retry_enabled[0] = True
                    call('cutflow.jobs.retryDownload', {'jobId': submitted['jobId']})
                    complete(submitted['jobId'])
                    assert posts[model] == 1
            submitted = call('cutflow.generate', {'operationId':run_id+'-multi-image', 'kind':'image', 'model':'gpt-image-1.5', 'prompt':'fixture', 'n':4, 'maxGenerationCalls':4, 'aspect_ratio':'1:1', 'image_size':'low'})
            images = complete(submitted['jobId'])['result']['assets']
            assert len(images) == 4 and len(set(image['path'] for image in images)) == 4
            assert posts['gpt-image-1.5'] == 4
            assert posts['custom-image'] == 1 and posts['broken-stream'] == 1
            assert authorized_media and all(value is None for value in authorized_media)
            if (stage / 'CutFlow-editor.exe').is_file():
                # 冷启动必须携带 --project，并保留中文与空格路径边界。
                project = fixtures / '剪辑工程 with spaces'
                project.mkdir()
                (project / 'concat.json').write_text(json.dumps({
                    'concat':'0.2.7', 'name':'剪辑启动回归',
                    'video':{'width':1280,'height':720,'rateNum':25,'rateDen':1}
                }, ensure_ascii=False), encoding='utf-8')
                call('cutflow.editor.open', {'projectPath':str(project)})
                editor = json.loads((portable / 'editor/automation.json').read_text())['process']
                current = call('cutflow.project.current')
                assert os.path.samefile(current['projectPath'], project)
                recents = call('cutflow.editor.recents')
                assert any(os.path.samefile(item['path'], project) for item in recents)
                imported = call('cutflow.assets.import', {
                    'projectPath':str(project),'expectedRevision':current['revision'],
                    'operationId':run_id+'-editor-import','files':[str(fixtures/'image.png')],'place':False})
                complete(imported['jobId'])
                first = json.loads((portable / 'editor/automation.json').read_text())['process']
                call('cutflow.editor.open')
                assert json.loads((portable / 'editor/automation.json').read_text())['process'] == first
                editor = first
            close_window(hub)
            if isinstance(editor, int):
                assert call('cutflow.editor.status')['running'] is True
                user32 = ctypes.WinDLL('user32')
                callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
                user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
                user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
                user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
                @callback_type
                def close_editor(window, _parameter):
                    owner = wintypes.DWORD()
                    user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
                    if owner.value == editor:
                        user32.PostMessageW(window, 0x0010, 0, 0)
                    return True
                user32.EnumWindows(close_editor, 0)
            print('PASS: mock OpenAI models, reference upload, image/video, four image API outputs, no-project generation, replay, broken stream, quota, download-only retry, no leaked media key; ' + ('independent editor verified' if editor else 'editor check deferred: binary not present in this stage'))
        finally:
            if hub.poll() is None:
                hub.terminate()
                hub.wait(timeout=10)
            if isinstance(editor, int):
                subprocess.run(['taskkill.exe', '/PID', str(editor), '/T', '/F'], capture_output=True, creationflags=flags)
            provider.shutdown()
            provider.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--stage', type=Path, required=True)
    options = parser.parse_args()
    verify(options.stage.resolve(strict=True))
