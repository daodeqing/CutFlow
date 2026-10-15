use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Call {
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub const METHODS: &[(&str, &str)] = &[
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
    ("cutflow.models.list", "从已连接 Flow2API 刷新可用模型"),
    (
        "cutflow.generate",
        "生成素材；需要 projectPath、operationId、model、prompt、kind=image/video、maxGenerationCalls>=1，可选 references 本地图片路径数组；结果通过 jobs.get 查询",
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
            "command":{"type":"object"},"commands":{"type":"array","items":{"type":"object"}},
            "version":{"type":"integer","const":1},"stopAt":{"type":"string","const":"draft"},
            "scenes":{"type":"array","items":{"type":"object"}},"output":{"type":"object"},
            "subtitles":{"type":"boolean"},"bgm":{"type":"string"}
        },"additionalProperties":false}
        });
        let fields=match *method {
            "cutflow.project.current"|"cutflow.connection.get"|"cutflow.models.list"|"cutflow.account.start"|"cutflow.account.status"|"cutflow.account.logout"|"cutflow.references.list"|"cutflow.jobs.list"=>"",
            "cutflow.project.save"|"cutflow.edit.undo"|"cutflow.edit.redo"=>"projectPath operationId expectedRevision",
            "cutflow.edit.apply"=>"projectPath operationId expectedRevision command commands",
            "cutflow.assets.import"=>"projectPath operationId expectedRevision files place",
            "cutflow.preview"=>"projectPath time",
            "cutflow.export.open"=>"projectPath",
            "cutflow.connection.save"=>"baseUrl apiKey",
            "cutflow.generate"=>"projectPath operationId model prompt kind references maxGenerationCalls",
            "cutflow.account.poll"=>"sessionId",
            "cutflow.bilibili.favorites"=>"page folderId",
            "cutflow.bilibili.parse"=>"url",
            "cutflow.bilibili.download"=>"projectPath operationId url rights parts quality allowLower",
            "cutflow.references.save"=>"kind name path prompt",
            "cutflow.speech.run"=>"projectPath operationId expectedRevision text voice speed modelId",
            "cutflow.captions.run"=>"projectPath operationId expectedRevision file modelId",
            "cutflow.workflow.run"=>"version projectPath operationId expectedRevision scenes maxGenerationCalls voice subtitles stopAt output bgm",
            _=>"jobId",
        };
        let required=match *method {
            "cutflow.edit.apply"|"cutflow.project.save"|"cutflow.edit.undo"|"cutflow.edit.redo"=>"projectPath operationId expectedRevision",
            "cutflow.assets.import"=>"projectPath operationId expectedRevision files",
            "cutflow.preview"|"cutflow.export.open"=>"projectPath",
            "cutflow.connection.save"=>"baseUrl",
            "cutflow.generate"=>"projectPath operationId model prompt kind maxGenerationCalls",
            "cutflow.account.poll"=>"sessionId",
            "cutflow.bilibili.parse"=>"url",
            "cutflow.bilibili.download"=>"projectPath operationId url rights",
            "cutflow.references.save"=>"kind name path",
            "cutflow.speech.run"=>"projectPath operationId expectedRevision text",
            "cutflow.captions.run"=>"projectPath operationId expectedRevision file",
            "cutflow.workflow.run"=>"version projectPath operationId expectedRevision scenes",
            "cutflow.jobs.get"|"cutflow.jobs.cancel"=>"jobId",
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
