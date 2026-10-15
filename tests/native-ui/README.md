# 原生 UI 和协调器检查

此目录引用真实的 downloads.slint 与 downloads.rs。构建时从真实 app.slint 提取下载面板的属性、回调声明和接线，避免测试维护另一份手写接口。

`fixtures/concat-host` 和 `fixtures/concat-project` 是明确隔离的媒体宿主测试夹具：只模拟当前工程、媒体登记、探测和保存失败。它们不参与生产软件构建，也不实现真实 FFmpeg 解码、GPU 合成或时间线。测试成功不等于完整编辑器验收。

## 构建与检查

```sh
cargo check --locked --lib --manifest-path tests/native-ui/Cargo.toml
cargo run --locked --manifest-path tests/native-ui/Cargo.toml --bin coordinator-smoke
cargo run --locked --manifest-path tests/native-ui/Cargo.toml --bin cutflow-native-ui-check
```

协调器 smoke 目前使用 Unix 的本地组件夹具；在 Windows 上明确跳过。Windows 需运行真实应用验收。离屏渲染和 UI 库检查可用于独立验证 Slint 组件。

Linux 开发环境需要 fontconfig 开发支持和 pkg-config；没有它们时需要先准备相应开发依赖。可选的 `CUTFLOW_UI_FONT` 指向本机有许可的 CJK 字体文件；提供该变量会在测试 UI 构建中导入该字体，不修改生产字体清单。未提供时依赖系统字体。

渲染参数：

- `CUTFLOW_UI_WIDTH`、`CUTFLOW_UI_HEIGHT`：截图尺寸，默认 1120×900。
- `CUTFLOW_UI_OUTPUT`：PNG 输出路径。
- `CUTFLOW_UI_LIGHT=1`：浅色主题。
- `CUTFLOW_UI_CLICK_CHECK=1`：在固定 1120×900 尺寸进行鼠标命中与 Escape 检查。

示例数据不是用户素材或真实 B站下载结果。不要将 QA 程序作为 CutFlow.exe 分发。
