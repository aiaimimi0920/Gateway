# OpenAI API 模板验收（2026-10-27）

## 结论与授权边界

`openai-api` 按用户明确允许的“余额受限但调用链正确”标准完成，已登记模板完成标记。
内部 template ID / preset 保持 `openai`，不迁移既有服务商或数据库。
`generationPassed=false`：没有成功生成文本，也没有成功建立 SSE 输出；不得将模板标记解释成账号可用或模型生成通过。

## 改动

- 显示名和新建默认 Provider ID 改为 `openai-api`；固定官方 Base URL、OpenAI API 调用适配器。
- 新建空池无账号、模型、路由；移除预置模型，自动测试关闭，手动测试仅选 `gpt-4o-mini`，无标准答案。
- 目录接入固定 `https://api.openai.com/v1/models`，管理认证、secret grant、精确账号及 revision 校验复用既有 owner；禁止重定向，限制响应大小和超时。
- 保留旧 OpenRouter 地址与 `preset: openai` 的兼容，不把目录导入当生成验证。
- 实测发现余额 HTTP 429 被误归类限流，重试后被本地 circuit 503 掩盖。修正精确额度 code 为 `InsufficientQuota`，保留原始 429/code，不再同账号重试。
- 覆盖 `insufficient_quota`、`credit_balance_exhausted`、organization/project spend limit、organization usage limit；其他限流及 401 不误归类。
- 官方错误码文档已联网核对：<https://developers.openai.com/api/docs/guides/error-codes>。

## 正常 UI 流程

隔离后端 `127.0.0.1:4300`，源码 UI `127.0.0.1:1435/ui/`。
UI 新建空服务商 revision `r1-19b07b1be7d1`：0 账号、0 模型、0 路由。
正常新增账号 `openai-acceptance-20261027`，使用用户提供的 Key 拉取官方目录，导入 128 项模型并保存
revision `r2-ed16cc4ec2cf`；只有该服务商和该账号，无其他凭据回退。
导入阶段无生成审计、无 discovery job；Key 格式检查不是完整字符串相等验证。

UI 精确账号测试于 `2026-10-27T04:22:16.3922512Z` 返回真实 HTTP 429，status failed、callable false、score null；
审计 `probe-7b4f6d4b-c37d-4904-a608-d22fa30402ab` 的凭据归属正确。没有为了完成标记改写失败状态。

## 最终 r3 普通调用

全部仅调用 `gpt-4o-mini`，没有批量调用目录中的其他 127 个模型。

| 模式 | 实际结果 | request ID | audit ID |
| --- | --- | --- | --- |
| Chat Completions 非流式 | HTTP 429 / `credit_balance_exhausted` / `InsufficientQuota`，3234 ms | `1602a9ef-f6ef-45cf-88ad-1fc4cbba9ba0` | `gw-audit-cea3019a-f92d-4d00-8037-e425f0a0f26a` |
| Chat Completions `stream:true` | HTTP 429 JSON / 同上，1078 ms；建立 SSE 前被上游拒绝 | `e3e430c6-7508-41b2-9c69-c19ac0c37648` | `gw-audit-f0abb9ff-6dac-466d-8423-7d62063e1738` |

两次均为单 candidate、单 route attempt、上述精确账号。上游说明无剩余 credits，需要充值；不属于认证失败。
最终审计 errorKind 为 `InsufficientQuota`，既有健康层 failureClass 仍可为 `rate_limited`，本轮没有宣称全健康分类统一。

保留全部 8 条审计：旧非流式 503 两条、UI 429 一条、r2 stream 429 一条、r3 buffered 429 一条、
r3 stream 本地无 candidate 的 400 两条、最终 r3 stream 429 一条。两条 400 未到上游。
调度开关快速关/开曾被保存队列合并；后通过正常 UI 分两次保存并读取持久 revision，确认关闭
`r9-d6db02f3a62b`、开启 `r10-ed16cc4ec2cf` 后才进行最终请求。未直接改 SQL、未清审计。
最终失败/冷却状态保持真实，不再次清除，不再发送真实请求。

## 构建、检查与证据

本轮使用官方 devtest API 构建，非正式发布、非桌面整包。r3 构建 7m16s，来源前后匹配。

- EXE SHA256：`4d4686cc40aa88d7c6b54bb835fa01219e3ba95f58a23619e477fccf2ff6227e`
- source fingerprint：`716c75494b2f97c911876b47950ad76b725117ad5c74ba0d27b3b55276ef1f1d`
- builtAt：`2026-10-27T04:45:03.9829193+00:00`
- 前端相关 4 文件 30 tests passed，TypeScript typecheck 通过。
- Rust catalog 定向检查 48 passed；最终 error tests 39 passed。
- 有效行 checker 31 passed，ratchet 通过；既有 debt 未借本轮扩张。
- 完成标记为验收后追加的 UI 源码，不声称已部署到用户当前 4200 桌面实例。

证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-openai-template-acceptance-20261027/`。
包含 `ui-empty-pool.json`、`ui-imported-account.json`、`ui-import-no-generation.json`、
`ui-test-results.json`、`audits-sanitized.json`、`chat-completions-buffered-r3.json`、
`chat-completions-stream-r3-admitted.json`、`integrity.json` 及 r3 构建凭据。
凭据、原始响应及独立运行数据仅保留在 private，不写入本文或源码。SQLite integrity 为 ok。
现有 4200 实例、正式 `.ng` 与现有 `.ng-devtest-4300` 配置不变。

未验：成功生成、SSE 文本与终止事件、Responses、其他目录模型、桌面整包及长期稳定性。
未提交、未推送、未部署到现有实例。

收尾：最终 rustfmt 定向检查、`git diff --check` 通过；源码 UI 重启后实际确认模板蓝色完成标记，
截图为 `ui-template-complete.jpg`。4300 后端经正常 drain 退出，1435 自建 helper 与验收标签关闭，
两端口已释放；4200 原监听 PID 55776、4280 保持不变。独立数据和全部证据保留，见 `cleanup.json`。
