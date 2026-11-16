# Agnes AI 国际站 Key 接入合同

本实现承接国际站公开 API Key，包括操作者从控制台明确创建的订阅 Key。
账号拥有订阅并不意味着普通 Key 自动使用订阅额度；Gateway 不按 Key 前缀猜套餐、余额或价格。
Agnes Code、国内站账号、网页登录与套餐额度读取仍为独立范围。

## 公开合同与证据边界

2026-11-10 匿名联网复核以下官方文档，未使用真实凭据或调用模型：

- [Quickstart](https://wiki.agnes-ai.com/en/docs/quickstart.md)：
  `Authorization: Bearer YOUR_API_KEY` 与国际站 Chat 示例。
- [Agnes 3.0 Flash](https://wiki.agnes-ai.com/en/docs/agnes-30-flash.md)：
  固定 Base `https://apihub.agnes-ai.com/v1`，`POST /v1/chat/completions`、`POST /v1/responses`。
- [Theta](https://wiki.agnes-ai.com/en/docs/agnes-25-pro-theta.md)：
  Responses 正文从 `output[]` 读取，不要求顶层 `output_text`；用量字段存在两类命名。
- [订阅说明](https://wiki.agnes-ai.com/en/docs/tokenplan.md) 与
  [FAQ](https://wiki.agnes-ai.com/en/docs/faqs.md)：普通、企业和 Token Plan Key 的限额池独立，
  同类多个 Key 不叠加套餐限额。当前文档同时介绍 Agent Plan 与保留的 Token Plan，不能混为同一计费方案。
- [图像](https://wiki.agnes-ai.com/en/docs/agnes-image-25-flash.md) 与
  [视频](https://wiki.agnes-ai.com/en/docs/agnes-video-25-flash.md) 使用独立媒体端点。

`GET /v1/models` 是当前候选目录读取路径。本次官方索引未给出独立 HTTP Models API 合同；
匿名请求返回 401 `Token not provided`，不能据此证明成功响应格式、模型权限或推理可用性。
本地实现按 OpenAI `data[].id` 读取、限流限量、拒绝跳转，真实 Key 目录读取仍待验收。
目录失败时保留已有文档；操作者仍可按已知权限手工选模型，不写入猜测的默认模型。
排除 `agnes-image-`／`agnes-video-` 只过滤已知媒体命名空间，不是未知模型的文本能力证明。

## Gateway 行为

- 模板 `agnes-api`、profile `agnes_api`、feature `line-agnes-official-api`，使用现有调用适配框架。
- 固定国际站目标与 Bearer Key；禁止账号覆盖目标、改身份、附加 Cookie/任意 headers 或伪造 refresh。
  空账号池允许保存模板，空 Key 不允许外发；最终发送关闭跳转并清除环境默认身份头。
- 公共 Chat 与 Responses 使用对应上游端点；公共 Messages 由共享协议 owner 桥接到 Chat。
  这是本实现的选择，不表示 Agnes 官方没有原生 Messages。
- Responses JSON 保留原生正文及 `incomplete_details`，审计兼容 `input_tokens/output_tokens` 与
  `prompt_tokens/completion_tokens` 两套用量名称。非空 error、失败/取消、非终态或缺少 output 数组
  会明确失败、退还 Gateway 本地预扣且不自动重放；这不代表 Agnes 上游会返还费用。
- 支持文本、流和函数工具；本切片拒绝远端状态引用、后台任务、hosted tools、legacy Completions、
  Embeddings 和媒体端点，不将未实现操作静默降级为文本成功。
- 目录读取要求管理身份和敏感信息 grant，按明确账号或明确新 Key 读取，只返回元数据。
  应用表单后走正常 revision/CAS 保存；陈旧 revision、目标变更及跨产品 secret keep 仍受原 owner 保护。
- 额度、错误重试、取消退款、账号选择和审计使用现有 Gateway owner。无新增数据库版本或外部服务。

本地测试、真实账号调用、原生包、浏览器视觉与完整产品验收分别记录；manifest 的能力字段表示实现入口，
inventory 继续保留 `metadata_only`，不把本地合成测试标成真实账号接入通过。
