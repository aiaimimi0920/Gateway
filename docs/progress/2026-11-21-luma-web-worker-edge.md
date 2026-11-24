# L2 Web worker：真实 Edge、本机合成上游（2026-11-21）

本批不改生产代码。在[前批 SSE 修复](2026-11-21-luma-web-sse.md)与禁止创作 POST 重定向重放的基础上，补齐此前 VM 和 Node Fetch 未覆盖的浏览器脚本执行：仓库[显式检查入口](../../tools/luma/web-worker-check.mjs)以生产 CLI 启动实际 Microsoft Edge（本机安装版本 `157.0.4341.60`），不加载模拟 Playwright、不复用个人 profile。测试在随机 loopback 端口运行受控 HTML／HTTP 上游；凭据、动作、结果和媒体 URL 均为夹具，URL 不下载。

## 验证结果

- **正常完成通过。** 真实浏览器加载 board，带合成 Cookie 读取 signature，带合成 Bearer 订阅 SSE，再发送一次指定动作 POST。忽略其他产物与原产物的 `finalizing`，接受混合换行的原产物完成事件，返回匹配的 action ID、output ID 和签名 URL，关闭 SSE。
- **HTTP 307 通过。** 用真实 307 应答创作 POST。Chromium 下生产操作将其映射为 `lumalabs_action_redirect`，不返回结果、不访问重定向目标、不重发，关闭 SSE。两项均核对上游收到的确切请求顺序和唯一 POST 的 JSON 正文。
- `node --test tools/luma/web-worker-check.mjs`：**2 passed**。Node 语法、有效行检查器 **31 passed**、ratchet 通过；新检查入口 113 有效行。此前 40 项 Luma Node 回归复用上一批证据，未重复计算。

## 真实 Web 仍待验证

再次尝试当前已打开的浏览器时，桌面桥接在枚举窗口前失败：`Computer Use native pipe is unavailable: failed to connect native pipe: 系统找不到指定的文件。 (os error 2)`。这是访问工具失败，不是用户会话失效。公开的 Agents REST 文档不能替代 App 登录后的 board 合同。没有触发真实生成、扣费、下载、部署或新包。

下一步只读使用已登录页面已有的完成任务。会话／board 和任务详情可分别提供请求方法、脱敏路径、状态码，以及请求／响应字段名和类型；需要对应同一任务时，用一致的占位符代替真实值。若已有历史动作记录可一并提供，没有则保持模型／动作映射待验，不要求为补样本重新生成。不要提供原始 HAR、会话 Cookie、Authorization、提示词、作品内容或带签名的 URL。
