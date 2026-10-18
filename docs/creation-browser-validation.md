# CutFlow 0.6.0 创作页浏览器验收

2026-10-18，在本机 Chromium headless shell、Vite 开发服务器及模拟 Tauri 接口中验收。测试没有调用真实图片或视频生成服务。

- 16 个浏览器用例均有通过证据。最近完整套件为 15 通过、1 失败；缓存工作区阅读位置修复后，相关 2 项回归全部通过。没有把这两次运行写成一次 16/16。
- 覆盖浅深主题的 1440×900、1024×900 三栏，390×844 项目与参数抽屉，项目折叠，项目与会话创建、迟到回包隔离、连续改图、参数快照、隐藏窗口暂停读取及历史阅读位置。
- 压力夹具包含 500 个聊天、1,000 条生成历史；每页读取最多 50 条，聊天列表和生成记录按可见窗口渲染，生成轮次 DOM 不超过 20 个。
- 同设备预热后 30 次侧栏切换：中位数 42.8 ms，P95 76.1 ms，最大 76.6 ms。计时从 DOM 点击派发开始，到目标页无加载提示并经过两次绘制回调；不包含驱动传输及点击可操作性等待。
- 人工注入 500 ms 创作模块延迟，点击后 1.9 ms 出现加载 DOM。参考截图 9 张已逐张检查；测试未捕获页面脚本异常。
- 版本一致性检查通过：前端、Tauri、workbench、对应 Cargo 锁条目、构建产物和 UI 均为 0.6.0；编辑引擎和便携数据结构继续使用各自版本。

证据保留在本地 docs/verification-20261018-creation/：browser-summary.json、browser-verification.txt、browser-cache-repair.txt、navigation-performance.json、cold-navigation.json 及截图。浏览器用例与可复用夹具位于 tests/visual/creation-studio.spec.ts 和 creation-studio.mock.ts。

上述结果仅证明模拟接口下的浏览器交互与布局；没有代替原生 Windows WebView2 性能、真实 Google Flow 项目/账号绑定或真实生成质量验收。没有同条件修改前性能基线，不能据此声称提速百分比。
