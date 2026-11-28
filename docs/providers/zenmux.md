# ZenMux 官方 API

在服务商目录选择 `zenmux-api`。取消「同时添加首个账号」可以先建空池，再在账号表单填写 API Key。
固定目标 `https://zenmux.ai/api/v1`，目录请求为 `GET /models`；推理只声明
`POST /chat/completions`，支持普通 JSON 与 SSE。模型标识保留 `provider/model`
原文。function tools 取决于所选模型。目录不是可调用、免费或余额证明。

账号密钥使用标准 `Authorization: Bearer`，不要把 Key 放进 URL、日志或示例。
无默认账号或模型，不启用自动生成探测。首版不声明原生 Responses、Messages、
OAuth 或余额接口。定向离线测试过滤词为 `zenmux`，无需外部账号。
