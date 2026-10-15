# CutFlow Windows 视频剪辑工作台

CutFlow 0.2.0 是按所附规划实现的 **P0 原生集成源码版**：保留 Concat 的 Rust / Slint 编辑器，在同一窗口添加 B站素材采集，使用固定提交的 BDL CLI 作为下载子进程。

**此压缩包是源码工程，不包含已经编译的 Windows EXE。** 已完成 21 项下载适配层测试、原生面板渲染和模拟媒体宿主下的回调场景验证；完整 Windows 编译、安装和账号实测仍需执行。不要把本版本作为已经验收的 MVP 或正式发行版。具体证据和限制见 [验收与已知限制](docs/验收与已知限制.md)。

## 已写入的功能

- 使用 Concat 原有多轨时间线、预览、修剪分割、音量、字幕/语音和导出入口，不重建第二套剪辑引擎。这些上游能力需要随本修改版做 Windows 回归。
- 原生 Slint“B站素材”面板，支持普通视频和手选多个分 P，显示解析到的账号可用档位和编码。
- 串行下载队列；任务持久化；暂停、恢复、取消和手动失败重试。应用重新打开后不会自行恢复网络任务。
- 默认 qn120 + 禁止降档；下载后通过 ffprobe 探测真实像素、时长、编码、帧率和音视频轨。严格 UHD 的实际像素门槛为长边 ≥3840、短边 ≥2160，支持竖屏 UHD。3840×1600 超宽素材在该模式下会拒绝入库。
- 完整媒体校验后生成 `asset-manifest.json`，绑定创建任务时的工程。当前工程匹配才自动导入，否则等待用户打开绑定工程。
- 入库经当前窗口的 Studio / Command 路径执行；保存工程成功后才确认已入库。完成事件重放会检查工程中已有媒体路径，避免重复导入。
- Windows 当前用户 DPAPI 加密 Cookie；子进程仅通过环境接收，不放入命令行、工程或任务 JSON。退出账号保留已下载媒体。
- 同一用户下载队列文件锁；任务 JSON 原子替换；关闭程序停止所拥有的下载进程树。
- Windows 构建脚本、安装包配置、启动冒烟检查，以及手动触发的 GitHub Actions 工作流。

## 不装开发环境的构建方式

你可以把完整工程放进自己的 GitHub 仓库，使用 Windows 云构建。这里尚未创建仓库或触发任何远程构建。

1. 在 GitHub 创建一个你控制的空仓库，建议先使用私有仓库。
2. 上传 **CutFlow 文件夹内的全部内容**，让 `.github`、`scripts`、`editor`、`reference`、`downloader` 位于仓库根目录。网页上传不便时使用 GitHub Desktop 添加本地目录并发布仓库。
3. 打开仓库的 **Actions → Build CutFlow for Windows → Run workflow**。
4. 流水线在 Windows x64 上测试下载模块、编译 BDL 和编辑器、组装 DLL，执行启动检查，再生成 ZIP 和 setup.exe。
5. 只有流水线成功，才下载运行底部的 `CutFlow-0.2.0-windows-x64` Artifact。先解压 Artifact，再解压其中的便携 ZIP，启动 `CutFlow.exe`；也可以使用生成的 setup.exe。
6. 如果失败，保留失败步骤日志。不要把缺少 DLL 或编译报错的产物当成可用软件。

首次编译会下载大量 Rust 和原生依赖，需要网络与较长时间。脚本不包含代码签名证书，生成的包是未签名开发版。Windows 启动检查不等于实际视频剪辑验收。

## 在自己的 Windows 电脑构建

目标：Windows 11 x64。建议使用至少 16 GB 内存并预留约 25 GB 构建空间，实际消耗依赖编译缓存而变。

先安装以下工具：

1. **Visual Studio 2022 Build Tools**：选择“使用 C++ 的桌面开发”，包含 MSVC x64 和 Windows SDK。
2. **Rust MSVC 工具链**，然后执行 `rustup toolchain install 1.93.1 --profile minimal`。
3. **Git for Windows**，需要其 `bin\bash.exe`。
4. **PowerShell 7**、**Python 3.12 或更新版本**、**CMake**。
5. **LLVM x64**，包含 `libclang.dll`。默认检测 `C:\Program Files\LLVM\bin`，也可以设置 `LIBCLANG_PATH`。
6. 可选：**Inno Setup 6**，用于生成安装程序。

打开 **Developer PowerShell for VS 2022**，再进入 PowerShell 7，切换到工程根目录（请使用较短的本地路径，例如 `C:\dev\CutFlow`）：

```powershell
pwsh
cd C:\dev\CutFlow
rustup toolchain install 1.93.1 --profile minimal
.\scripts\Build-Windows.ps1 -Installer
```

不需要安装程序时，去掉 `-Installer`。如 PowerShell 明确提示脚本受本机执行策略阻止，核验源码后可在当前窗口执行 `Set-ExecutionPolicy -Scope Process Bypass` 再运行；不会修改系统长期策略。

构建脚本会使用上游 `editor/scripts/ci/deps.lock` 固定的 URL 与 SHA256 下载 FFmpeg 开发库、ONNX Runtime 与 sherpa-onnx，不用一个 `ffmpeg.exe` 冒充编辑器编译所需的头文件和链接库。BDL 的 `vendor/bpi-rs` 补丁与 Concat 的 Skia 补丁均保留。

成功输出：

```text
dist/
  CutFlow-0.2.0-windows-x64/
    CutFlow.exe
    *.dll
    tools/bdl.exe
    tools/ffmpeg.exe
    tools/ffprobe.exe
    tools/*.dll
    licenses/
  CutFlow-0.2.0-windows-x64.zip
  CutFlow-0.2.0-windows-x64.zip.sha256
  CutFlow-0.2.0-windows-x64-setup.exe   # 选择 -Installer 时
```

不要单独复制 `CutFlow.exe`，它需要同目录 DLL 与 tools。再次打包前，将旧 `dist` 目录移到别处；脚本会拒绝往已有发行目录混入旧文件。

## 第一次使用

1. 启动生成的 `CutFlow.exe`，新建一个 3840×2160、30 fps 工程，先保存到本地磁盘。
2. 可以先用“导入”加入本地录像，确认预览、拖入时间线、分割、保存、导出都正常。
3. 点击顶部 **B站素材**。无需先登录也能尝试账号允许匿名访问的内容。
4. 若目标内容需要你的账号访问权限，在面板粘贴自己账号的完整 Cookie header 文本（包含 `SESSDATA=...`），点击“保存凭据”。这里没有二维码登录界面。不要将 Cookie 发给别人或上传仓库。
5. 粘贴你有权保存的视频链接，点击“解析画质”。选择分 P，如 `1,2`，保留“严格 4K”，填写用途备注，然后加入队列。
6. 完成后素材自动进入绑定工程的素材库，再拖入时间线。若你中途切换工程，请重新打开原工程并点击“导入绑定工程”。
7. 在原有导出窗口设置 1080p 或 4K 输出。关闭并重新打开工程，检查编辑内容和素材引用。

权限不足、无 4K、验证、403/412/429 或下载错误会停止；不会换账号、代理或下载器自动继续。BDL 的共享预算和冷却规则保留。

## 工程布局与参考源码

| 目录 | 内容 |
|---|---|
| `editor/` | 完整 Concat 固定提交快照与 CutFlow 修改；含原 UI、引擎、测试、许可证和补丁 |
| `reference/bdl/` | 完整 BDL 固定提交源码；本版本保持未修改，用于构建 CLI |
| `downloader/` | 新增 Rust 下载进程适配层、画质校验、队列格式和 DPAPI |
| `scripts/` | Windows 依赖准备、构建、冒烟测试和安装程序配置 |
| `.github/workflows/windows.yml` | 手动触发的 Windows 云构建，不自动发布 |
| `docs/` | 架构、验收记录、后续缺口、源码提供说明 |
| `upstreams.lock.json` | 两份参考源码的精确提交 SHA |

参考项目：[jub0t/concat](https://github.com/jub0t/concat)、[Yuelioi/bdl](https://github.com/Yuelioi/bdl)。本修改版不代表上游项目或哔哩哔哩官方产品。

## 数据位置与恢复

- 下载任务、母版、凭据：`%LOCALAPPDATA%\CutFlow`。
- 媒体母版：`%LOCALAPPDATA%\CutFlow\jobs\<jobId>\media`，工程会引用它；**不能把此目录当缓存清理**。
- 每个任务的 BDL 检查点和资产清单位于同级目录。暂停和取消不会删除已下载文件。
- 原编辑器的工程和偏好位置沿用上游，工程中保留媒体绝对路径；移动电脑后需重定位媒体。
- 损坏或未知版本的队列会禁止覆盖和启动新任务，保留原文件供修复。
- 本版本没有自动缓存清理、自动更新、云同步或账号轮换。

## 测试

```powershell
cargo +1.93.1 test --locked --manifest-path downloader/Cargo.toml
cargo +1.93.1 check --locked --manifest-path tests/ui-compile/Cargo.toml
.\scripts\Test-Windows.ps1 -Bundle .\dist\CutFlow-0.2.0-windows-x64
```

Linux 下的集成测试会使用 `/usr/bin/ffmpeg` 与 `/usr/bin/ffprobe` 生成短样片，并通过假的 BDL CLI 边界测试实际媒体校验。它们没有连接 B站，也不代表真实账号下载验收。Windows 上会另运行 DPAPI 读写测试。

## 许可

本工程及编辑器修改按 AGPL-3.0-or-later 提供；保留 Concat 插件例外和全部版权告知。BDL 快照保留 MIT。真正分发的 FFmpeg、Slint、语音组件等许可取决于构建产物。对外分发时请同时向接收者提供这份对应源码工程和构建资料；参见 `THIRD_PARTY_NOTICES.md` 与 `docs/SOURCE_OFFER.md`。

## 0.2 UI 与交互更新

本版重新布局采集面板，补齐任务筛选、搜索、批量暂停/继续、取消解析、组件检查、文件夹入口、工程切换及记录移除。任务按钮按实际状态显示，Rust 后端再次验证操作条件；取消和移除只影响任务记录，不删除母版。修复周期性列表重建、取消单条任务影响整个队列和面板打开时后台编辑快捷键仍生效的问题。

详细行为见 `docs/UI与按钮行为.md`。`tests/native-ui` 使用真实 Slint 面板和真实下载协调器，媒体宿主使用明确隔离的测试夹具；仅用于界面与回调验证，不代替真实编辑引擎验收。
