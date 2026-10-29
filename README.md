<div align="center">
  <img src="reference/bdl/apps/desktop/src-tauri/icons/icon.png" width="112" alt="CutFlow 图标" />
  <h1>CutFlow</h1>
  <p><strong>让素材抵达世界。</strong></p>
  <p>从 B 站素材、时间线剪辑到 AI 创作与英文视频，一处工作台，连接创作的每一步。</p>

  [![Windows build](https://github.com/daodeqing/CutFlow/actions/workflows/windows.yml/badge.svg?branch=main)](https://github.com/daodeqing/CutFlow/actions/workflows/windows.yml)
  [![GitHub release](https://img.shields.io/github/v/release/daodeqing/CutFlow)](https://github.com/daodeqing/CutFlow/releases/latest)
  [![Platform](https://img.shields.io/badge/platform-Windows%20x64-0078D4)](https://github.com/daodeqing/CutFlow/releases/latest)
  [![License](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)](LICENSE)

  [下载 CutFlow](https://github.com/daodeqing/CutFlow/releases/latest) · [快速上手](#快速上手) · [英文视频工作流](docs/英文视频工作流.md) · [问题反馈](https://github.com/daodeqing/CutFlow/issues)
</div>

---

**少在工具之间来回切换，多把时间留给作品。** CutFlow 是一款开源的 Windows 视频创作工作台：管理素材、剪辑视频、连接你选择的 AI 服务，并把有授权的中文视频制作成带英文字幕和精选弹幕的版本。

## 一条从灵感到成片的工作流

```text
获取素材  →  剪辑与整理  →  AI 创作与英文制作  →  预览审核  →  发布
```

- **素材，随时就位。** 使用 B 站账号、链接解析、分 P、收藏夹和下载队列整理素材；按账号和原视频实际权限选择可用画质。
- **剪辑，专注表达。** 打开独立编辑器，将素材导入工程，使用时间线、预览和导出；主窗口与编辑器可分别运行。
- **AI，听你的。** 连接兼容 OpenAI Chat Completions 的服务，选择已有模型或输入模型名；图片、视频创作和英文工作流的具体能力由服务与模型决定。
- **让视频跨越语言。** 从已有字幕或语音识别开始，生成英文字幕、精选英文弹幕、视频简介和 X 推文；可以给 AI 一句修改要求，单独重写文字。
- **把审核留给创作者。** 预览成片后，用浏览器打开带文案的 X 页面；也可把上传指令交给 Cindy 协助。登录、浏览器权限、视频上传及最终发布以实际页面和用户确认为准。

## 快速上手

1. 从 [最新发行版](https://github.com/daodeqing/CutFlow/releases/latest)下载 **Windows x64 安装版**或**便携版 ZIP**，选择一种即可。便携版完整解压后运行 `CutFlow.exe`，保留同目录的 DLL、`tools` 和 WebView2。
2. 打开「B站素材」，登录后粘贴链接，或选择收藏夹和已完成任务。只处理你有权下载、修改及发布的内容。
3. 在「设置 → AI 服务」配置服务地址与密钥，检查连接并选择可用模型。需要 Flow 项目绑定的功能还取决于对应服务和账号。
4. 选择「剪辑」处理素材；或打开「英文制作」，设置字幕、弹幕、模型和画面选项，完成后审核简介与推文。
5. 在浏览器中确认 X 账号和成片，按需上传并发布。是否支持上传由 X 账号、浏览器权限及视频规格决定。

详细步骤见 [英文视频工作流](docs/英文视频工作流.md)、[创作与外部 AI](docs/创作与外部AI.md) 和 [验收与已知限制](docs/验收与已知限制.md)。

## 为创作而设计

| 你关心的事 | CutFlow 的做法 |
| --- | --- |
| **模型自由** | 默认模型与字幕、弹幕、语音、文案步骤可分别选择；支持手动输入模型名。 |
| **更轻的本地语音识别** | 可选本地 Whisper，首次使用时按需下载模型；也可选择支持音频的 AI 服务。 |
| **保留原素材** | 下载与英文成片分别保存；可选深色字幕条遮挡底部原字幕，避免画面拉花。 |
| **自己的工作目录** | 配置、任务、下载、模型和加密凭据放在软件旁的 `portable` 目录；工程媒体随工程保存。 |
| **自动检查新素材** | 可监控指定收藏夹或 UP 主；首次建立基线，后续在程序运行或驻留托盘时检查新增内容。 |

可选字幕条会遮住画面底部一部分，其他位置的烧录字幕不一定能清除。文案不自动读取 X 实时热榜，用户可提供相关话题作为参考。AI 服务可能将内容发送到云端并收费；本地代理地址本身不代表模型在本地运行。

## 数据与使用边界

- `portable/data` 保存 B 站数据，`portable/downloads` 保存下载，`portable/localizations` 保存英文制作结果；项目生成内容位于 `portable/generated`，编辑状态位于 `portable/editor`。
- Windows DPAPI 凭据通常不能跨 Windows 用户或电脑直接使用；迁移时请重新配置账号和密钥。升级前请备份工程及 `portable` 数据。
- 视频画质、AI 生成、语音输入、第三方登录和 X 上传都受对应账号、模型及服务权限影响。使用前请检查素材和音乐的授权。
- 完全退出 CutFlow 后，本地监控会暂停；任务运行时关闭主窗口可驻留托盘。自动化不会代替发布前的人工审核。
- 真实账号、新图、连续改图、4K 及视频生成的验收范围，以 [AI 功能检查](docs/ai-functional-verification-20261020.md) 和 [验收与已知限制](docs/验收与已知限制.md) 为准。

## 从源码构建

需要 Windows x64、Python 3.12+、Rustup、MSVC 2022 C++ 工具及 Windows SDK、CMake；生成安装程序还需要 Inno Setup 6。构建器会在工程 `.build` 下管理依赖和缓存，首次构建需要下载较多内容。

```bash
python -B -X utf8 scripts/build_windows.py --installer
```

省略 `--installer` 可只生成便携包。CutFlow 0.6.6 的产物名称为 `CutFlow-0.6.6-windows-x64.zip`，安装版和校验文件随发行版提供。构建包含主程序、编辑器、FFmpeg、ffprobe、语音识别组件、WebView2 和运行库；构建产物未签名。硬件渲染不可用时可使用软件渲染，必要时在启动前设置 `CUTFLOW_RENDERER=software`。

## 开发与贡献

```bash
python -B -X utf8 scripts/dev.py -- cargo test --locked --manifest-path workbench/Cargo.toml
python -B -X utf8 scripts/dev.py -- cargo test --locked --manifest-path downloader/Cargo.toml
python -B -X utf8 scripts/dev.py -- cargo check --locked --manifest-path tests/ui-compile/Cargo.toml
python -B -X utf8 tests/check_callback_wiring.py
```

欢迎通过 [Issues](https://github.com/daodeqing/CutFlow/issues) 反馈问题、分享想法或提交改进。`workbench` 负责本地控制和任务协议，`reference/bdl/apps/desktop` 提供主界面，`editor` 提供剪辑引擎；上游版本与快照见 `upstreams.lock.json` 和 `docs/bdl-source-snapshot.json`。

## 致谢与许可

CutFlow 站在优秀开源项目的肩膀上：[Concat](https://github.com/jub0t/concat)、[BDL](https://github.com/Yuelioi/bdl)、[Flow2API](https://github.com/TheSmallHanCat/flow2api) 与 [MoneyPrinterTurbo](https://github.com/harry0703/MoneyPrinterTurbo)。CutFlow 不代表这些项目或平台的官方产品。

工程及编辑器修改按 AGPL-3.0-or-later 提供，保留 Concat 插件例外、版权告知和 BDL MIT 许可。分发时请提供对应源码及构建资料；详情见 [第三方声明](THIRD_PARTY_NOTICES.md) 和 [源码提供说明](docs/SOURCE_OFFER.md)。
