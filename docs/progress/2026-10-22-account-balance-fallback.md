# 上游余额拒绝的候选切换修复

真实请求 `2dfd14bd-916d-4f75-9abd-de3cffaa8939` 已尝试四个服务商，
最终 `custom-api-provider-6` 返回 HTTP 403、`bad_response_status_code`、
`Insufficient account balance`。并非本地余额或 ChatGPT 账号拒绝。

原逻辑按 403 分类为 Authentication/Abort，导致不能继续候选；即使只修改分类，
工具请求的发送预算也需要识别明确的余额 admission refusal。

本次以共享窄判定处理 HTTP 402/403 且完全匹配余额拒绝文案的上游错误：
分类为 InsufficientQuota，保留原 status/code/message；在已有外层 Next 边界允许
下一次候选发送，维持六次上限、截止时间、账号授权和响应交付后的防重放。
健康诊断增加 QuotaExhausted 识别，不将其误当成未知错误或无效凭证。
普通 403、内容拒绝、401/500 同文错误不获得该预算许可。

相关测试通过：pipeline_send_runtime_contract 31 项、request_budget_contract 12 项、
request_budget_rejected_channel 6 项；checker 31 项与有效行数 ratchet、diff 检查通过。
实际 loopback Responses 流覆盖 tools/tool_choice + 503 → 403 → response.completed，
每个上游只发送一次。未执行全量回归，未修改正式账号、数据库或当前实例。

新 release 目标目录为 gateway-account-balance-20261022-r4；构建及隔离 smoke 结果
以 linshi/gateway-account-balance-release-20261022 下的证据为准。
