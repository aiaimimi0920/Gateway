# ChatGPT Web：普通浏览器授权导入方案

## 真实失败与修正

用户在 managed-login 手测中选择 Google 登录，收到“此浏览器或应用可能不安全”。
因此不能将“模拟登录测试通过”当成真实账号接入完成，也不应登记蓝色完成标记。
不通过隐去自动化标记、伪造指纹或降低账号保护去解决该问题。

新路线把登录与会话交接分开：用户在日常浏览器正常登录，授权 ChatGPT 专用扩展导出，Gateway 通过既有 JSON/secret patches 事务校验并保存。
扩展不是 OAuth 客户端，不读取 Google 登录材料。它不会保证 Google 登录一定成功，只是不再要求在 Playwright 启动的浏览器中登录。

## 本轮改动

- 新增 `scripts/chatgpt-web-import-extension/`，Manifest V3，本地开发版本。
- 权限仅 cookies、scripting 和精确 ChatGPT host；没有后台脚本、自动注入、Google 权限或外部消息入口。
- 用户必须勾选导出敏感凭据确认，并点击导出；只读取当前活动 ChatGPT 页及对应 Cookie store。
- JSON 只包含 Web token、限定域 Cookie、真实 token 过期时间、设备和白名单浏览器元数据；无 OAuth refresh token。
- 文件不自动上传；读取和输出有 128 KiB 上限，错误不回显秘密。
- 默认账号界面移除 managed-login 入口，保留正常 JSON 导入；旧后端能力未作破坏性删除。
- 不重启或修改当前 4200 运行实例，不改现有账户凭据，不热改已发布手测包。

## 操作体验与未完成事项

当前是“扩展导出文件 → Gateway 选择文件”的显式交接版本，并非已经完成一键回传。
这一步优先验证真实普通浏览器的会话材料能否完整进入 Gateway，避免再次先承诺自动登录成功。
后续若实现一键回传，应另做本机配对与一次性授权，不把管理令牌放进网页 URL，不开放任意网站可写的本机凭据接口。
扩展加载和敏感权限授权必须由用户确认；未自动安装。开发者模式加载不是正式面向普通用户的最终分发方式。
正式使用仍需浏览器安装/权限审核、下载行为、真实账号导出导入、指定账号调用验收；目前不标蓝。
导出文件包含明文登录凭据，用户导入后应自行删除，不能自动删除用户下载目录文件。

## 验证

- 扩展定向单元测试 4 项通过：权限范围、字段白名单、过期/异常拒绝、Cookie store/页面变化隔离。
- 账号编辑器相关前端测试 20 项通过；TypeScript typecheck 通过。
- 有效行检查器测试 31 项通过。没有真实上游模型调用。
- 本轮未编译新 EXE；旧版的会话 JSON 备用入口可用于后续授权手测。

联网核对的官方文档（2026-10-28，HTTP 200）：

- https://developer.chrome.com/docs/extensions/reference/api/scripting
- https://developer.chrome.com/docs/extensions/reference/api/cookies
- https://developer.chrome.com/docs/extensions/develop/concepts/network-requests
