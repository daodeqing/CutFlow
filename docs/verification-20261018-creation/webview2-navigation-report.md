# Windows WebView2 真实导航前后对比

日期：2026-10-18。本轮由正式 Windows EXE 与同一个固定 WebView2 runtime 运行，无本地生产编译。

|场景|版本|预热样本|预热首帧P50|预热首帧P95|冷切换最大值|
|---|---|---:|---:|---:|---:|
|idle|baseline|50|207.2 ms|212.9 ms|487.2 ms|
|idle|candidate|50|7.4 ms|20.9 ms|113.7 ms|
|active-download|baseline|50|207.2 ms|213.2 ms|323.4 ms|
|active-download|candidate|50|7.5 ms|22.0 ms|119.3 ms|

新版两种场景下的预热 P95 都小于 100 ms，冷切换最大值都小于 250 ms。每种场景 50 次预热、4 次冷切换，合计 216 次原始测量。这里是本机受控运行结果，不代表所有设备，也没有统计跨设备置信区间。

基线为 0.5.0.0，build-info 的 commit=6202e3e68d998714842b8ac79ebb2d1c367f3101、run=39030223570，EXE 和 agent 的本轮哈希均匹配。新版为云端 run=39054585273 的成功 Build hub，commit=d477ede777f7431cbfacd48e3bd6b2069b2287b5，文件版本 0.6.0.0。该次总体 workflow 因 Build editor 失败而失败，package 跳过；本测量只验证成功主窗口产物，不能称安装包或整套云构建完成。

两版引擎均为 Edg/155.0.4283.45，来自同一个 D 盘固定 runtime。msedgewebview2.exe SHA-256=46b3c6ac9169416cc2ee76b23ee3d9db0eb8daefb914b1d9cc0f5d2387b033ac。两版实际 CSS 视口均 1426×892，DPR=1.25（窗口按 1440×900 设置）；不是 1440×900 CSS 像素。新版 EXE SHA-256=4f87311e24f1095bd629e0dee02ea772e8983f35853eb2ee63b83031f707398d。

两版在不同 D 盘隔离 stage 启动，原安装程序和 portable 未修改。使用相同旧版 agent 作为通用 call RPC transport，哈希=d2dfe18eb5d214d0007a2c948809d35ae77c5481fd5aa9a639049263ed05cf69；已核对其 call 分支无方法白名单，未使用旧 schema/MCP 目录，不将该 agent 称为新版配套程序。

数据为每版 1,000 个持久化历史任务，新版另有 500 个会话、全部 1,000 轮归入最近会话以检查历史分页。旧版无项目/聊天能力，因此没有伪造其支持 500 会话。所有图片是 1×1 本机占位，不包含真实4K解码负载；该测量主要覆盖页面切换、玻璃渲染及历史状态处理。

active-download 场景为本机模拟 AI 结果下载：旧/新各仅一个 custom-image 请求及一个媒体 GET，新版能力探测共两次；未向真实 Google/B站发起生成或下载。不是 B站传输队列高并发验收。四个场景均零 pageerror。真实 Flow 创建/生成、真实账号额度及真实 B站传输仍未由本记录验证。

计时在浏览器捕获 click 时开始，用 requestAnimationFrame 检查目标页面/骨架的非透明可见元素并等待下一绘制机会；不会仅以导航 active 属性变化作为完成。该测量是用户可见首帧代理，不是操作系统逐像素呈现时间。原始点击、settled 时间、冷切换目标、版本、凭据以外的来源信息见 webview2-navigation-comparison.json。

首轮工具遇到原生控制服务晚于 CDP 就绪的启动时序，已增加只读 connection.get 就绪等待后重测。失败首轮与成功第二轮各自独立，不复用状态。测试启动的主窗口与 WebView2 子进程已关闭；隔离缓存和临时数据按清理记录回收。
