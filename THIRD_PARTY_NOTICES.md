# CutFlow 第三方来源

CutFlow 是独立命名的 Concat 修改版，包含完整对应源码。上游不为该修改版背书。精确提交见 `upstreams.lock.json`。

- Concat: https://github.com/jub0t/concat ，Copyright 2026 Jareer and Concat contributors，AGPL-3.0-or-later。保留 `editor/LICENSE`、`editor/LICENSE-EXCEPTIONS.md`、`editor/THIRD_PARTY_NOTICES.md`、`editor/TRADEMARK.md`。
- BDL: https://github.com/Yuelioi/bdl ，Copyright YUELI and contributors，MIT。保留 `reference/bdl/LICENSE` 和其 vendor 许可证。
- 新增 CutFlow 下载适配层、原生面板和构建脚本：AGPL-3.0-or-later。
- CutFlow 原创几何图标：随本工程 AGPL-3.0-or-later 提供；上游图标保留在参考素材目录但不作为 CutFlow Windows 图标。
- FFmpeg 发行构建由 Concat 的依赖锁固定，使用 GPL shared build。构建包复制上游归档内的许可文件；提供或分发编码器二进制时，须保留该实际构建所对应的源码、修改、配置及许可告知。
- Slint、Skia、ONNX Runtime、sherpa-onnx、whisper、字体及 Rust 依赖：沿用上游清单；并未据本文件声明已经完成整个依赖树的发行审计。可选模型在运行时下载，其许可也应在对外分发前逐项核对。

`editor/` 与 `reference/bdl/` 中的所有原作者版权和许可证仍有效。构建脚本生成未签名开发包；不要宣传为完成供应链审计的正式产品。
