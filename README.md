# CutFlow Windows 视频剪辑工作台

CutFlow 0.3 保留 Concat 的 Rust / Slint 编辑器，将 B站素材、Flow2API 图片视频生成、本地配音字幕和外部 AI 草稿操作整合到原生窗口。当前验证状态见 [验收与已知限制](docs/验收与已知限制.md)，使用步骤及 MCP / CLI 参数见 [创作与外部 AI](docs/创作与外部AI.md)。

## 使用

1. 从本仓库 Actions 的成功构建下载 `CutFlow-0.3.0-windows-x64` Artifact，解压其中的便携 ZIP，启动 `CutFlow.exe`；也可运行 Setup。EXE、DLL 和 tools 必须保持完整。
2. 新建或打开工程，先保存到 D 盘。顶部「素材」可导入和采集素材，「AI 创作」提供 B站扫码、收藏夹、图片/视频生成和角色场景库，「剪辑」返回时间线。
3. 在「AI 创作 → 连接与外部 AI」填写已有 Flow2API 服务地址和密钥，测试连接后选择模型。完整模型变体决定比例与时长；具体权限和额度以服务为准。
4. 默认突出日常剪辑；检查器「进阶」展开特效、增强和调色。高级效果与旧工程格式保留。
5. 外部 AI 通过 `tools/cutflow-agent.exe mcp` 或 `call` 操作当前工程。先在窗口启用外部控制，草稿默认停在人工检查和导出确认。

没有可用硬件显卡时自动使用软件渲染。遇到显卡驱动相关启动错误，也可在启动前设置环境变量 `CUTFLOW_RENDERER=software`；此模式的预览速度受 CPU 性能影响。

B站会员不保证每条视频都有 4K。严格模式不静默降档，并检查下载文件实际尺寸和音视频轨。取消云端生成不保证停止后台计费；结果待核查的任务不会自动重新提交。

## 数据

配置、模型、日志、任务、参考库和加密凭据均在软件旁的 `portable`。B站队列在 `portable/localappdata/CutFlow`。工程媒体随工程保存在 `assets` / `audio`；原编辑器工程保留绝对路径，移动整个工程后仍可能需要重定位。不要清理仍被工程引用的素材。DPAPI 凭据跨 Windows 用户或电脑需要重新设置。

## Windows 构建

需要 Windows x64、Python 3.12+、Rustup、MSVC 2022 Build Tools（C++ 桌面工具与 Windows SDK）、CMake。可选 Inno Setup 6 生成安装包。已有工具可以只读复用，Rust 工具链、Cargo 包、原生依赖、临时文件和编译缓存统一写入工程 `.build`。

```text
python -B -X utf8 scripts/build_windows.py --installer
```

不需要安装包时省略 `--installer`。旧的 PowerShell 7 入口 `scripts/Build-Windows.ps1` 仅转发给同一 Python 构建器。首次构建要下载较多依赖；构建器验证上游锁定的 FFmpeg、ONNX 和 sherpa SHA256，libclang 使用指定版本的 Python wheel。生成包未签名。

成功输出为 `dist/CutFlow-0.3.0-windows-x64.zip`、SHA256 和可选 Setup；包含编辑器、BDL、FFmpeg、ffprobe、cutflow-agent、DLL、说明、TaskSpec 示例和许可证。打包拒绝覆盖现有同名发行目录。

GitHub Actions 在 main 推送后自动构建，也支持手动运行；只有构建及启动检查成功才上传产物。真实账号权限、付费生成、最终音画效果需要单独实测，模拟测试不能替代。

## 开发与测试

```text
python -B -X utf8 scripts/dev.py -- cargo test --locked --manifest-path workbench/Cargo.toml
python -B -X utf8 scripts/dev.py -- cargo test --locked --manifest-path downloader/Cargo.toml
python -B -X utf8 scripts/dev.py -- cargo check --locked --manifest-path tests/ui-compile/Cargo.toml
python -B -X utf8 tests/check_callback_wiring.py
```

`workbench` 提供本地控制、任务与生成协议；`downloader` 管理 BDL 进程和实际画质校验；`editor` 保留完整原生编辑引擎；`reference/bdl` 保留固定版本账号模块和 CLI。上游固定提交见 `upstreams.lock.json`。

参考项目：[Concat](https://github.com/jub0t/concat)、[BDL](https://github.com/Yuelioi/bdl)、[Flow2API](https://github.com/TheSmallHanCat/flow2api)、[MoneyPrinterTurbo](https://github.com/harry0703/MoneyPrinterTurbo)。阶段式草稿流程参考 MoneyPrinterTurbo，未复制其代码。CutFlow 不代表上游或平台官方产品。

## 许可

工程及编辑器修改按 AGPL-3.0-or-later 提供，保留 Concat 插件例外、版权告知和 BDL MIT 许可。分发时向接收者提供对应源码及构建资料；见 `THIRD_PARTY_NOTICES.md` 与 `docs/SOURCE_OFFER.md`。
