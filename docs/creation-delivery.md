# CutFlow 0.6.0 交付验收

项目式 AI 创作、连续改图和流畅导航已实现并上传 GitHub。GFlow 0.1.12 最终代码已完成五平台云编译；CutFlow 的主窗口、编辑器和最终安装包均完成云端构建与验证。

## 下载

- [CutFlow 0.6.0 完整构建产物](https://github.com/daodeqing/CutFlow/actions/runs/39090561957/artifacts/12123150717)：安装 EXE 426.5 MB、便携 ZIP 591.6 MB，附两份 SHA-256；安装版与便携版选择一种使用。
- [GFlow 0.1.12 Windows 插件](https://github.com/daodeqing/cpa-plugin-GFlow/actions/runs/39054893053/artifacts/12109331190)：项目绑定需要同时更新客户端与插件。

本机安装和 GitHub Release 保持原状。没有替换现有代理插件或重启用户服务。

## 精确构建来源

- CutFlow 原生组件：4ea187edf2c308ef9fb7777a907a8556b14075d0，run39088247743 的 Build hub / Build editor 均 success。原 package 在两个组件成功后主动取消，由新版说明打包任务替代。
- 最终打包：b9e9320b8832ba10d5403c5d6fd12770887f4b91，run39090561957 success。复用校验确认差异仅为说明及打包脚本；没有复用不同运行时代码。
- GFlow 最终运行时代码：d8b0e01a0305088aae2b31b6ecb56707b159523a，run39054893053 五平台 success。后续提交只更新验收记录。
- 最终主程序 SHA-256：21eef0acce23a5bfc2b501fabd46f5e316a80d9531d515df0b9acf17b0ef3a12，从最终便携包按范围提取并独立计算，匹配 build-info.json。
- 检查最终 ZIP 目录和小文件 CRC：必须程序、WebView2、项目说明齐全；portable 无测试或用户数据。安装包及完整便携 ZIP 未重新整包下载计算 SHA，发布的 SHA 文件已读回。

## 最终交付主程序的实机导航

使用同一固定 WebView2 Edg/155.0.4283.45、同设备、相同实际 CSS1426×892/DPR1.25，比较已核实0.5.0基线与最终交付0.6.0主程序。每场景50次预热与4次冷切换，共216次。

|场景|版本|预热首帧P95|冷切换最大|
|---|---|---:|---:|
|idle|baseline|213.1 ms|328.2 ms|
|idle|candidate|25.9 ms|113.4 ms|
|active-download|baseline|212.8 ms|334.0 ms|
|active-download|candidate|21.1 ms|125.5 ms|

新版两场景达到预热P95≤100ms、冷切换≤250ms目标，全部零pageerror。夹具为1000历史任务、新版500聊天和1×1本机图片；活动场景是本机模拟AI结果下载。不能据此宣称真实4K解码、B站并发或跨设备性能。

此前源代码相同的预备hub另有216次记录；本节使用最终包哈希对应的新一轮测量，原始记录见 verification-20261018-creation/webview2-final-comparison.json。

## 验证范围

- 前端58文件309项通过；工作台Rust47项通过；编辑引擎114项通过。类型检查、相关lint/style检查通过。
- 16个浏览器交互用例均有通过证据；最后整轮15通过1失败，阅读位置修复后相关2项复验通过，未冒充同一轮16/16。
- 云端整包验证通过主窗口、编辑器、软件渲染启动以及模拟媒体、幂等、错误、停止、下载重试、素材导入和独立进程检查。
- 插件本地Python128通过9原生bootstrap跳过；云端冻结后9个原生bootstrap通过；专属项目34、上游29、Rust14项通过。最终Windows包SHA验证后，在独立真实CPA宿主执行6项免费控制接口检查。
- 没有创建真实Google Flow项目或付费生成，实际账号额度、上游创建/生图和视频质量未验收。控制协议宿主验证与模拟项目绑定测试分别记录。

## 清理

累计移入回收站 10,656,932,507 字节，约 10.657 GB / 9.925 GiB。文件/目录名、字节数和状态见 verification-20261018-creation/cleanup-summary.json。
测试应用、WebView2子进程、临时CPA和Vite均已关闭。保留源码、正式复用工具、截图与测量记录、最终插件包，以及D盘Node/Rust工具和已安装依赖；原有安装、媒体与portable未改。
