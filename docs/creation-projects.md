# 项目式 AI 创作与验收边界

AI 创作采用项目、聊天、生成轮次三层，左右栏与历史独立滚动。侧栏取消串行退场/入场，全屏折射只在演示弹窗挂载。预热导航首帧 P95 ≤100 ms、冷切换占位 ≤250 ms 是验收目标；源码测试通过不等于实机达到目标。

## 接口、迁移与目录

cutflow.creation.catalog 默认只读本地连接、模型、参考库与能力缓存，不传历史；refresh:true 执行无媒体生成的项目能力检查，失败在 fault 中显示。projects.*、sessions.*、turns.list、drafts.save 分别管理项目、聊天、轮次和草稿。列表默认及最大每页 50 条，游标为上一页末项 ID；项目/聊天按更新时间倒序，轮次接口最新优先，界面按生成顺序显示，时间为 Unix 毫秒。

cutflow.generate 增加 creationProjectId、sessionId、turnId，并继续以 operationId 幂等。同一聊天最多运行一轮，跨聊天受全局四任务上限约束；提交时固定归属、参数快照与 Flow 上下文。jobs.list 支持 sessionId、activeOnly、sinceRevision，增量包含终态；查询复制所需任务后释放锁，磁盘检查在锁外。无筛选调用与无项目生成兼容旧工作流，新界面不使用无项目路径。

workbench.json 从 v1 升至 v2 前，将原始字节备份至 portable/backups/workbench.v1.json。旧作品路径不变，迁入只读“历史未归类”，不推测云端归属。新素材保存至 portable/generated/<本地项目ID>/<聊天ID>/<任务ID>/，逐图生成缩略图，原图仅在用户打开时加载。旧素材缺少缩略图时展示占位，不批量改写。未知版本/损坏存储阻止覆盖。

## Flow 绑定与连续改图

gflow-project-control 使用普通生成 API Key 调用 /v1/chat/completions，stream:false，单条 user 消息内放版本 1 JSON。capabilities 无付费；projects.create 携带 workspaceId、本地 projectId、operationId、名称和首选模型；projects.get 沿用本地 UUID 和原 operationId，可附真实 flowProjectId 诊断。标准 completion 的 content 内返回控制 JSON，业务错误为 {version:1,error:{code,message,status_code}}；本地保留 code/message。

创建前先落盘，再由插件自动选择可用账号并调用真实 Flow 创建 RPC。确认后保存真实 Flow 项目、账号显示名、服务实例与固定账号参数目录。生成的 gflow_project_id 必须为真实 Flow UUID，另传 gflow_workspace_id、gflow_instance_id。插件校验账号身份、启用开关、档位、验证路由与并发，刷新后再次校验；不会退回其他账号或项目池。额度是否足够以 Google 实际响应为准，目录检查不能替代额度验证。

超时/中断后标记“创建结果待核查”，查询原记录而不自动重建；服务重启保留幂等记录，未确认创建不重发。服务实例变化禁止沿用旧绑定。专属项目 purpose=cutflow，原轮询池只枚举 pool。工作区 UUID 是分组标识，不构成额外多租户认证。

连续改图只针对图片。显式引用当前聊天选中的已生成图片；未手选时默认最近一轮第一张。自动引用与手动参考去重，超过模型上限阻止提交。continuationAssetPath 必须属于当前聊天历史图片且明确含于本轮 references，后端重新验证。独立生成可移除自动引用，不同聊天不自动借用作品。图片/视频草稿独立，修改右栏只影响下一轮。视频保留原生参考与参数流程，本次不承诺视频续写或文字聊天。

## 已执行验证

- 工作台 Rust 47/47：4 个单元、27 个原契约、16 个项目契约。覆盖原始备份/资产路径、重启、幂等/unknown、固定绑定/实例变化、同聊天运行锁、连续引用归属、固定账号能力、仅下载重试及结构化控制错误。
- 前端三个创作回归文件 31/31：11 个 store、12 个生命周期、8 个工作区。覆盖迟到列表/revision、跨会话回执、草稿保留、请求合并、停用/隐藏停止轮询、停止后部分作品、按需原图、模型/引用校验。Editor 工程切换导入测试逐字保留。
- 大历史模拟数据为 500 个聊天、1,000 个历史任务。测试发现并修正了旧列表迟到覆盖新轮次的问题。
- 上述均使用模拟数据和本机 loopback 服务，无真实凭据，未创建真实 Google Flow 项目、未发起付费图片或视频生成。插件套件、完整前端、GitHub 构建和真实 WebView2 性能在总验收记录分别报告，不能用本节数字替代。

## 同设备旧/新 WebView2 对比方案

现有 windows-webview-smoke.mjs 只检查启动渲染，9222 依赖 smoke 配置；check_hub.py 验证原生 RPC 与模拟媒体，没有导航计时。新验收工具 tests/check_creation_navigation.py 使用用户指定同一 WebView2 runtime，通过本机 CDP 附加旧/新真实应用；不重新编译。

旧程序只读来源为 D:/LUO/video editing/dist/CutFlow/CutFlow.exe，新程序来自 GitHub hub 构建产物。两者分别复制到项目 .build 的隔离目录，各配自己的 agent；不接触原 portable。旧版预置 v1 的 1,000 个任务，新版预置 v2 的同量任务及 500 个聊天。它比较相同历史负载，不声称旧版具备聊天功能。

顺序测试冷切换、预热连续切换及可选本机媒体下载压力。计时从真实 click 到目标内容/占位首次可见的绘制机会，不能把导航激活状态当作页面绘制完成。输出原始延时、P50/P95、版本、视口、EXE/运行时哈希和本机 provider 请求计数；活动压力是 AI 结果下载，不标记为真实 B站下载。

运行前必须确认旧/新各自的 tools/cutflow-agent.exe 齐全，输出目录在 D 盘项目内且不存在；所有运行时/缓存/日志重定向到隔离目录。只关闭工具自己创建的进程。隔离 stage、日志和用户数据在复核后统一移至回收站并记录大小；正式可复用脚本保留。工具编写、静态检查与真实执行须在总记录分别标注；真实执行等待新云端产物。
