# CutFlow 0.5 素材、剪辑与 AI 创作

打开 CutFlow，左侧有「B站素材」「剪辑」「AI 创作」。素材下载和 AI 生成无需工程。「剪辑」打开完整的独立 Concat 编辑器，检查器默认显示日常操作，「进阶」展开特效、增强和调色。

## B站素材

在主窗口右上角账号菜单扫码，等待手机确认。程序使用 BDL 原有二维码轮询、刷新、登录检查和退出流程。账号名和大会员状态来自实际账号查询。Cookie 导入保留在登录的进阶选项。界面和 MCP 共用这套账号；不会读取独立 BDL 的系统凭据。

「我的收藏与订阅」使用原有收藏夹浏览界面；「链接解析 / 多 P」选择分 P 和画质；「下载队列」管理暂停、重试、地址刷新与输出文件。严格 4K 选择「4K / 120」及「阻止创建任务」，最高可用选择「最优画质」。实际输出验证长边至少 3840，并检查要求的音视频轨和有效时长。会员身份不保证每条视频都有 4K。

## 图片与视频

在「设置 → AI 服务」设置已有 Flow2API 或其他兼容服务的地址和 API Key，保存后检查连接。地址支持服务根目录或 `/v1`。密钥本机 DPAPI 加密，不写入工程或命令行参数。软件连接已有服务。

模型来自 `GET /v1/models`，创作页只显示 GFlow 模型，按图片／视频类别筛选并优先显示易读名称。手填 GFlow ID 会标记能力未确认，失效的列表模型需重新选择。比例、分辨率、参考图上限和视频时长受服务实际能力限制。

创作页上方预览选中作品，下方固定参数、提示词和生成按钮，支持 1024×680 逻辑像素。多张结果通过缩略图切换；历史作品、参考库、流式设置和错误详情放在浮层。图片和视频分别保留本次打开期间的草稿。主界面有运行中的生成时不能再新建，停止后等待任务终态；已保存的部分结果保留。

填写提示词，选择参考图后生成一次。请求采用 `POST /v1/chat/completions`，`messages` 包含文本及图片 data URL，支持流式和非流式。识别 Markdown 图片、HTML 媒体、普通媒体地址及结构化内容数组，下载验证后预览。未指定工程时保存在 `portable/generated`；有 `projectPath` 时保持工程保存行为。结果地址保存在任务目录 `generation-result.json`；「只重试下载结果」只取回媒体，不重新提交生成。第三方媒体下载不携带 API Key。

下载或生成完成后点击「加入剪辑」，到「剪辑」页确认目标工程。素材复制到工程内并作为一次可撤销操作加入素材库；默认不插入时间线。确认后若工程切换或修订变化，拒绝导入并保留待导入列表，用户刷新并重新确认。

「角色与场景」保存命名参考图和提示词，选中后复用。参考图辅助一致性，不保证外观完全一致。每次生成只上传选定的图片。Flow2API 服务自身的账号、额度、重试策略和可用性也影响结果；CutFlow 的生成次数上限限制客户端提交次数，不能约束服务内部重试或承诺退款。

任务状态为 `result_unknown` 时先核查服务端记录。客户端取消只停止本地等待，后端可能继续生成并计费。软件重启将未完成任务标记中断或结果待核查，不自动重新提交。

## 外部 AI：MCP 和命令行

先独立启动 CutFlow，程序提供仅回环地址的令牌控制通道，每次请求验证本机 DPAPI 令牌。主窗口和编辑器分别持有状态及锁；MCP 桥接程序和 AI 客户端退出不会关闭软件或取消已提交任务。主窗口关闭后仍可连接已运行的剪辑器；生成和 B站功能需要主窗口。

MCP 客户端配置中的 `command` 指向完整的 `tools/cutflow-agent.exe`，`args` 为 `["mcp"]`；放在其他位置时追加 `--portable` 和该 CutFlow 的 portable 完整路径。命令行与 MCP 共用相同的方法及 JSON 参数：

```text
tools\cutflow-agent.exe schema
tools\cutflow-agent.exe call cutflow.editor.open
tools\cutflow-agent.exe call cutflow.editor.status
tools\cutflow-agent.exe call cutflow.project.current
tools\cutflow-agent.exe call cutflow.workflow.run --params examples\draft.json
tools\cutflow-agent.exe call cutflow.jobs.get --stdin
```

`schema` 输出每个工具的参数定义。MCP 工具名将点换为下划线，如 `cutflow_project_current`。CLI 成功输出 JSON，错误输出 `{code,message}` 到 stderr 并返回非零退出码。常见稳定错误码：`revision_conflict`、`project_changed`、`operation_conflict`、`result_unknown`、`login_required`、`insufficient_credit`、`rate_limited`、`insufficient_material`、`cancelled`。

先读取 `cutflow.project.current`，获得当前工程路径、revision、document、素材与片段 ID。所有修改请求携带唯一 `operationId` 和读取到的 `expectedRevision`；重发同一个操作得到原结果，不重复插入。相同 ID 配不同请求返回冲突。人工修改、撤销、切换工程后，请重新读取状态。任务准备期间可以编辑，最终提交会检查 revision，冲突时保留已完成素材供新草稿复用。

`cutflow.edit.apply` 接受原生 `command` 或 `commands`，由窗口统一命令通道执行，可撤销；先在工程副本验证，批量作为一次编辑提交。`cutflow.assets.import` 接受本地文件路径数组和 `place`，后台复制到工程后导入。`cutflow.preview` 定位秒数，`cutflow.export.open` 打开人工导出确认界面。

## TaskSpec v1

安装包 `examples/draft.json` 是可修改示例。替换路径，并用当前工程真实 revision 更新 `expectedRevision`，每份新请求使用新的 `operationId`。`stopAt` 固定为 `draft`。`output` 使用 `width`、`height`、`rateNum`、`rateDen`；时间单位是秒，并对齐工程帧率。

分镜 `source.kind` 支持：

- `local`：`path` 本地媒体。
- `bilibili`：`url`、`rights` 使用权限说明，可选 `parts` 和 `quality`，默认严格 4K。
- `generate`：`model`、`prompt`、`mediaKind=image/video`、可选 `references` 路径数组；每个生成分镜占一次 `maxGenerationCalls`。

每个分镜有唯一 `id`、可选 `narration`、`duration` 和 `sourceStart`。默认中文 Kokoro 音色 45；首次使用下载多语言模型。旁白分句生成，以实际探测时长安排镜头与字幕。视频短于旁白会明确报错，图片可以持续显示。可选 `bgm` 使用完整配乐，不自动循环填充。

`cutflow.speech.run` 单独生成配音；`cutflow.captions.run` 以 Whisper 转录真实音轨，返回时间段。模型按需下载到 portable。草稿保留可编辑片段，检查后在窗口中导出。

## 文件位置与验收边界

配置、模型、日志、令牌、参考库、任务记录在程序旁 `portable`；BDL 数据库在 `portable/data`，凭据在 `portable/credentials`。工程媒体在工程 `assets` 或 `audio` 下。关闭有任务的主窗口默认转入托盘，托盘菜单明确退出才停止本地任务；编辑器独立运行。DPAPI 文件只供原 Windows 用户解密，跨电脑使用需重新配置。

自动测试使用本地模拟 HTTP 响应验证协议、断流、失败、取消、任务恢复与幂等，不能替代真实 B站会员下载或 Flow 付费生成。真实扫码、权限、最终导出的分辨率/音轨/切点与中文音质须在配置服务后验证。不要把源码编译通过等同于这些实测已通过。
