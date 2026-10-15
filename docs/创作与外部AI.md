# CutFlow 0.3 创作工作台

打开 CutFlow，新建或打开工程。顶部有「素材」「AI 创作」「剪辑」。剪辑检查器默认显示日常操作，「进阶」展开特效、增强和调色，旧工程中的高级效果继续参与预览与导出。

## B站素材

在「AI 创作 → B站账号」扫码，等待手机确认。程序自动检查扫码状态，也可以手动检查或刷新二维码。账号名和大会员状态来自实际账号查询。查询失败显示未知，不把登录等同于大会员。Cookie 导入保留在素材下载的进阶设置。

「我的收藏夹」显示收藏夹 ID。输入 ID 和页码后「读取内容」，复制其中视频地址到「素材」。支持分 P 和画质选择。严格 4K 下载不会静默降档，最高可用模式需要明确选择。会员身份不保证每条视频都有 4K。完成后仍检查实际尺寸、音视频轨和文件完整性。请确认素材的使用权限。

## 图片与视频

在「连接与外部 AI」设置已有 Flow2API 的地址和 API Key，保存后测试连接。地址支持服务根目录或 `/v1`。密钥本机 DPAPI 加密，不写入工程或命令行参数。首次配置只连接现有服务，软件不部署 Google 网关。

模型来自 `GET /v1/models`，可筛选图片、文生视频、图生/参考图视频、比例和时长，再选择完整模型 ID，也支持手动填写。能力提示由模型名推断；服务未在列表中提供完整能力协议，因此不能保证任意兼容模型都支持相同参数。比例和时长通过选定的具体模型变体确定。

填写提示词，选择参考图后生成一次。请求采用 `POST /v1/chat/completions`，`messages` 包含文本及图片 data URL，`stream=true`。流式结果的 Markdown 图片或 HTML video 地址下载到当前工程，探测通过后才能加入素材库或时间线。结果地址另保存于任务素材目录的 `generation-result.json`；下载失败可先核查这个记录，避免重新付费生成。

「角色与场景」保存命名参考图和提示词，选中后复用。参考图辅助一致性，不保证外观完全一致。每次生成只上传选定的图片。Flow2API 服务自身的账号、额度、重试策略和可用性也影响结果；CutFlow 的生成次数上限限制客户端提交次数，不能约束服务内部重试或承诺退款。

任务状态为 `result_unknown` 时先核查服务端记录。客户端取消只停止本地等待，后端可能继续生成并计费。软件重启将未完成任务标记中断或结果待核查，不自动重新提交。

## 外部 AI：MCP 和命令行

先独立启动 CutFlow，并在「连接与外部 AI」启用外部控制。服务只监听回环地址，每次请求验证本机 DPAPI 令牌。MCP 桥接程序和 AI 客户端退出不会关闭 CutFlow 或取消已提交任务。控制默认关闭。

MCP 客户端配置中的 `command` 指向完整的 `tools/cutflow-agent.exe`，`args` 为 `["mcp"]`；放在其他位置时追加 `--portable` 和该 CutFlow 的 portable 完整路径。命令行与 MCP 共用相同的方法及 JSON 参数：

```text
tools\cutflow-agent.exe schema
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

配置、模型、日志、令牌、参考库、任务记录在程序旁 `portable`；B站凭据在 `portable/localappdata/CutFlow`。工程媒体在工程 `assets` 或 `audio` 下。DPAPI 文件只供原 Windows 用户解密，跨电脑使用需重新配置账号与密钥。

自动测试使用本地模拟 HTTP 响应验证协议、断流、失败、取消、任务恢复与幂等，不能替代真实 B站会员下载或 Flow 付费生成。真实扫码、权限、最终导出的分辨率/音轨/切点与中文音质须在配置服务后验证。不要把源码编译通过等同于这些实测已通过。
