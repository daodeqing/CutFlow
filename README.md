# CutFlow Windows 视频剪辑工作台

CutFlow 0.5 以 BDL 原有 Vue/Tauri 界面提供 B站账号登录与下载，独立打开完整 Concat 剪辑窗口，并提供兼容 OpenAI Chat Completions 的图片、视频创作页。当前验证状态见 [验收与已知限制](docs/验收与已知限制.md)，使用步骤及 MCP / CLI 参数见 [创作与外部 AI](docs/创作与外部AI.md)。

## 使用

1. 从[最新发行版](https://github.com/daodeqing/CutFlow/releases/latest)下载 Setup 安装版或便携 ZIP，选择一种即可。便携版完整解压后启动 `CutFlow.exe`。EXE、DLL、tools 和 WebView2 必须保持完整。
2. 「B站素材」复用 BDL 的账号菜单、扫码登录、链接解析、多 P、收藏夹和下载队列。完成下载后，从任务的更多操作中选择「加入剪辑」。
3. 在「设置 → AI 服务」填写已有兼容服务地址和密钥，检查连接后进入「AI 创作」，选择或手填模型 ID。无需先创建工程，结果可预览、打开本地文件或加入剪辑。比例和时长由模型变体或服务决定。
4. 默认突出日常剪辑；检查器「进阶」展开特效、增强和调色。高级效果与旧工程格式保留。
5. 「剪辑」打开独立的 `CutFlow-editor.exe`。新建或选择工程后，确认待导入素材的目标工程，再导入素材库。默认不插入时间线。外部 AI 通过 `tools/cutflow-agent.exe mcp` 或 `call` 操作当前工程，草稿默认停在人工检查和导出确认。

没有可用硬件显卡时自动使用软件渲染。遇到显卡驱动相关启动错误，也可在启动前设置环境变量 `CUTFLOW_RENDERER=software`；此模式的预览速度受 CPU 性能影响。

B站会员不保证每条视频都有 4K。严格模式不静默降档，并检查下载文件实际尺寸和音视频轨。取消云端生成不保证停止后台计费；结果待核查的任务不会自动重新提交。

## 数据

配置、模型、日志、任务、参考库和加密凭据均在软件旁的 `portable`。BDL 数据库在 `portable/data`，下载在 `portable/downloads`，独立生成在 `portable/generated`，编辑控制状态在 `portable/editor`，WebView2 用户数据在 `portable/webview`。工程媒体随工程保存在 `assets` / `audio`。关闭有任务的主窗口会转入托盘；明确退出才停止本地任务。主窗口和编辑器可以分别关闭。DPAPI 凭据跨 Windows 用户或电脑需要重新设置。

## Windows 构建

需要 Windows x64、Python 3.12+、Rustup、MSVC 2022 Build Tools（C++ 桌面工具与 Windows SDK）、CMake。可选 Inno Setup 6 生成安装包。已有工具可以只读复用，Rust 工具链、Cargo 包、原生依赖、临时文件和编译缓存统一写入工程 `.build`。

```text
python -B -X utf8 scripts/build_windows.py --installer
```

不需要安装包时省略 `--installer`。旧的 PowerShell 7 入口 `scripts/Build-Windows.ps1` 仅转发给同一 Python 构建器。首次构建要下载较多依赖；构建器验证上游锁定的 FFmpeg、ONNX 和 sherpa SHA256，libclang 使用指定版本的 Python wheel。生成包未签名。

成功输出为 `dist/CutFlow-0.5.0-windows-x64.zip`、SHA256 和可选 Setup；包含主窗口、独立编辑器、BDL、FFmpeg、ffprobe、cutflow-agent、固定 WebView2、DLL、说明、TaskSpec 示例和许可证。WebView2 从微软官方下载并验证锁定 SHA256，随包运行，不安装到系统。打包拒绝覆盖现有同名发行目录。

GitHub Actions 在 main 推送后自动构建，也支持手动运行；只有构建及启动检查成功才上传产物。真实账号权限、付费生成、最终音画效果需要单独实测，模拟测试不能替代。

## 开发与测试

```text
python -B -X utf8 scripts/dev.py -- cargo test --locked --manifest-path workbench/Cargo.toml
python -B -X utf8 scripts/dev.py -- cargo test --locked --manifest-path downloader/Cargo.toml
python -B -X utf8 scripts/dev.py -- cargo check --locked --manifest-path tests/ui-compile/Cargo.toml
python -B -X utf8 tests/check_callback_wiring.py
```

`workbench` 提供本地控制、任务与生成协议；`reference/bdl/apps/desktop` 是主窗口，BDL 原生状态是唯一账号与下载来源；`editor` 保留完整原生编辑引擎。`downloader` 保留旧协议兼容与共享凭据工具。上游固定提交见 `upstreams.lock.json`，本地 BDL 源码快照见 `docs/bdl-source-snapshot.json`。

参考项目：[Concat](https://github.com/jub0t/concat)、[BDL](https://github.com/Yuelioi/bdl)、[Flow2API](https://github.com/TheSmallHanCat/flow2api)、[MoneyPrinterTurbo](https://github.com/harry0703/MoneyPrinterTurbo)。阶段式草稿流程参考 MoneyPrinterTurbo，未复制其代码。CutFlow 不代表上游或平台官方产品。

## 许可

工程及编辑器修改按 AGPL-3.0-or-later 提供，保留 Concat 插件例外、版权告知和 BDL MIT 许可。分发时向接收者提供对应源码及构建资料；见 `THIRD_PARTY_NOTICES.md` 与 `docs/SOURCE_OFFER.md`。
