# Gemini Canvas 调用阶段 HTTP-only

## 当前运行方式

标准 `gemini_canvas_web_reverse_compatible` adapter 由 Rust 直接请求 Google Canvas
宿主 `q4uTj` / `RouteLlmRequest`。调用不启动 Edge、Chromium、无头浏览器、CDP 或
Node worker，不需要可用的 browser pool，也不使用普通 Web `StreamGenerate` 或官方 API Key。

Gemini Web 和 Gemini Canvas 维持两个独立服务商模板。Canvas 历史 preset ID
`gemini-canvas-browser-relay` 为兼容旧配置保留，不代表调用仍使用浏览器。
标准 adapter 强制 `DirectHttp`，旧保存的 `browser_backed` 和 endpoint override
不会重新启用浏览器；不要求用户编辑数据库或重新创建已有账号。

首次登录、导入账号和凭据失效后的重新认证仍可使用浏览器。HTTP 请求读取程序正常导入的
账号材料，不导出登录状态，不维护跨请求的共享 Cookie jar，不承诺永久免登录。
本路径没有接入 Web HTTP auth checkpoint 或 legacy Canvas browser checkpoint。
独立 `gemini_canvas_program_web_reverse_compatible` 实验线不在本轮迁移范围。

## 能力与边界

已通过新构建后端真实公共 API 验收的范围：文字、buffered SSE、图片生成与编辑、
TTS WAV、图片理解、WAV/MP3 音频理解、专用 multipart 转写、PDF 文档理解和视频输入理解。
SSE 仍是完整结果转换后的 buffered SSE，不是上游逐 token 传输。

Canvas 音乐生成和视频生成仍返回 `gemini_canvas_host_media_contract_unavailable`。
视频输入理解成功不代表视频生成可用；不会自动回退到普通 Web、浏览器或其他账号。
已验收格式不代表所有文档格式、音频编码或模型别名均可用。

## 故障与资源约束

每次调用先按精确账号读取材料，执行一次引导 GET 和至多一次生成 POST。
重定向、challenge、认证失败或不完整媒体结果均明确失败，不做浏览器 fallback、
跨账号切换或不确定执行后的自动重放。错误只输出固定分类，不包含认证材料或原始上游正文。

认证或 challenge 需要用户重新登录时，应通过正常账号导入/重新认证流程处理，
不要通过重试生成、手工导出 Cookie 或增加无头浏览器来绕过。
读取、编码和响应均设有大小限制，总 deadline 覆盖整个 exchange。
调用阶段不再为每个请求配置浏览器；这不意味着桌面管理界面本身不使用 WebView2。

详细实现、安全边界、11 项真实用例与资源快照见
[r32 验收记录](progress/2026-10-31-gemini-canvas-http-only-r32.md)。
多账号并发、长期 Cookie 有效性、内存压力/泄漏和正式发布门禁不属于该次验收结论。
