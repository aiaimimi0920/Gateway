# ChatGPT Web 会话导出扩展（待真实浏览器验收）

本扩展是本地开发版本，尚未发布到扩展商店，也没有完成真实账号端到端验收。
它不是 Google OAuth 客户端，不读取 Google Cookie，不隐藏自动化标记，不修改浏览器安全设置。

## 使用流程

1. 由用户审核源码并明确授权后，在日常 Chrome/Edge 的扩展管理页面加载本目录。
   加载未打包扩展属于开发测试方式，不应作为最终面向普通用户的分发体验。
2. 在同一个日常浏览器中正常登录 ChatGPT（可以使用原本已登录的账号）。
3. 保持当前标签页为 `https://chatgpt.com/`，点击扩展图标。
4. 确认将当前账号的登录凭据保存到本机，再点击“导出当前账号”。
5. 在 Gateway 的 chatgpt-web 新增账号窗口，选择下载的 `gateway-chatgpt-web-session.json`，再保存。
6. 文件包含 access token 和会话 Cookie。不要发送到聊天、工单、仓库或同步盘；导入后由用户删除。

## 权限与边界

- 仅申请 `cookies`、`scripting` 和 `https://chatgpt.com/*`，没有 `<all_urls>`、Google、历史记录或后台持续采集权限。
- 仅在点击导出后，读取当前活动 ChatGPT 标签页的会话接口与对应 cookie store 中的 Cookie。
- 不向 Gateway 或任何第三方自动上传材料，不监听网页传入的消息，不读取页面 localStorage。
- Cookie、token 不显示在扩展界面、不写日志；失败只返回固定提示。
- 不保证网页会话永久有效，也不把导出成功等同于模型调用成功。
- 不同 Cookie store 被隔离；尚未真实验证浏览器的分区 Cookie、企业策略和下载限制。

正式分发前仍需扩展安装/权限审核、真实浏览器导出、Gateway 导入与指定账号调用验收。
