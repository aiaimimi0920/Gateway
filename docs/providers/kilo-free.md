# Kilo AI Gateway Free

从服务商目录选择 **`kilo-free`**，不需要添加账号或填写 API Key。
仅暴露 `kilo-auto/free`，固定使用官方匿名 Chat Completions；支持普通 JSON
和 SSE，不导入付费目录，不因失败改选 Kilo 付费模型。
手动配置见 [路由示例](examples/kilo-free.routes.sample.json)。

按 [Kilo 官方说明](https://kilo.ai/docs/gateway/usage-and-billing)，免费模型按
出口 IP 共享每小时 200 次。Neuro Gateway 不把它拆成各本地调用者的配额，
也不猜测剩余额度或准确重置时间。上游返回 429 时保留 `Retry-After` 的内部
冷却依据；当前请求立即返回，不重试、不切换候选。
