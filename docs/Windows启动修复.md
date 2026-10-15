# Windows 启动访问冲突修复

2026-10-15，在本机 Windows 11 / Ryzen 7 7735H / RTX 4060 Laptop 上复现。

## 根因与证据

提交 f7fe369 的安装包能通过 `--version` 和 `--smoke`，正常显示窗口时在第一帧文字绘制中以 0xC0000005 崩溃。`--smoke` 在进入可见窗口事件循环之前退出，因此没有覆盖此路径。

崩溃位于 CutFlow.exe RVA 0x2d4f3e0，调用点为 RVA 0x241fad6。该调用对应 Skia `SkCanvas::getTopProps()`；MSVC C++ 成员函数把 `this` 放在 RCX、隐藏的结构体返回指针放在 RDX，生成的 Rust 调用点却把二者交换，导致访问了错误的虚函数表。

源代码修复：Windows 文字渲染通过 `Canvas::props()` 取得属性，其结果经显式输出指针传递，避开按值返回结构体的接口。其他平台保留 `top_props()`。

`tests/check_windows_startup.py` 新增真实窗口检查：正常启动、等待窗口绘制、确认可见且无未响应状态、发送 WM_CLOSE 并检查干净退出。`scripts/Test-Windows.ps1` 在原有冒烟检查之后运行该检查。

## 已安装版本的本地应急修复

安装位置：`D:\LUO\video editing\dist\CutFlow`。

原程序 SHA-256：`9bda22c484a0d953692ee798a52cc83c210913de529e121ec1efc412f0d65830`。

在确认整个文件哈希及指令字节匹配后，仅将 RVA 0x241face 的 8 字节从 `488d4c24404889ea` 改为 `4889e9488d542440`，使该唯一调用点遵循 C++ 成员函数调用约定。程序原件保留为 `CutFlow.original.exe.bak`。该办法只适用于此精确二进制，不得套用到其他版本。

修复后 SHA-256：`225c46be24a241b12e91ae2149cc711bbbcdd1a19f00523d8082ee8e83d3a729`。

该本地程序经实际窗口验证；源代码修复需要通过后续云端重新编译生成新安装包。原 ZIP 和原 setup.exe 仍包含旧程序，重装会覆盖本地修复。

安装目录增加 `portable` 文件夹，编辑器设置、模型和日志按已有便携机制存放在此。历史 C 盘数据未移动或删除；新建工程时仍需自行选择 D 盘项目位置。

## 验证边界

已验证本机启动、主界面显示和新建工程设置对话框。启动检查不代表素材下载、完整剪辑流程或导出验收。
