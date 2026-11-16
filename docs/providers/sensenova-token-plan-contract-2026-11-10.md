# SenseNova Token Plan Key 合同

本地实现使用操作者在 SenseNova 控制台明确创建的 Token Plan API Key。控制台 OAuth、积分池读取和小浣熊账号是独立范围；不读取浏览器、桌面客户端或社区插件的登录材料。

## 官方依据

2026-11-10 匿名联网复核 [官方 API 文档](https://platform.sensenova.cn/docs)、[Token Plan 页面](https://www.sensenova.cn/token-plan) 与 [API Keys](https://platform.sensenova.cn/console/keys)。文档是客户端渲染页面，实际读取其公开引用的 `2582-7ac5809c83d50574.js` 中 `82285` 模块的 Markdown 字符串，未执行 JavaScript。原文快照保存在 `linshi/gateway-sensenova-20261110/contracts/official-api.md`，含来源、UTC 和 SHA-256。

- 固定 Base 为 `https://token.sensenova.cn/v1`，使用 `Authorization: Bearer KEY`。同一 Key 支持 Chat、Messages、Responses；本实现选 Chat 作为唯一上游 wire。
- `GET /models` 返回 `data[]`，包含 `id`、`input_modalities`、`output_modalities` 等。目录不是订阅权益保证，不能从可见模型推断实际推理权限。官方当前模型名已不同于固定社区快照，因此不内置旧静态列表。
- Chat 支持 SSE 与函数工具；工具结果使用 `role:tool` 和 `tool_call_id`。官方 GLM 小节明确 `data: [DONE]`，结束原因包括 `stop`、`length`、`tool_calls`、`content_filter`。文档没有逐字 Chat SSE 返回样本，不能声称所有模型的终帧布局均已真实验证。
- 用量的 completion/output tokens 已包括 reasoning tokens，不能重复相加。积分池、现金余额和 Token 用量不是同一个数值。
- Responses 官方具有独立模型支持集和无状态限制；公共 Responses 在本切片桥接 Chat，不冒称原生 Responses 及其 reasoning 完整往返。
- 文档错误例为 `{error:{type,code,message}}`；匿名 `/models` 实际返回 HTTP 401、`{"error":{"code":16,"message":"Authorization Not Found"}}`，不能强制所有错误都有字符串 type/code。

固定社区 `magpie-community/plugins@93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/sensenova/README.md` 明确声明 `pool-usage`、浏览器截获授权码、自动创建 Key 使用未公开控制台接口。官方当前资料未提供独立第三方 OAuth client 注册合同；本实现不借 `client_id=nova` 或自动管理账号 Key。

## Gateway 行为

- preset `sensenova-token-plan`、profile `sensenova_token_plan`、feature `line-sensenova-token-plan`；UI 入口 `sensenova-token-plan-api`。固定目标、普通 Key 表单、SecretPatch、grant 与 revision/CAS 复用现有 owner，不新增存储版本。
- 目录使用独立管理读取：显式账号或显式新 Key、敏感信息 grant、固定 `/models`，30 秒期限、2 MiB 响应和 2048 项上限，禁跳转。仅导入 `output_modalities` 明确含 `text` 的条目，排除 U 系列图像生成；失败保留旧目录。应用表单后仍需正常 CAS 保存，不自动发起模型推理。
- 公共 Chat、Messages、Responses 的文本与函数工具统一编码到 `/chat/completions`；最终 sender 再校验目标、协议和选中 Key，清除其他身份头，关闭 ambient headers 和 redirect。禁止 Native Codex、XML tool fallback 绕过产品 owner。
- 非流式回复使用共享有界读取和严格 Chat 结构校验；流式在转换前复用严格 Chat admission，合法 finish、`[DONE]` 和 EOF 到齐后才释放结束与用量帧。错误包、截断、非法 JSON 和错误尾部明确失败；已收到生成响应的协议失败不重放。
- 共享 Chat 守卫新增工具字段到齐校验：已出现的工具必须具有非空 ID、名称和 arguments 字段，不能由转换器生成默认工具身份。只保留字段存在标志，不积累第二份参数正文。Qwen workspace 也复用此修复，其原错误码和重试／fallback 策略保持不变。
- 文本模型手工选择仍可使用；不默认模型、价格、积分池余额或自动测试，不写真实接入验收标记。远端会话、后台任务、hosted tools、legacy Completions、Embeddings 与媒体入口未开放。

本地合成测试只证明 Gateway 合同。真实 Key、订阅权限、实际 SSE 字节、费用／上游退款、浏览器视觉、原生包、Server／Redis 和完整产品验收另列。Gateway 本地预扣恢复不代表商汤返还上游消费。
