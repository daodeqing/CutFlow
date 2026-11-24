# CutFlow YouTube 扩展

这个扩展复用当前 Chrome 的 YouTube 登录会话。它只在 CutFlow 打开的
`https://www.youtube.com/@deqingdao/videos` 和 YouTube Studio 页面工作：选择英文成片，填入英文标题和简介，然后停在发布前审核页，不会点击发布，也不会把任务带到其他频道。

安装一次：

1. 在 Chrome 地址栏打开 `chrome://extensions`。
2. 打开右上角“开发者模式”，点击“加载已解压的扩展程序”。
3. 选择 CutFlow 安装目录中的 `youtube-extension` 文件夹。
4. 在扩展详情中允许它访问 YouTube；首次使用 Chrome 会显示调试权限提示，请选择允许。

使用前先在当前 Chrome 登录目标 YouTube 账号，并打开 `https://www.youtube.com/@deqingdao/videos`。CutFlow 的“上传到 YouTube”按钮会把地址交给已打开的 Chrome 窗口并在当前配置文件中打开新标签，再把该标签带到 Studio；上传和标题/简介填入完成后仍需要你在 Studio 检查可见性、受众、版权和最终发布按钮。
