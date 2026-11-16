# SAP AI Core 显式 Azure OpenAI Chat 部署合同

2026-11-10 匿名联网核实。此切片使用操作者自己的 SAP AI Core Service Key，经 `client_credentials` 获取短期 token，再调用显式配置的 Azure OpenAI Chat 部署。它不借用社区 client、浏览器会话或其他产品的订阅身份。

## 官方依据

- [Create a Service Key](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/create-a-service-key-7323ff4.md)：Service Key 可供第三方 API 平台及 curl 使用；client-secret 模式包含 `clientid`、`clientsecret`、`url`、`serviceurls.AI_API_URL`。x.509 是另一种认证模式，本切片未实现。
- [Use a Service Key](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/use-a-service-key-3a97465.md)：本地客户端及部署外实体可以使用自己的 Service Key；PowerShell 示例明确使用 HTTP Basic 与 form `grant_type=client_credentials` 请求 `<url>/oauth/token`。
- [Get an Auth Token](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/get-an-auth-token-0808d42.md)：响应返回 `access_token`、`expires_in`、`token_type`；示例的 7199 秒不是固定有效期。
- [OpenAI](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/openai-490f55d.md)：请求为 `<deploymentUrl>/chat/completions?api-version=...`，携带 Bearer token 与 `AI-Resource-Group`。部署选择模型，请求示例不含 `model`；旧示例使用 `2023-05-15`，o1/o3 示例使用 `2024-12-01-preview`。
- 当前官方 SDK [model-types.ts](https://github.com/SAP/ai-sdk-js/blob/main/packages/foundation-models/src/azure-openai/model-types.ts) 使用 `2024-10-21`；[Chat client](https://github.com/SAP/ai-sdk-js/blob/main/packages/foundation-models/src/azure-openai/azure-openai-chat-client.ts) 显式发送 `stream_options.include_usage=true`；[API spec](https://github.com/SAP/ai-sdk-js/blob/main/packages/foundation-models/src/azure-openai/spec/inference.json) 要求 `api-version`，支持 `tools`、`tool_choice` 和以 `[DONE]` 结束的 SSE。
- [Create a Deployment](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/create-a-deployment-b32e7a8.md)：先创建部署并确认运行状态，再使用返回的 `deploymentUrl`。本切片不创建、查询或自动选择部署。
- [SAP HANA Cloud remote source](https://help.sap.com/docs/hana-cloud-database/sap-hana-cloud-sap-hana-database-vector-engine-guide/create-remote-source) 给出 `api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com` 的官方 AWS origin 示例。本切片只接受已核实的 AWS hostname 模式，不声明覆盖所有 SAP 区域或云平台。

原文快照、抓取 UTC 和 SHA-256：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sap-aicore-20261110/contracts/sources.json`。参考链接的 main 会变化，核对本轮证据应使用已保存快照。社区 SAP 插件仅作为只读索引，未执行其代码，也未继承其模型版本筛选政策。

## Gateway 配置与凭据

| 配置 | 语义 |
| --- | --- |
| `preset: sap-aicore-chat` | 独立 SAP Chat 产品，不按模型品牌推断协议 |
| `base_url` | 完整部署 URL：`https://api.ai.prod.<region>.aws.ml.hana.ondemand.com/v2/inference/deployments/<id>` |
| `headers.AI-Resource-Group` | 显式资源组 |
| `extra_body.sap_api_version` | 显式 API 日期版本，可带 `-preview`；只用于 query，不进入推理正文 |
| 账号 `api_key` secret slot | 完整 Service Key JSON；沿用加密存储、grant、SecretPatch 和 revision/CAS |
| 支持模型 | 部署 ID；需要公开别名时使用既有 `model_map` |

一个 provider 绑定一个部署、资源组及版本。控制台创建空池，不设置默认模型、不触发付费探测或部署发现；添加账号后显示固定部署 ID。示例见 [无秘密的路由样例](examples/sap-aicore-chat.routes.sample.json)，样例中的区域、部署、资源组和版本必须按实际部署填写。

认证 URL 仅接受 `https://<tenant>.authentication.<region>.hana.ondemand.com`，不带查询、端口、用户信息或任意路径。Service Key 的 `AI_API_URL` 必须与部署同 origin，可带 `/v2`。同 origin 是本地实现的支持约束，不能理解为 SAP 保证所有部署拓扑如此。部署 URL 不接受重定向、编码路径、附加查询或路径逃逸。

完整 Service Key 上限 64 KiB。秘密替换需要现有敏感信息 grant；只改名称可 Keep，但 Keep 不允许改变产品、部署、资源组或版本。稳定账号 ID 保留，无数据库版本升级、无启动迁移，也不读取其他应用的凭据。

现有通用账号识别、模型目录及账号测试不适用此认证材料。本切片在 stored-account discovery、后台 discovery 配置、通用目录 URL 和通用探测入口拒绝 SAP；公共 header builder 也不将 Service Key 构造成 Bearer。2026-11-11 已补专属控制台显式 prompt 测试：使用同一 SAP sender 和 token owner，默认连通性仍不生成；不复用普通 Key 测试通道。详见 [控制台测试证据](../progress/2026-11-11-sap-aicore-console-probe.md)。

## 生命周期与协议边界

Token 缓存由 `UpstreamClient` 持有，最多 256 个条目，按完整密钥修订、部署和资源组隔离；同条目并发请求只交换一次 token。等待锁、交换与读取共同受 15 秒预算约束，响应最多 64 KiB。按服务器 `expires_in` 同时检查单调时钟和墙钟，提前至多 30 秒刷新；不落盘 token、不建后台刷新任务，不伪造 refresh token。推理失败不自动重放，包括 401 后重新生成。

公共 Chat、Messages、Responses 的文本及函数工具统一转换为 Chat wire，支持 JSON 与 SSE。发送前删除 Gateway 部署别名 `model`，仅携带本条 SAP token 和资源组，禁跳转及 ambient headers。不使用 Native Codex、Native Claude、XML 工具桥或普通 Key Bearer sender。

旧于 `2024-10-21` 的 API 版本不自动添加 `stream_options`；调用方显式设置的值保留。当前版本允许共享 Chat packer 请求 usage；没有 usage 时不编造 token 计数。版本兼容性最终由实际部署决定，本地格式校验不代表该版本已在 SAP 发布或该账号可用。

不支持的 reasoning 配置／历史、非文本内容、custom/hosted tools、远端会话状态、background、Embeddings、legacy Completions 和媒体在凭据外发前拒绝。原生 Responses、Bedrock、Orchestration、完整推理模型控制以及自动部署目录不在本切片内。

JSON 和 SSE 复用共享 Chat 完成校验，坏 JSON、缺终态、缺工具字段、错误尾部及截断不能记为完成。上游错误只返回固定本地诊断，终止同账号重试及候选回退；取消释放源连接，恢复 Gateway 本地预扣。该恢复不代表 SAP 退还已消耗的额度。

真实账号权限、有效部署、实际 token 生命周期、真实 SSE 与费用、桌面 EXE、Server/Redis、独立 feature 构建均需单列证据。不能把 manifest 登记或本地合成测试称为真实产品验收。
