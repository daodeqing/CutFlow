"""Compare two real Windows WebView2 apps using isolated, synthetic project data.

No installers, builds, user state, remote providers, or paid generation are used.
Run --help for arguments. Each output folder is created once and retained for audit.
"""
import argparse
import base64
from collections import Counter
import ctypes
from ctypes import wintypes
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
INSTANCE = str(uuid.uuid5(uuid.NAMESPACE_URL, "cutflow-navigation-fixture/instance"))
WORKSPACE = str(uuid.uuid5(uuid.NAMESPACE_URL, "cutflow-navigation-fixture/workspace"))
PROJECT = str(uuid.uuid5(uuid.NAMESPACE_URL, "cutflow-navigation-fixture/project"))
FLOW_PROJECT = str(uuid.uuid5(uuid.NAMESPACE_URL, "cutflow-navigation-fixture/flow"))
KEY = "cutflow-local-navigation-fixture"
PNG = base64.b64decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a4T0AAAAASUVORK5CYII=")
MODEL = {"id":"gflow-nano-banana-2-lite","description":"Synthetic navigation fixture","kind":"image",
         "maxReferences":3,"duration":None,"aspect":None,"aspects":["1:1","16:9","9:16"],"imageSizes":["1K"],"durations":[]}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def file_version(path):
    version=ctypes.WinDLL("version",use_last_error=True)
    version.GetFileVersionInfoSizeW.argtypes=[wintypes.LPCWSTR,ctypes.POINTER(wintypes.DWORD)]
    version.GetFileVersionInfoSizeW.restype=wintypes.DWORD
    size=version.GetFileVersionInfoSizeW(str(path),None)
    if not size:return None
    buffer=ctypes.create_string_buffer(size)
    version.GetFileVersionInfoW.argtypes=[wintypes.LPCWSTR,wintypes.DWORD,wintypes.DWORD,ctypes.c_void_p]
    if not version.GetFileVersionInfoW(str(path),0,size,buffer):return None
    value=ctypes.c_void_p();length=wintypes.UINT()
    version.VerQueryValueW.argtypes=[ctypes.c_void_p,wintypes.LPCWSTR,ctypes.POINTER(ctypes.c_void_p),ctypes.POINTER(wintypes.UINT)]
    if not version.VerQueryValueW(buffer,"\\",ctypes.byref(value),ctypes.byref(length)):return None
    class Fixed(ctypes.Structure):
        _fields_=[("signature",wintypes.DWORD),("structVersion",wintypes.DWORD),("fileMS",wintypes.DWORD),("fileLS",wintypes.DWORD)]
    info=ctypes.cast(value,ctypes.POINTER(Fixed)).contents
    return ".".join(str(n) for n in (info.fileMS>>16,info.fileMS&65535,info.fileLS>>16,info.fileLS&65535))


def provenance(source):
    metadata={"source":str(source),"version":file_version(source),"sha256":digest(source)}
    info=source.parent/"build-info.json"
    if info.exists():
        metadata["buildInfo"]=json.loads(info.read_text(encoding="utf-8"))
        expected=metadata["buildInfo"].get("files",{}).get(source.name,{}).get("sha256")
        if expected and expected!=metadata["sha256"]:
            raise ValueError("Executable hash does not match its build-info.json: "+str(source))
    return metadata


def json_write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def require_d(path):
    path = path.resolve(strict=True)
    if path.drive.lower() != "d:":
        raise ValueError("This task allows runtime and files on D: only: " + str(path))
    return path


def draft():
    return {"model":MODEL["id"],"modelSource":"catalog","prompt":"","aspect":"auto","imageSize":"auto",
            "count":"1","duration":"8","references":[],"stream":True,"independent":False,"selectedAssetPath":""}


def seed(stage, version, address):
    root = stage / "portable"
    fixture = root / "fixtures/thumbnail.png"
    fixture.parent.mkdir(parents=True)
    fixture.write_bytes(PNG)
    timestamp = 1800000000000
    sessions = [{"id":str(uuid.uuid5(uuid.NAMESPACE_URL, "navigation-session/" + str(i))),"projectId":PROJECT,
                 "title":"测试会话 " + str(i),"createdAt":timestamp+i,"updatedAt":timestamp+i,
                 "drafts":{"image":draft(),"video":draft()}} for i in range(500)]
    # Put 1,000 turns in the most recent session to exercise paged history.
    sid = sessions[-1]["id"]
    jobs, turns = [], []
    for i in range(1000):
        ident = str(uuid.uuid5(uuid.NAMESPACE_URL, "navigation-job/" + str(i)))
        params = {"operationId":"fixture-"+str(i),"model":MODEL["id"],"kind":"image","prompt":"本地测试图片 "+str(i),"maxGenerationCalls":1}
        if version == 2:
            params.update(creationProjectId=PROJECT,sessionId=sid,turnId="turn-"+ident)
        entry = {"id":ident,"operationId":params["operationId"],"method":"cutflow.generate","params":params,
                 "status":"completed","stage":"done","result":{"assets":[{"path":str(fixture),"thumbnail":str(fixture),"width":1,"height":1,"stillImage":True}],"thumbnail":str(fixture)},"error":None}
        if version == 2:
            entry.update(revision=i+1,createdAt=timestamp+i,updatedAt=timestamp+i)
            turns.append({"id":"turn-"+ident,"sessionId":sid,"jobId":ident,"prompt":params["prompt"],"params":params,"createdAt":timestamp+i})
        jobs.append(entry)
    store = {"version":version,"connection":{"baseUrl":address+"/v1"},"models":[MODEL],"references":[],"jobs":jobs}
    if version == 2:
        store.update(workspaceId=WORKSPACE,instanceId=INSTANCE,projectBinding=True,revision=1001,
                     projects=[{"id":PROJECT,"name":"模拟项目","state":"ready","flowProjectId":FLOW_PROJECT,
                                "instanceId":INSTANCE,"accountLabel":"本机模拟账号","models":[MODEL],
                                "createdAt":timestamp,"updatedAt":timestamp}],sessions=sessions,turns=turns)
    json_write(root/"workbench.json",store)
    json_write(root/"data/settings.json",{"download_dir":str(root/"downloads"),"auto_check_updates":False,"usage_notice_acknowledged":True})
    json_write(root/"layout-version.json",{"version":"0.4.0"})
    return {"jobs":1000,"sessions":500 if version == 2 else 0,"storeVersion":version}


class FixtureProvider(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        self.posts = Counter()
        self.cancel_media = threading.Event()
        self.media_started = threading.Event()
        super().__init__(("127.0.0.1",0),Provider)


class Provider(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def send_json(self, value, status=200):
        body=json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type","application/json")
        self.send_header("Content-Length",str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if self.path != "/v1/chat/completions" or self.headers.get("Authorization") != "Bearer "+KEY:
            return self.send_json({"error":{"code":"fixture_only","message":"Only the local test credential is allowed"}},403)
        payload=json.loads(self.rfile.read(int(self.headers.get("Content-Length","0"))))
        self.server.posts[payload.get("model","")] += 1
        if payload.get("model") == "gflow-project-control":
            control=json.loads(payload["messages"][0]["content"])
            if control.get("op") != "capabilities":
                return self.send_json({"error":{"code":"fixture_readonly","message":"This benchmark never creates projects"}},400)
            value={"version":1,"instanceId":INSTANCE,"workspaceId":control["workspaceId"],
                   "projectBinding":True,"capabilities":{"projectBinding":True},"models":[MODEL]}
            return self.send_json({"choices":[{"message":{"content":json.dumps(value)}}]})
        if payload.get("model") != "custom-image":
            return self.send_json({"error":{"code":"fixture_only","message":"Real models are not enabled in this benchmark"}},400)
        address="http://127.0.0.1:"+str(self.server.server_port)+"/slow.png"
        self.send_json({"choices":[{"message":{"content":"![local fixture]("+address+")"}}]})

    def do_GET(self):
        if self.path == "/v1/models":
            return self.send_json({"data":[MODEL]})
        if self.path != "/slow.png":
            return self.send_json({"error":{"message":"Unknown local fixture"}},404)
        # A real local response download remains in flight until this run is closed.
        self.send_response(200)
        self.send_header("Content-Type","image/png")
        self.send_header("Content-Length",str(1024*1024))
        self.end_headers()
        try:
            self.wfile.write(PNG); self.wfile.flush()
            self.server.posts["media-download"] += 1
            self.server.media_started.set()
            while not self.server.cancel_media.wait(0.1):
                self.wfile.write(b"\0"*1024); self.wfile.flush()
        except (OSError, ConnectionError):
            pass


def port_number():
    with socket.socket() as server:
        server.bind(("127.0.0.1",0))
        return server.getsockname()[1]


def ready(endpoint, process):
    deadline=time.monotonic()+45
    while time.monotonic()<deadline:
        if process.poll() is not None:
            raise RuntimeError("Isolated app exited before CDP was ready")
        try:
            with urllib.request.urlopen(endpoint+"/json/version",timeout=1) as response:
                return json.load(response)
        except (OSError, ValueError):
            time.sleep(0.2)
    raise RuntimeError("WebView2 CDP unavailable; release did not accept WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS")


def call(stage, method, params=None):
    result=subprocess.run([str(stage/"tools/cutflow-agent.exe"),"call",method,"--stdin"],
                          input=json.dumps(params or {}).encode(),capture_output=True,timeout=130,
                          creationflags=subprocess.CREATE_NO_WINDOW)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8",errors="replace"))
    return json.loads(result.stdout)


def resize_and_close(process, close=False):
    user=ctypes.WinDLL("user32",use_last_error=True)
    callback_type=ctypes.WINFUNCTYPE(wintypes.BOOL,wintypes.HWND,wintypes.LPARAM)
    user.EnumWindows.argtypes=[callback_type,wintypes.LPARAM]
    user.GetWindowThreadProcessId.argtypes=[wintypes.HWND,ctypes.POINTER(wintypes.DWORD)]
    user.SetWindowPos.argtypes=[wintypes.HWND,wintypes.HWND,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,wintypes.UINT]
    user.PostMessageW.argtypes=[wintypes.HWND,wintypes.UINT,wintypes.WPARAM,wintypes.LPARAM]
    @callback_type
    def visit(window,_):
        owner=wintypes.DWORD()
        user.GetWindowThreadProcessId(window,ctypes.byref(owner))
        if owner.value==process.pid:
            if close:user.PostMessageW(window,0x0010,0,0)
            else:user.SetWindowPos(window,None,0,0,1440,900,0x0040)
        return True
    user.EnumWindows(visit,0)


def terminate_owned(process):
    import psutil
    try:
        parent=psutil.Process(process.pid)
        children=parent.children(recursive=True)
    except psutil.Error:
        children=[]
    if process.poll() is None:
        resize_and_close(process,close=True)
        try:process.wait(timeout=5)
        except subprocess.TimeoutExpired:process.terminate();process.wait(timeout=5)
    _,alive=psutil.wait_procs(children,timeout=2)
    for child in alive:
        try:child.terminate()
        except psutil.Error:pass
    _,alive=psutil.wait_procs(alive,timeout=2)
    for child in alive:
        try:child.kill()
        except psutil.Error:pass


def percentile(values, quantile):
    values=sorted(values)
    return values[max(0,min(len(values)-1,__import__("math").ceil(len(values)*quantile)-1))]


def run_one(label, source, version, scenario, options, provider):
    stage=options.output/(label+"-"+scenario)
    stage.mkdir()
    executable=stage/"CutFlow.exe"
    shutil.copy2(source,executable)
    (stage/"tools").mkdir()
    agent=options.agent_exe or source.parent/"tools/cutflow-agent.exe"
    shutil.copy2(agent,stage/"tools/cutflow-agent.exe")
    address="http://127.0.0.1:"+str(provider.server_port)
    fixture=seed(stage,version,address)
    environment=os.environ.copy()
    for name in ("APPDATA","LOCALAPPDATA","TEMP","TMP"):
        directory=stage/"environment"/name.lower()
        directory.mkdir(parents=True)
        environment[name]=str(directory)
    port=port_number()
    endpoint="http://127.0.0.1:"+str(port)
    environment.update(WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=str(options.runtime_dir),
                       WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port="+str(port))
    posts_before=provider.posts.copy()
    process=None;download_job=None
    with (stage/"application.log").open("wb") as log:
        try:
            process=subprocess.Popen([str(executable)],cwd=stage,env=environment,stdout=log,stderr=log,
                                     creationflags=subprocess.CREATE_NO_WINDOW)
            initial_version=ready(endpoint,process)
            resize_and_close(process)
            deadline=time.monotonic()+45
            while True:
                try:
                    call(stage,"cutflow.connection.get")
                    break
                except (RuntimeError,ValueError):
                    if process.poll() is not None or time.monotonic()>=deadline:raise
                    time.sleep(0.2)
            call(stage,"cutflow.connection.save",{"baseUrl":address,"apiKey":KEY})
            if version==2:call(stage,"cutflow.creation.catalog",{"refresh":True})
            if scenario=="active-download":
                provider.cancel_media.clear()
                provider.media_started.clear()
                download_job=call(stage,"cutflow.generate",{"operationId":"local-active-download","model":"custom-image",
                    "prompt":"local test only","kind":"image","maxGenerationCalls":1,"stream":False})["jobId"]
                if not provider.media_started.wait(10):
                    raise RuntimeError("Local synthetic media download did not start")
            probe=subprocess.run([str(options.node),str(ROOT/"tests/navigation_webview_probe.mjs"),endpoint,str(options.rounds)],
                                 cwd=ROOT,env=environment,capture_output=True,text=True,encoding="utf-8",timeout=300,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
            (stage/"probe.stderr.log").write_text(probe.stderr,encoding="utf-8")
            if probe.returncode:
                raise RuntimeError("WebView navigation probe failed: "+probe.stderr+probe.stdout)
            result=json.loads(probe.stdout.strip().splitlines()[-1])
            result.update(label=label,scenario=scenario,fixture=fixture,sourceProvenance=provenance(source),rpcTransport={"source":str(agent),"sha256":digest(agent),"sharedTransport":bool(options.agent_exe)},
                          executableSha256=digest(executable),
                          initialWebView=initial_version,providerRequests=dict(provider.posts-posts_before))
            warm=[entry["firstFrameMs"] for entry in result["measurements"] if entry["phase"]=="warm"]
            cold=[entry["firstFrameMs"] for entry in result["measurements"] if entry["phase"]=="cold"]
            result["summary"]={"warmCount":len(warm),"warmP50Ms":percentile(warm,.5),"warmP95Ms":percentile(warm,.95),
                               "coldCount":len(cold),"coldMaxMs":max(cold)}
            json_write(stage/"result.json",result)
            return result
        finally:
            if download_job and process and process.poll() is None:
                try:call(stage,"cutflow.jobs.cancel",{"jobId":download_job})
                except (OSError,RuntimeError,subprocess.TimeoutExpired):pass
            provider.cancel_media.set()
            if download_job and process and process.poll() is None:
                deadline=time.monotonic()+3
                while time.monotonic()<deadline:
                    try:
                        if call(stage,"cutflow.jobs.get",{"jobId":download_job})["status"] not in ("queued","running"):
                            break
                    except (OSError,RuntimeError,subprocess.TimeoutExpired):
                        break
                    time.sleep(0.1)
            if process:terminate_owned(process)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-exe",type=Path,required=True)
    parser.add_argument("--candidate-exe",type=Path,required=True)
    parser.add_argument("--runtime-dir",type=Path,required=True)
    parser.add_argument("--node",type=Path,required=True)
    parser.add_argument("--agent-exe",type=Path,help="Optional shared generic RPC call transport; its hash and source are recorded")
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--rounds",type=int,default=10)
    parser.add_argument("--active-download",action="store_true")
    options=parser.parse_args()
    if os.name!="nt":parser.error("Requires Windows and the real WebView2 runtime")
    if not 1<=options.rounds<=100:parser.error("rounds must be in 1..100")
    for name in ("baseline_exe","candidate_exe","runtime_dir","node"):
        setattr(options,name,require_d(getattr(options,name)))
    if options.agent_exe:options.agent_exe=require_d(options.agent_exe)
    runtime=options.runtime_dir/"msedgewebview2.exe"
    if not runtime.is_file():parser.error("runtime-dir must directly contain msedgewebview2.exe")
    for executable in (options.baseline_exe,options.candidate_exe):
        if not options.agent_exe and not (executable.parent/"tools/cutflow-agent.exe").is_file():
            parser.error("Each EXE requires its corresponding tools/cutflow-agent.exe")
    baseline=provenance(options.baseline_exe)
    if baseline["version"]!="0.5.0.0":
        parser.error("This comparison requires the verified CutFlow 0.5.0 baseline, not an older installed version")
    options.output=options.output.resolve()
    if options.output.drive.lower()!="d:" or not options.output.is_relative_to(ROOT):
        parser.error("Output must stay inside the D: project directory")
    options.output.mkdir(parents=True,exist_ok=False)
    provider=FixtureProvider()
    threading.Thread(target=provider.serve_forever,daemon=True).start()
    results=[]
    try:
        for scenario in (["idle","active-download"] if options.active_download else ["idle"]):
            for label,source,version in (("baseline",options.baseline_exe,1),("candidate",options.candidate_exe,2)):
                results.append(run_one(label,source,version,scenario,options,provider))
        summary={"runtimeDirectory":str(options.runtime_dir),"runtimeSha256":digest(runtime),"runtimeFileVersion":file_version(runtime),"results":results,
                 "boundary":"Synthetic 500 sessions/1000 history; local result download; no paid Google/Bilibili request",
                 "thresholds":{"warmFirstFrameP95Ms":100,"coldFirstFrameMaxMs":250}}
        json_write(options.output/"comparison.json",summary)
        print(json.dumps({"output":str(options.output),"results":[{"label":r["label"],"scenario":r["scenario"],**r["summary"]} for r in results]}))
    finally:
        provider.cancel_media.set()
        provider.shutdown()
        provider.server_close()


if __name__=="__main__":
    main()
