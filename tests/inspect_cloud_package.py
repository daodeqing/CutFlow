"""Read cloud ZIP manifests with bounded HTTP ranges; never download a full installer."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import struct
import subprocess
import zipfile
import requests


class Ranges(io.RawIOBase):
    def __init__(self, url, size, budget=16*1024*1024):
        self.url, self.size, self.position = url, size, 0
        self.budget = budget
        self.cache = {}
        self.transferred = 0

    def seekable(self):
        return True

    def seek(self, offset, whence=0):
        self.position = offset if whence == 0 else self.position + offset if whence == 1 else self.size + offset
        if self.position < 0:
            raise ValueError("Negative ZIP offset")
        return self.position

    def tell(self):
        return self.position

    def read(self, count=-1):
        count = self.size-self.position if count < 0 else min(count, self.size-self.position)
        if count > 8*1024*1024:
            raise ValueError("Refusing an unbounded artifact read")
        result = bytearray()
        while count > 0:
            start = self.position // 524288 * 524288
            if start not in self.cache:
                end = min(self.size-1, start+524287)
                if self.transferred + end-start+1 > self.budget:
                    raise ValueError("Artifact inspection exceeded its bounded transfer budget")
                response = requests.get(self.url, headers={"Range": f"bytes={start}-{end}"}, stream=True, timeout=90)
                if response.status_code != 206:
                    response.close()
                    raise ValueError(f"Server did not honor bounded range: HTTP {response.status_code}")
                value = response.content
                response.close()
                if len(value) != end-start+1:
                    raise ValueError("Truncated ZIP range")
                self.cache[start] = value
                self.transferred += len(value)
            block = self.cache[start]
            offset = self.position-start
            take = min(count,len(block)-offset)
            result.extend(block[offset:offset+take])
            self.position += take
            count -= take
        return bytes(result)


class Slice(io.RawIOBase):
    def __init__(self, source, offset, size):
        self.source,self.offset,self.size,self.position=source,offset,size,0

    def seekable(self):
        return True

    def seek(self, offset, whence=0):
        self.position=offset if whence==0 else self.position+offset if whence==1 else self.size+offset
        return self.position

    def tell(self):
        return self.position

    def read(self,count=-1):
        count=self.size-self.position if count<0 else min(count,self.size-self.position)
        self.source.seek(self.offset+self.position)
        value=self.source.read(count)
        self.position+=len(value)
        return value


def inspect(repo, artifact_id, gh, verify_exe=None):
    token = subprocess.check_output([str(gh),"auth","token"],text=True).strip()
    response=requests.get(f"https://api.github.com/repos/{repo}/actions/artifacts/{artifact_id}",
                          headers={"Authorization":"Bearer "+token},timeout=45)
    response.raise_for_status()
    metadata=response.json()
    response=requests.get(f"https://api.github.com/repos/{repo}/actions/artifacts/{artifact_id}/zip",
                          headers={"Authorization":"Bearer "+token},allow_redirects=False,timeout=45)
    if response.status_code != 302:
        raise ValueError(f"Artifact redirect failed: HTTP {response.status_code}")
    source=Ranges(response.headers["Location"],metadata["size_in_bytes"],64*1024*1024 if verify_exe else 16*1024*1024)
    outer=zipfile.ZipFile(source)
    entries=outer.infolist()
    delivery=next(value for value in entries if value.filename.endswith("-windows-x64.zip"))
    setup=next(value for value in entries if value.filename.endswith("-setup.exe"))
    if delivery.compress_type != zipfile.ZIP_STORED:
        raise ValueError("Cloud artifact must store the portable ZIP without outer compression")
    checksums={value.filename:outer.read(value).decode("utf-8").strip() for value in entries if value.filename.endswith(".sha256")}
    source.seek(delivery.header_offset)
    header=source.read(30)
    name_length,extra_length=struct.unpack_from("<HH",header,26)
    nested=zipfile.ZipFile(Slice(source,delivery.header_offset+30+name_length+extra_length,delivery.file_size))
    root=delivery.filename[:-4]+"/"
    build_info=json.loads(nested.read(root+"build-info.json"))
    required=["CutFlow.exe","CutFlow-editor.exe","tools/cutflow-agent.exe","tools/bdl.exe","tools/ffmpeg.exe","tools/ffprobe.exe","WebView2/msedgewebview2.exe"]
    names=set(nested.namelist())
    missing=[value for value in required if root+value not in names]
    portable=[value for value in names if value.startswith(root+"portable/") and not value.endswith("/")]
    if missing or portable:
        raise ValueError("Incomplete package or bundled user state")
    executable_check=None
    if verify_exe:
        verify_exe.parent.mkdir(parents=True,exist_ok=True)
        digest=hashlib.sha256()
        with nested.open(root+"CutFlow.exe") as stream, verify_exe.open("xb") as output:
            while block:=stream.read(1024*1024):
                output.write(block)
                digest.update(block)
        assert digest.hexdigest()==build_info["files"]["CutFlow.exe"]["sha256"]
        executable_check={"path":str(verify_exe),"sha256":digest.hexdigest(),"bytes":verify_exe.stat().st_size,"matchedManifest":True}
    guide="项目与聊天.md"
    guide_present=root+guide in names
    guide_version="CutFlow 0.6" in nested.read(root+"创作与外部AI.md").decode("utf-8")
    return {"repo":repo,"artifactId":artifact_id,"artifactName":metadata["name"],"artifactBytes":metadata["size_in_bytes"],
            "cloudRun":metadata.get("workflow_run"),"entries":[{"name":value.filename,"bytes":value.file_size} for value in entries],
            "checksums":checksums,"buildInfo":build_info,"requiredFilesPresent":not missing,"portableEmpty":not portable,
            "projectGuidePresent":guide_present,"usageGuideVersion06":guide_version,
            "rangeBytesTransferred":source.transferred,"executableCheck":executable_check,"boundary":"ZIP directory and small-file CRC checked; large deliverable SHA files read, not independently recomputed."}


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo",default="daodeqing/CutFlow")
    parser.add_argument("--artifact",type=int,required=True)
    parser.add_argument("--gh",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--verify-exe",type=Path)
    args=parser.parse_args()
    root=Path(__file__).resolve().parents[1]
    output=args.output.resolve()
    if not output.is_relative_to(root) or output.drive.lower()!="d:":
        parser.error("Inspection report must stay inside this D: project")
    if args.verify_exe:
        args.verify_exe=args.verify_exe.resolve()
        if not args.verify_exe.is_relative_to(root) or args.verify_exe.exists():
            parser.error("Validation executable must be a new file inside this project")
    try:
        result=inspect(args.repo,args.artifact,args.gh,args.verify_exe)
    except requests.RequestException as error:
        raise SystemExit(type(error).__name__+" while reading cloud artifact") from None
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"artifactId":args.artifact,"run":result["buildInfo"]["cloud_run"],"version":result["buildInfo"]["version"],
                      "portableEmpty":result["portableEmpty"],"rangeBytesTransferred":result["rangeBytesTransferred"]}))
