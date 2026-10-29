use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Call {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub const METHODS: &[(&str, &str)] = &[
    ("cutflow.creation.catalog", "读取轻量创作目录；refresh=true 执行不生成媒体的项目能力检查"),
    ("cutflow.creation.projects.list", "分页列出真实 Flow 项目绑定和历史未归类"),
    ("cutflow.creation.projects.create", "创建专属 Flow 项目；需要 name、preferredModel、operationId"),
    ("cutflow.creation.projects.get", "查询项目；未确认的创建仅查询原请求，不自动重建"),
    ("cutflow.creation.sessions.list", "分页列出 projectId 内的聊天"),
    ("cutflow.creation.sessions.create", "在已绑定项目内创建聊天；需要 projectId、operationId"),
    ("cutflow.creation.turns.list", "按 sessionId 分页列出生成轮次及任务；最新优先"),
    ("cutflow.creation.drafts.save", "按聊天独立保存图片或视频草稿"),
    ("cutflow.editor.open", "打开或唤起独立 Concat 剪辑器；可选 projectPath"),
    ("cutflow.editor.status", "查询剪辑器是否运行及当前工程"),
    ("cutflow.editor.focus", "唤起剪辑窗口"),
    ("cutflow.editor.recents", "列出最近剪辑工程"),
    ("cutflow.jobs.retryDownload", "只重试媒体下载，不重复提交生成；需要 jobId"),
    (
        "cutflow.project.current",
        "读取当前可见工程、document、revision 和素材 ID",
    ),
    (
        "cutflow.project.save",
        "保存当前工程；需要 projectPath、operationId、expectedRevision",
    ),
    (
        "cutflow.edit.apply",
        "在当前时间线执行原生 command 或 commands 数组，可撤销；需要 projectPath、operationId、expectedRevision",
    ),
    (
        "cutflow.edit.undo",
        "撤销一次编辑；需要 projectPath、operationId、expectedRevision",
    ),
    (
        "cutflow.edit.redo",
        "重做一次编辑；需要 projectPath、operationId、expectedRevision",
    ),
    (
        "cutflow.assets.import",
        "导入 files 本地路径数组，可选 place=true 放入时间线；需要 projectPath、operationId、expectedRevision",
    ),
    (
        "cutflow.preview",
        "定位 time 秒并返回当前工程状态，软件显示预览",
    ),
    (
        "cutflow.export.open",
        "打开当前草稿的导出确认界面；不自动导出",
    ),
    (
        "cutflow.connection.get",
        "读取 Flow2API 服务地址，不返回 API Key",
    ),
    (
        "cutflow.connection.save",
        "设置 baseUrl 和可选 apiKey；API Key 使用 DPAPI 加密",
    ),
    ("cutflow.connection.test", "使用一个文本模型发送最小测试，验证实际回复"),
    ("cutflow.models.list", "从已连接 Flow2API 刷新可用模型"),
    (
        "cutflow.generate",
        "生成素材；需要 operationId、model、prompt、kind=image/video、maxGenerationCalls>=1；projectPath 可选，省略时保存到公共素材库。可选 references、stream；通过 jobs.get 查询",
    ),
    (
        "cutflow.account.start",
        "创建 B站登录二维码，只在用户请求登录时调用",
    ),
    (
        "cutflow.account.poll",
        "用 sessionId 检查扫码确认状态；不返回 Cookie",
    ),
    ("cutflow.account.status", "查询真实 B站登录及会员状态"),
    ("cutflow.account.logout", "退出 B站账号，保留素材"),
    (
        "cutflow.bilibili.favorites",
        "读取自己的收藏夹列表，可选 page",
    ),
    ("cutflow.bilibili.parse", "用当前账号解析 url 的可用画质"),
    (
        "cutflow.bilibili.download",
        "下载指定 url；需要 projectPath、operationId、rights，默认 parts=1、quality=120、allowLower=false",
    ),
    ("cutflow.localize.run", "将本地视频与字幕、弹幕制作成英文版；需要 operationId、videoPath 和 model，完成后必须审核"),
    ("cutflow.localize.start", "从 B站链接或已完成下载制作英文版；需要 operationId、rights、model 和 url 或 taskId"),
    ("cutflow.x.compose", "打开独立 X 发布窗口；用户自行登录、上传并发布"),
    ("cutflow.localize.watch.list", "列出收藏夹和 UP 主监控及待处理视频"),
    ("cutflow.localize.watch.add", "添加已获授权的收藏夹或 UP 主监控；首次仅建立基线"),
    ("cutflow.localize.watch.remove", "停止指定来源监控并移除其待处理列表"),
    ("cutflow.localize.watch.scan", "立即检查监控来源，不绕过首次基线"),
    ("cutflow.references.list", "列出本地角色和场景参考图"),
    (
        "cutflow.references.save",
        "保存参考图；需要 kind=character/scene、name、path，可选 prompt",
    ),
    (
        "cutflow.speech.run",
        "生成本地配音；需要 projectPath、operationId、text，可选 voice=45、speed=1、modelId；结果通过 jobs.get 查询",
    ),
    (
        "cutflow.captions.run",
        "转录 file 的真实语音，返回字幕时间段；需要 projectPath、operationId，可选 modelId=base",
    ),
    (
        "cutflow.workflow.run",
        "执行 TaskSpec v1，准备混合素材、旁白字幕并作为一次可撤销操作组装草稿，默认 stopAt=draft",
    ),
    (
        "cutflow.jobs.list",
        "列出持久化任务及进度；不会重发云端请求",
    ),
    ("cutflow.jobs.get", "按 jobId 获取任务状态及已完成素材"),
    (
        "cutflow.jobs.cancel",
        "按 jobId 请求取消；停止本地等待不保证云端退款或停单",
    ),
];

pub fn tools() -> Value {
    Value::Array(METHODS.iter().map(|(method, description)| {
        let mut tool=json!({
        "name": method.replace('.', "_"), "description": description,
        "inputSchema": {"type":"object","properties":{
            "projectPath":{"type":"string"},"operationId":{"type":"string"},
            "creationProjectId":{"type":"string"},"continuationAssetPath":{"type":"string"},"projectId":{"type":"string"},"turnId":{"type":"string"},
            "title":{"type":"string"},"preferredModel":{"type":"string"},"refresh":{"type":"boolean"},
            "cursor":{"type":["string","null"]},"limit":{"type":"integer","minimum":1,"maximum":50},
            "draft":{"type":"object"},"activeOnly":{"type":"boolean"},"sinceRevision":{"type":"integer","minimum":0},
            "expectedRevision":{"type":"integer","minimum":0},"jobId":{"type":"string"},
            "url":{"type":"string"},"parts":{"type":"string"},"quality":{"type":"string"},
            "allowLower":{"type":"boolean"},"rights":{"type":"string"},
            "model":{"type":"string"},"kind":{"type":"string"},"prompt":{"type":"string"},
            "references":{"type":"array","items":{"type":"string"}},
            "maxGenerationCalls":{"type":"integer","minimum":0},"baseUrl":{"type":"string"},
            "apiKey":{"type":"string"},"sessionId":{"type":"string"},"page":{"type":"integer","minimum":1},"folderId":{"type":"integer","minimum":1},
            "name":{"type":"string"},"path":{"type":"string"},"file":{"type":"string"},
            "files":{"type":"array","items":{"type":"string"}},"place":{"type":"boolean"},
            "text":{"type":"string"},"voice":{"type":"integer"},"speed":{"type":"number"},
            "modelId":{"type":"string"},"time":{"type":"number","minimum":0},
            "n":{"type":"integer","minimum":1,"maximum":4},"image_size":{"type":"string"},
            "stream":{"type":"boolean"},"aspect_ratio":{"type":"string"},"duration":{"type":"number","exclusiveMinimum":0},
            "command":{"type":"object"},"commands":{"type":"array","items":{"type":"object"}},
            "version":{"type":"integer","const":1},"stopAt":{"type":"string","const":"draft"},
            "scenes":{"type":"array","items":{"type":"object"}},"output":{"type":"object"},
            "subtitles":{"type":"boolean"},"bgm":{"type":"string"},
            "taskId":{"type":"string"},"watchId":{"type":"string"},"videoPath":{"type":"string"},"subtitlePath":{"type":"string"},"danmakuPath":{"type":"string"},
            "sourceUrl":{"type":"string"},"subtitleModel":{"type":"string"},"danmakuModel":{"type":"string"},"postModel":{"type":"string"},
            "speechModel":{"type":"string"},"transcription":{"enum":["remote","whisper"]},"whisperModel":{"enum":["tiny","base","small"]},
            "removeBottomText":{"type":"boolean"}
        },"additionalProperties":false}
        });
        let fields=match *method {
            "cutflow.creation.catalog"=>"refresh",
            "cutflow.creation.projects.list"=>"cursor limit",
            "cutflow.creation.projects.create"=>"name preferredModel operationId",
            "cutflow.creation.projects.get"=>"projectId refresh",
            "cutflow.creation.sessions.list"=>"projectId cursor limit",
            "cutflow.creation.sessions.create"=>"projectId title operationId",
            "cutflow.creation.turns.list"=>"sessionId cursor limit",
            "cutflow.creation.drafts.save"=>"sessionId kind draft",
            "cutflow.jobs.list"=>"sessionId activeOnly sinceRevision",
            "cutflow.localize.start"=>"operationId model url taskId parts quality rights subtitleModel danmakuModel postModel speechModel transcription whisperModel removeBottomText",
            "cutflow.x.compose"=>"",
            "cutflow.localize.watch.list"|"cutflow.localize.watch.scan"=>"",
            "cutflow.localize.watch.add"=>"url model rights quality subtitleModel danmakuModel speechModel postModel transcription whisperModel removeBottomText",
            "cutflow.localize.watch.remove"=>"watchId",
            "cutflow.editor.status"|"cutflow.editor.focus"|"cutflow.editor.recents"=>"",
            "cutflow.editor.open"=>"projectPath",
            "cutflow.project.current"|"cutflow.connection.get"|"cutflow.models.list"|"cutflow.account.start"|"cutflow.account.status"|"cutflow.account.logout"|"cutflow.references.list"=>"",
            "cutflow.project.save"|"cutflow.edit.undo"|"cutflow.edit.redo"=>"projectPath operationId expectedRevision",
            "cutflow.edit.apply"=>"projectPath operationId expectedRevision command commands",
            "cutflow.assets.import"=>"projectPath operationId expectedRevision files place",
            "cutflow.preview"=>"projectPath time",
            "cutflow.export.open"=>"projectPath",
            "cutflow.connection.save"=>"baseUrl apiKey",
            "cutflow.connection.test"=>"model",
            "cutflow.generate"=>"projectPath creationProjectId sessionId turnId continuationAssetPath operationId model prompt kind references maxGenerationCalls stream aspect_ratio duration n quality image_size",
            "cutflow.account.poll"=>"sessionId",
            "cutflow.bilibili.favorites"=>"page folderId",
            "cutflow.bilibili.parse"=>"url",
            "cutflow.bilibili.download"=>"projectPath operationId url rights parts quality allowLower",
            "cutflow.references.save"=>"kind name path prompt",
            "cutflow.localize.run"=>"operationId videoPath subtitlePath danmakuPath sourceUrl title model subtitleModel danmakuModel postModel speechModel transcription whisperModel removeBottomText",
            "cutflow.speech.run"=>"projectPath operationId expectedRevision text voice speed modelId",
            "cutflow.captions.run"=>"projectPath operationId expectedRevision file modelId",
            "cutflow.workflow.run"=>"version projectPath operationId expectedRevision scenes maxGenerationCalls voice subtitles stopAt output bgm",
            _=>"jobId",
        };
        let required=match *method {
            "cutflow.creation.projects.create"=>"name preferredModel operationId",
            "cutflow.creation.projects.get"=>"projectId",
            "cutflow.creation.sessions.list"=>"projectId",
            "cutflow.creation.sessions.create"=>"projectId operationId",
            "cutflow.creation.turns.list"=>"sessionId",
            "cutflow.creation.drafts.save"=>"sessionId kind draft",
            "cutflow.edit.apply"|"cutflow.project.save"|"cutflow.edit.undo"|"cutflow.edit.redo"=>"projectPath operationId expectedRevision",
            "cutflow.assets.import"=>"projectPath operationId expectedRevision files",
            "cutflow.preview"|"cutflow.export.open"=>"projectPath",
            "cutflow.connection.save"=>"baseUrl",
            "cutflow.connection.test"=>"model",
            "cutflow.generate"=>"operationId model prompt kind maxGenerationCalls",
            "cutflow.account.poll"=>"sessionId",
            "cutflow.bilibili.parse"=>"url",
            "cutflow.bilibili.download"=>"projectPath operationId url rights",
            "cutflow.references.save"=>"kind name path",
            "cutflow.localize.run"=>"operationId videoPath model",
            "cutflow.localize.start"=>"operationId model rights",
            "cutflow.localize.watch.add"=>"url model rights",
            "cutflow.localize.watch.remove"=>"watchId",
            "cutflow.speech.run"=>"projectPath operationId expectedRevision text",
            "cutflow.captions.run"=>"projectPath operationId expectedRevision file",
            "cutflow.workflow.run"=>"version projectPath operationId expectedRevision scenes",
            "cutflow.jobs.get"|"cutflow.jobs.cancel"|"cutflow.jobs.retryDownload"=>"jobId",
            _=>"",
        };
        tool["inputSchema"]["properties"].as_object_mut().unwrap().retain(|name,_| fields.split_whitespace().any(|field|field==name));
        tool["inputSchema"]["required"]=json!(required.split_whitespace().collect::<Vec<_>>());
        if *method=="cutflow.workflow.run" {
            tool["inputSchema"]["properties"]["scenes"]=json!({"type":"array","minItems":1,"maxItems":64,"items":{
                "type":"object","required":["id","source"],"additionalProperties":false,"properties":{
                    "id":{"type":"string","minLength":1},"narration":{"type":"string"},"duration":{"type":"number","exclusiveMinimum":0},"sourceStart":{"type":"number","minimum":0},
                    "source":{"oneOf":[
                        {"type":"object","required":["kind","path"],"additionalProperties":false,"properties":{"kind":{"const":"local"},"path":{"type":"string"}}},
                        {"type":"object","required":["kind","model","prompt","mediaKind"],"additionalProperties":false,"properties":{"kind":{"const":"generate"},"model":{"type":"string"},"prompt":{"type":"string"},"mediaKind":{"enum":["image","video"]},"references":{"type":"array","items":{"type":"string"}}}},
                        {"type":"object","required":["kind","url","rights"],"additionalProperties":false,"properties":{"kind":{"const":"bilibili"},"url":{"type":"string"},"rights":{"type":"string"},"parts":{"type":"string"},"quality":{"type":"string"}}}
                    ]}
                }
            }});
        }
        tool
    }).collect())
}
