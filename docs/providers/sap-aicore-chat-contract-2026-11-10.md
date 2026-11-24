# SAP AI Core 显式 Azure OpenAI Chat 部署合同

2026-11-10 匿名联网核实。此切片使用操作者自己的 SAP AI Core Service Key，经 `client_credentials` 获取短期 token，再调用显式配置的 Azure OpenAI Chat 部署。它不借用社区 client、浏览器会话或其他产品的订阅身份。

## 官方依据

- [Create a Service Key](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/create-a-service-key-7323ff4.md)：Service Key 可供第三方 API 平台及 curl 使用；client-secret 模式包含 `clientid`、`clientsecret`、`url`、`serviceurls.AI_API_URL`。X.509 是另一种认证模式，见下文。
- [Use a Service Key](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/use-a-service-key-3a97465.md)：本地客户端及部署外实体可以使用自己的 Service Key；PowerShell 示例明确使用 HTTP Basic 与 form `grant_type=client_credentials` 请求 `<url>/oauth/token`。
- [Get an Auth Token](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/get-an-auth-token-0808d42.md)：响应返回 `access_token`、`expires_in`、`token_type`；示例的 7199 秒不是固定有效期。
- [OpenAI](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/openai-490f55d.md)：请求为 `<deploymentUrl>/chat/completions?api-version=...`，携带 Bearer token 与 `AI-Resource-Group`。部署选择模型，请求示例不含 `model`；旧示例使用 `2023-05-15`，o1/o3 示例使用 `2024-12-01-preview`。
- 当前官方 SDK [model-types.ts](https://github.com/SAP/ai-sdk-js/blob/main/packages/foundation-models/src/azure-openai/model-types.ts) 使用 `2024-10-21`；[Chat client](https://github.com/SAP/ai-sdk-js/blob/main/packages/foundation-models/src/azure-openai/azure-openai-chat-client.ts) 显式发送 `stream_options.include_usage=true`；[API spec](https://github.com/SAP/ai-sdk-js/blob/main/packages/foundation-models/src/azure-openai/spec/inference.json) 要求 `api-version`，支持 `tools`、`tool_choice` 和以 `[DONE]` 结束的 SSE。
- [Create a Deployment](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/create-a-deployment-b32e7a8.md)：先创建部署并确认运行状态，再使用返回的 `deploymentUrl`。Gateway 不创建、修改或自动选择部署；部署查询见下文。
- [SAP HANA Cloud remote source](https://help.sap.com/docs/hana-cloud-database/sap-hana-cloud-sap-hana-database-vector-engine-guide/create-remote-source) 给出 `api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com` 的官方 AWS origin 示例。2026-11-22 另核实 [SAP Developer Center 教程第 6 步](https://developers.sap.com/tutorials/dev-garage-error-resolution-ai-adapter) 公布的 Azure EU20 AI Core origin：`https://api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com`。教程调用的是 Orchestration，仅作为该 origin 的官方依据；Chat 仍按 `foundation-models` 的部署 URL 与 OpenAI 合同调用。保留 AWS hostname 模式，仅新增这个 Azure origin，不推断其他区域或云平台。

原文快照、抓取 UTC 和 SHA-256：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sap-aicore-20261110/contracts/sources.json`。参考链接的 main 会变化，核对本轮证据应使用已保存快照。社区 SAP 插件仅作为只读索引，未执行其代码，也未继承其模型版本筛选政策。

## Gateway 配置与凭据

| 配置 | 语义 |
| --- | --- |
| `preset: sap-aicore-chat` | 独立 SAP Chat 产品，不按模型品牌推断协议 |
| `base_url` | 完整部署 URL：`https://<host>/v2/inference/deployments/<id>`；`host` 必须匹配既有 `api.ai.prod.<region>.aws.ml.hana.ondemand.com` 模式或精确等于 `api.ai.prod-eu20.westeurope.azure.ml.hana.ondemand.com` |
| `headers.AI-Resource-Group` | 显式资源组 |
| `extra_body.sap_api_version` | 显式 API 日期版本，可带 `-preview`；只用于 query，不进入推理正文 |
| `extra_body.sap_image_input` | 可选布尔值，默认关闭；只在配置为 `2024-10-21` 且操作者确认部署支持视觉时启用，不进入推理正文 |
| `extra_body.sap_o3_mini` | 可选布尔值，默认关闭；操作者确认部署为 o3-mini 且 API 版本为 `2024-12-01-preview` 时显式启用，不进入推理正文 |
| `extra_body.sap_native_responses` | 可选布尔值，默认关闭；只在操作者确认当前绑定支持原生 Responses 时启用，仅改变公共 Responses 入口 |
| `extra_body.sap_native_responses_image_input` | 可选布尔值，默认关闭；仅在部署支持原生 Responses 图片且已开启 `sap_native_responses` 时启用 |
| `extra_body.sap_stored_responses` | 可选布尔值，默认关闭；依赖 `sap_native_responses`，须确认目标部署的持久化资格及[恢复限制](sap-stored-responses.md) |
| 账号 `api_key` secret slot | 完整 Service Key JSON；沿用加密存储、grant、SecretPatch 和 revision/CAS |
| 支持模型 | 部署 ID；需要公开别名时使用既有 `model_map` |

一个 provider 绑定一个部署、资源组及版本。控制台创建空池，不设置默认模型，也不会随创建动作触发探测或查询；添加账号后显示固定部署 ID。示例见 [无秘密的路由样例](examples/sap-aicore-chat.routes.sample.json)，样例中的区域、部署、资源组和版本必须按实际部署填写。

认证 URL 仅接受 `https://<tenant>.authentication.<region>.hana.ondemand.com`，不带查询、端口、用户信息或任意路径。Service Key 的 `AI_API_URL` 必须与部署同 origin，可带 `/v2`。同 origin 是本地实现的支持约束，不能理解为 SAP 保证所有部署拓扑如此。部署 URL 不接受重定向、编码路径、附加查询或路径逃逸。

完整 Service Key 上限 64 KiB。秘密替换需要现有敏感信息 grant；只改名称可 Keep，但 Keep 不允许改变产品、部署、资源组或版本。稳定账号 ID 保留，无数据库版本升级、无启动迁移，也不读取其他应用的凭据。

现有通用账号识别、模型目录及账号测试不适用此认证材料。本切片在 stored-account discovery、后台 discovery 配置、通用目录 URL 和通用探测入口拒绝 SAP；公共 header builder 也不将 Service Key 构造成 Bearer。2026-11-11 已补专属控制台显式 prompt 测试：使用同一 SAP sender 和 token owner，默认连通性仍不生成；不复用普通 Key 测试通道。详见 [控制台测试证据](../progress/2026-11-11-sap-aicore-console-probe.md)。

## X.509 Service Key

2026-11-22 复核固定快照 `8fc8e1de` 的 SAP [创建 Service Key](https://github.com/SAP-docs/sap-artificial-intelligence/blob/8fc8e1de82fcb34766e1867e83640efc4d0bb93e/docs/sap-ai-core/create-a-service-key-7323ff4.md) 和 [使用 Service Key](https://github.com/SAP-docs/sap-artificial-intelligence/blob/8fc8e1de82fcb34766e1867e83640efc4d0bb93e/docs/sap-ai-core/use-a-service-key-3a97465.md)。同一 `api_key` secret slot 可导入其中一种完整 JSON：原有 `clientsecret` 模式，或 `clientid`、`certurl`（也接受 `certUrl`）、`certificate`、`key` 和 `serviceurls.AI_API_URL` 的 X.509 模式。拒绝同时声明两种认证材料或重复声明证书 URL。

- X.509 仅允许 `https://<tenant>.authentication.cert.<region>.hana.ondemand.com` 根地址，遵循 SAP XSUAA 的 [证书端点命名](https://help.sap.com/docs/btp/sap-business-technology-platform/authorization-and-trust-management-rate-limiting)。如 Service Key 还带 `url`，它必须是相同租户和区域的普通认证地址；不使用该地址换 Token。`AI_API_URL` 与 client-secret 模式使用相同的 AWS／Azure EU20 origin 规则，且仍必须与部署同源。
- 证书字段使用 PEM（可含证书链，叶证书在首位）；私钥字段接受明文 RSA PKCS#1 或 PKCS#8 PEM。导入检查 PEM 形状；实际握手仍校验证书、私钥与服务端域名，不将导入成功视为凭据有效。
- 使用一次性直连 HTTPS 客户端，只对校验过的 `<certurl>/oauth/token` 出示证书。表单为 `grant_type=client_credentials` 和 `client_id`，不发送 Basic，不经过系统代理，不跟随重定向，不自动重发。目录、探测及推理只携带取得的短期 Bearer，不转发证书或私钥。
- 替换时粘贴新完整 Service Key，沿用原账号、SecretPatch 和 CAS；证书到期与签发仍由租户管理员管理。旧 `clientsecret` 无需迁移，轮换后不借用旧证书的缓存 Token。

## 只读部署发现与手动绑定

2026-11-22 核实 SAP [Deployment API](https://sap.github.io/ai-sdk/java-api/v2/com/sap/ai/sdk/core/client/DeploymentApi.html)、固定版本的 [deployment resolver](https://github.com/SAP/ai-sdk-js/blob/3b791d54/packages/ai-api/src/utils/deployment-resolver.ts) 与 [模型提取](https://github.com/SAP/ai-sdk-js/blob/3b791d54/packages/ai-api/src/utils/model.ts)。在新建 SAP 服务商时，填资源组并取得敏感信息授权后，可以粘贴临时 Service Key 进行查询。

- 管理接口 `POST /v1/internal/gateway/console/sap-aicore-deployments` 接收 `serviceKey`、`resourceGroup` 与可选的 `protocol`（`azure`、`bedrock` 或 `orchestration`，省略为 `azure`）。仅向 Key 内经校验的认证地址交换短期令牌，再以 `AI-Resource-Group` 调用同 Key 的 `GET /v2/lm/deployments`，Azure 与 Bedrock 固定查询 `foundation-models`、`RUNNING`，executable 分别为 `azure-openai` 和 `aws-bedrock`。Orchestration 固定查询 `orchestration`、`RUNNING`，不指定 executable。调用者不能指定额外路径、origin、场景或 executable。没有推理、部署写入或自动重放。
- 发现与推理共用已校验的 Key 和令牌交换实现，但查目录不读写推理缓存。令牌交换与列表共用 15 秒预算；最多 5 页，每页最多 50 项、512 KiB。达到上限时返回 `truncated`。不跟随重定向，不回传上游正文、Service Key 或令牌。
- 两种目录均要求条目的部署 ID 与 URL 匹配，且 URL 与 Key 同 origin、场景和 executable 与请求匹配、状态为 `RUNNING`。Azure 目录另外要求有已知 Chat 模型元数据，接受 `gpt-4o`、`gpt-4o-mini`、`gpt-4.1`、`gpt-4.1-mini`、`gpt-4.1-nano`；同 executable 下的 Embeddings、未识别、缺失元数据的条目不自动列入。列表不是模型可用性或推理能力证明。
- SAP 官方[场景目录](https://help.sap.com/docs/ai-launchpad/sap-ai-launchpad/models-and-scenarios-in-generative-ai-hub-fef463b24bff4f44a33e98bb1e4f3148?locale=en-US)将 `aws-bedrock` 列为 `foundation-models` 的 executable。Bedrock 目录只证明其部署类型和运行状态，不证明支持 Converse；按可用元数据展示模型名、配置名或仅部署 ID。模型详情不是候选资格条件。不查询额外详情、不探测推理，不自动开启任何模型能力。
- 目录不会默认选择。用户选择后只填部署 URL，Azure 仍需显式填写 API 版本，Bedrock 无需填写 Azure 的 API 版本。模型版本不填入 `sap_api_version`；上游模型 ID 仍是部署 ID。切换资源组或部署类型会取消进行中的查询、清掉旧结果和来自旧选择的 URL。仍可手填，并按现有 CAS 流程创建空池。
- 查询用的 Service Key 不写入路由草稿，不随空池持久化。需要实际调用时，仍在账号对话框独立录入凭据，经原有 SecretPatch 和 CAS 保存；Keep 的绑定限制不变。

## 生命周期与协议边界

Token 缓存由 `UpstreamClient` 持有，最多 256 个条目，按完整密钥修订、部署和资源组隔离；同条目并发请求只交换一次 token。等待锁、交换与读取共同受 15 秒预算约束，响应最多 64 KiB。按服务器 `expires_in` 同时检查单调时钟和墙钟，提前至多 30 秒刷新；不落盘 token、不建后台刷新任务，不伪造 refresh token。推理失败不自动重放，包括 401 后重新生成。

未启用原生 Responses 时，公共 Chat、Messages、Responses 的文本及函数工具统一转换为 Chat wire，支持 JSON 与 SSE。发送前删除 Gateway 部署别名 `model`，仅携带本条 SAP token 和资源组，禁跳转及 ambient headers。不使用 Native Codex、Native Claude、XML 工具桥或普通 Key Bearer sender。

旧于 `2024-10-21` 的 API 版本不自动添加 `stream_options`；调用方显式设置的值保留。当前版本允许共享 Chat packer 请求 usage；没有 usage 时不编造 token 计数。版本兼容性最终由实际部署决定，本地格式校验不代表该版本已在 SAP 发布或该账号可用。

未开启的图片、其他非文本内容、除下述 o3-mini Chat 参数外的 reasoning 配置／历史、custom/hosted tools、远端会话状态、background、Embeddings、legacy Completions 和媒体输出在凭据外发前拒绝。原生 Responses 和独立的 Orchestration v2 Chat 模式见下文；Bedrock 与完整推理模型控制不在 Azure OpenAI Chat 协议范围内。

JSON 和 SSE 复用共享 Chat 完成校验，坏 JSON、缺终态、缺工具字段、错误尾部及截断不能记为完成。上游错误只返回固定本地诊断，终止同账号重试及候选回退；取消释放源连接，恢复 Gateway 本地预扣。该恢复不代表 SAP 退还已消耗的额度。

真实账号权限、有效部署、实际 token 生命周期、真实 SSE 与费用、桌面 EXE、Server/Redis、独立 feature 构建均需单列证据。不能把 manifest 登记或本地合成测试称为真实产品验收。

## 显式开启内嵌图片输入

2026-11-22 复核 [SAP OpenAI 文档](https://github.com/SAP-docs/sap-artificial-intelligence/blob/8fc8e1de82fcb34766e1867e83640efc4d0bb93e/docs/sap-ai-core/generativeAI/openai-490f55d.md)、[SAP SDK 2024-10-21 wire](https://github.com/SAP/ai-sdk-js/blob/3b791d54/packages/foundation-models/src/azure-openai/spec/inference.json) 和 [Microsoft Vision 示例](https://learn.microsoft.com/en-us/azure/ai-services/openai/how-to/gpt-with-vision)。SAP 文档示例使用 GPT-4o 系列的 Chat 用户图片块，SDK schema 声明 `image_url.url` 可承载 Base64 图像数据，Azure 示例使用 Base64 data URL。Gateway 仅实现这里列出的子集，不把其他模型或版本宣称为已适配。

- 在创建 SAP 服务商时，选择或手填已确认支持视觉的 GPT-4o 部署、填写资源组，并使用 `2024-10-21`，再勾选“启用 GPT-4o 图片输入”。手动配置为 `extra_body: { sap_api_version: "2024-10-21", sap_image_input: true }`。账号级配置不得覆盖图片开关；部署列表或模型别名本身不能启用。未声明的旧配置不变，不迁移数据库或秘密。
- 公共 Chat 接受用户 `image_url`，Messages 接受用户 `image.source.type=base64`，Responses 接受用户 `input_image.image_url`。已接收的图片与文本按原顺序转换为 Chat 用户内容；Chat／Responses 的 `auto`、`low`、`high` 保留。该转换不代表 SAP 已接通原生 Messages 或 Responses。
- 只接收内嵌 PNG／JPEG 的标准 Base64：最多 10 张、累计最多 4 MiB Base64 文本。Gateway 检查编码和内容块，不解码像素；图片有效性由已选部署验证。远程 URL、文件 ID、其他 MIME、非用户图片、无法保真的图片扩展及音频输出在 OAuth 前拒绝。不会替客户端取图。Gateway 现金额度仍使用现有仅文本预留规则。
- 见 [图片输入本地验收](../progress/2026-11-22-sap-chat-image-input.md)。

## 显式开启 o3-mini Chat 推理参数

2026-11-22 核对 [SAP OpenAI Chat 示例](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/openai-490f55d.md)、[SAP 参数文档](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/supported-parameters-55d2197.md)及 [Microsoft Azure OpenAI reasoning 文档](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/reasoning)。在控制台手填已核实的 o3-mini 部署 URL 和资源组，填写 `2024-12-01-preview`，选择“启用 o3-mini 推理参数”。旧配置默认不变，不根据部署 ID 或公开别名猜测底层模型，不自动生成。Local 路由文档的账号级配置不得覆盖这个部署能力。Hosted 合并时，`sap_o3_mini` 和 `sap_image_input` 只承认服务商账号层的声明；仅当最终 SAP 部署地址、资源组和 API 版本仍与该层一致时才应用。凭据层不能添加、关闭或把声明转移到另一部署绑定。普通 SAP Chat 路由仍可使用既有凭据覆盖配置。

- 公共 Chat 的 `reasoning_effort` 只接收 `low`、`medium`、`high`；未提供则由部署选择默认值。`max_tokens`、`max_completion_tokens`、`max_output_tokens` 表达的单个正整数上限映射成 `max_completion_tokens`，同时限制可见内容和推理 tokens；拒绝冲突或无效上限。普通 Chat 池保持原线协议。
- 不静默丢弃显式不兼容的采样控制。Messages／Responses 的文本桥接仍走 Chat，可以使用各自限制字段，但未接通其原生思考控制、历史、推理摘要或原生 Responses endpoint。本切片不保证其他推理模型、其他日期版本或实际租户的可用性。

## 显式开启原生无状态 Responses

2026-11-22 复核 [SAP 支持的调用方式](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/supported-consumption-methods-47eb0c7.md)、[Java SDK 无状态 JSON／流式示例](https://sap.github.io/ai-sdk/docs/java/foundation-models/openai/responses)和 [Java SDK 固定版本请求夹具](https://github.com/SAP/ai-sdk-java/blob/rel/1.22.0/foundation-models/openai/src/test/resources/mappings/createResponse.json)。静态部署调用使用 `POST <deploymentUrl>/responses` 与 `AI-Resource-Group`，不拼接 Chat 的 `api-version`。SDK 的无状态请求不需要 `model`；Gateway 不把本地部署 ID 或调用者模型别名发给 SAP。SAP 示例不显式关闭存储时返回 `store:true`，因此 Gateway 始终发送 `store:false`。

- 对已经确认支持原生 Responses 的部署，在创建服务商时勾选“启用原生 Responses”。手工路由使用 `extra_body: { sap_api_version: "2024-10-21", sap_native_responses: true }`；版本仍用于同池 Chat 与配置绑定。只读目录返回的模型名或部署 ID 不证明原生能力。修改部署地址、资源组或版本会清除表单勾选，不因恢复旧值自动复选。Local 的凭据配置不得改变该开关或绑定；Hosted 仅接受账号层声明，并检查合并后绑定一致。
- 仅公共 `/v1/responses` 走本原生路径。公共 Chat／Messages 和未启用原生的旧池仍按 Chat 合同执行。原生子集包含文本、函数工具与结果、`reasoning` 及带 `encrypted_content` 的无状态推理历史；由客户端在下一轮重送所需历史。无状态模式拒绝 `store:true`、后台任务、远端响应／会话／item／文件引用和内置工具。显式开启的持久化部署使用[独立生命周期合同](sap-stored-responses.md)，不改变无状态默认。图片输入须按下节独立开启；其他媒体仍不支持。
- JSON／SSE 保留原生输出与扩展，公开模型名使用调用者可见别名。SSE 不依赖 Chat 的 `[DONE]`，验证匹配的 Responses 终态并使用终态的合法用量；畸形、缺失终态与取消不记为成功，不重放生成。沿用同一 SAP 服务凭据、固定部署、超时和本地预扣恢复策略。

## 显式开启原生 Responses 内嵌图片

2026-11-22 对照 [SAP Responses API](https://help.sap.com/docs/sap-ai-core/generative-ai/responses-api) 指向的 Azure OpenAI 合同及 [Microsoft 原生 Responses 图片示例](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/responses#base64-encoded-image)。这些资料描述接口形状，不证明某个 SAP 租户、模型或部署已启用图片能力。

- 在确认当前部署支持原生 Responses 图片后，先启用“原生无状态 Responses”，再选择“启用原生 Responses 图片输入”。手工配置经 `extra_body.sap_native_responses_image_input: true` 开启；不从 `sap_image_input`、模型别名或目录结果推断。设置只来自 SAP 服务商配置，凭据不能新增或覆盖；变更部署、资源组或版本，以及关闭原生 Responses，须重新确认。
- 公共 `/v1/responses` 的 `user` 消息可包含 `type: input_image` 与 `image_url`。支持内嵌 PNG／JPEG 标准 Base64，`detail` 可省略或使用 `auto`、`low`、`high`。整个请求最多 10 张、累计最多 4 MiB Base64 文本；Gateway 校验编码，不解码像素。合法块保持原顺序走原生 `/responses`，并继续强制 `store:false`。
- 远程图片 URL、文件 ID、非用户图片、额外图片属性和其他媒体在 OAuth 前拒绝。Chat、Messages 及旧池的文本／图片桥接行为不变。真实 SAP 部署能力、费用和产品出包另行验收。

## Orchestration v2 Chat 模式

- 在已确认的 Orchestration 部署上，手填完整部署根 URL 和资源组，选择“使用 Orchestration v2”，填写 SAP 模型名。路由 API 对应 `extra_body: { sap_orchestration_model: "<SAP 模型名>" }`。不设置 `sap_api_version`，不与 Chat 图片、o3-mini 或原生 Responses 开关并用。现有 Azure OpenAI 部署无需迁移。模型版本未固定，由 SAP 选择；不能从 Gateway 对外模型别名切换 SAP 模型。
- `/v1/chat/completions` 接受字符串文本历史。纯文本轮次接受 `system`、`user`、`assistant` 角色，可用单个 `max_tokens` 或 `max_completion_tokens`。转换为 SAP `messages_history`、固定模型和可选 `max_completion_tokens`，调用 `<deploymentUrl>/v2/completion`。不会解释聊天内容中的模板占位符；不会透传调用者的工作流配置、`user` 标识或外部凭据。
- 仍使用相同的 Service Key、部署 origin 校验和短期 Token。对外只使用 `final_result` 中经校验的 Chat 结果和用量，不将 `intermediate_results` 或失败详情返回给调用方，也不因缺少终态降级或重放。修改部署地址或资源组须重新填写模型，凭据不能覆盖 Orchestration 模式或模型，保留密钥不能默许变更绑定模型。
- `stream:true` 使用 SAP `config.stream.enabled`。公共 `stream_options` 可省略，也可指定 `{"include_usage":true}`；显式开启用量会映射到 `model.params.stream_options`。此切片不支持关闭用量或传 SAP 模块流控制参数。SSE 只输出 `final_result` 的单 choice 文本／拒绝内容、原生函数工具片段及有效终态用量；需要合法 finish、`[DONE]` 和正常 EOF，输出模型名保持 Gateway 别名。未返回用量时不会用中间结果伪造用量。见 [SSE 实现与验收状态](../progress/2026-11-22-sap-orchestration-v2-sse.md)。
- Chat 支持标准 `tools[].function` 声明、`assistant.tool_calls` 历史和 `role:tool` 文本结果。声明函数时末条消息必须是 `user` 或 `tool`；声明放入 `config.modules.prompt_templating.prompt.tools`，末条消息内容作为固定占位符的值，原顺序与其他历史保留。工具参数字符串不执行、不修正；Gateway 不执行函数。仅接受结构匹配、由 `tool_calls` 完成的原生调用；没有声明函数时不返回新的工具调用。
- 确认选定模型支持图片后，可在池上显式启用 `sap_orchestration_image_input: true`。仅公共 Chat 的 `user` 消息可把内嵌 PNG／JPEG 与文本按原顺序混排；保留 `detail`，每请求最多 10 张、合计最多 4 MiB base64。拒绝远程 URL，不由 Gateway 或 SAP 代为抓图；不支持其他媒体。带函数声明时最后一条图文消息保留模板块形状，文本与 data URL 作为独立的固定占位符值传递。切换部署地址、资源组或模型时，控制台清除图片选择；凭据不能授予或覆盖该能力。上游格式参见 [SAP Orchestration 图像合同](https://sap.github.io/ai-sdk/docs/js/orchestration/chat-completion#image-recognition)。
- 确认选定模型支持推理后，可在池上独立启用 `sap_orchestration_reasoning: true`（默认关闭）。公开 Chat 的 `reasoning_effort` 只接受 `none|minimal|low|medium|high|xhigh`，映射至 `config.modules.prompt_templating.model.params.reasoning_effort`，不套用 Azure o3-mini 的档位限制；实际可用档位因 SAP 模型而异。未启用或参数无效时在 OAuth 前拒绝。控制台在变更部署、资源组、模式或模型时清除勾选；凭据不能创建或覆盖该能力。JSON 消息及 SSE 增量只投影 `final_result` 的合法 `reasoning_content` 块；客户端续轮应原样重送助手消息中的 `content`／`signature` 块。Gateway 在恢复推理块前核对原始与当前的非 system 历史，不以原始请求覆盖经过处理的历史。见 [SAP Orchestration v2 推理示例](https://sap.github.io/ai-sdk/docs/python/examples/orchestration-service2)及 [ReasoningBlock](https://sap.github.io/ai-sdk/api/v2/types/orchestration_src.ReasoningBlock.html)。
- `tool_choice`、`parallel_tool_calls`、custom／hosted tools、Messages／Responses 转接、调用者工作流模板和原生 Bedrock 不在本次范围；本切片不自行创建部署。控制台也可显式发起只读 Orchestration 查询：按 [SAP SDK 固定版 resolver](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/packages/ai-api/src/utils/deployment-resolver.ts) 的场景和状态过滤，仅列出同 origin、ID 与 URL 匹配的运行中部署；可显示配置名，不从元数据推断可选模型。显式选中只填写 URL，操作者仍需输入模型；切换协议或资源组时不复用旧结果。临时 Service Key 不随空池保存。见 [非流式开发回执](../progress/2026-11-22-sap-orchestration-v2.md)。

## SAP 托管的 Bedrock Converse

操作者显式绑定 `foundation-models` 场景下支持 Converse 的 `aws-bedrock` 部署，可从只读目录选取或手填 URL。账号池 `extra_body` 基础配置为
`{"sap_bedrock_converse": true}`；与 Azure Chat/Responses 的 `sap_api_version`、Orchestration
的模型声明互斥。不要按模型名猜协议；目录不自动选择协议或确认 Converse 资格。Service Key、部署 URL、
资源组、凭据 Keep 限制与无重放策略保持不变。详见 [Bedrock Converse 文本切片](../progress/2026-11-22-sap-bedrock-converse.md)。

- 同一部署的 `/converse` 与 `/converse-stream` 支持公共 Chat 文本及客户端函数工具。函数 schema 顶层为 `object`；
  显式 `strict` 映射到 `toolSpec.strict`，是否支持取决于实际部署。`tool_choice` 可省略或用 `auto`、`required`、
  已声明的指定函数。Converse 没有等价的 `none`；此切片不把它降级为 `auto`。不接收 `parallel_tool_calls` 开关。
- 客户端执行函数，将助手的原始调用及对应 `role:tool` 文本结果发回。Gateway 匹配 ID、验证参数为无重名键的 JSON 对象，
  并生成原生 `toolUse`／`toolResult`；不会执行工具，也不会把 JSON 字符串结果改为结构化结果。查询内容与工具参数不进入错误诊断。
- 流式参数增量保持公开工具索引；终态与用量等到合法关闭及干净 EOF 后发布，不输出内部 relay／continuation。
  服务器侧／自定义工具、reasoning 和原生 Bedrock 入站接口不在范围内。见[函数工具回执](../progress/2026-11-22-sap-bedrock-converse-tools.md)。
- 对已确认支持图片的 Converse 部署，可在账号池额外显式设置 `sap_bedrock_image_input: true`。默认关闭；只读目录不推断图片能力，切换部署、资源组或协议会清除此选择，Hosted 凭据不能覆写。
- 仅公共 Chat 的 `user` 消息接受内嵌 PNG／JPEG data URI（可与文本混排，`detail` 仅允许省略或 `auto`）。每次请求最多 10 张，Base64 编码内容合计不超过 4 MiB。Gateway 验证编码并按原顺序发送 `image.format` 和 `image.source.bytes`，不解码像素，不访问远程图片 URL 或 S3。未启用或请求形状不受支持时，在 OAuth 和推理前拒绝。图片输出不在此合同内。
