# magpie 服务商对照与第一批原生接入

盘点日期：2026-11-08。本文记录最初三张插件截图；后续五张红框的 23 张第三方卡片另见
[第三方来源与订阅顺序](magpie-third-party-2026-11-08.md)。不是将 magpie 插件直接装进 Gateway。
本文区分代码接入、离线合同、真实账号调用和正式发布；任何一项都不能替代下一项。

## 来源与实现方式

- magpie 主仓固定到 [627fd1a3](https://github.com/yetone/magpie/tree/627fd1a3b78c65e9f3b7a838dcd2b24bb368c93d)。
- 主仓关联的社区插件库固定到 [93dacd73](https://github.com/magpie-community/plugins/tree/93dacd738f2c35981cf0364ad1bfbfe7b427a78a)。这是社区实现，不是各厂商官方 SDK。
- 参考了插件的产品身份、认证方式和协议差异，没有安装/执行插件，没有引入 Bun、AI SDK 或独立代理进程。
- 本批复用 Gateway 的 preset、implementation line、模型目录、SecretPatch、call flow、HTTP sender、SSE 和账号探测 owner；没有重写 auth → filter → route → send → finalize。
- River、Cloudflare 以各自官方文档复核协议。没有复制社区的静态模型价格、免费额度推测或默认模型表。

## 本轮前的 Gateway 状态

控制台原有 46 个入口（45 个命名预设和 1 个 custom）。入口数量不是“服务商验收通过”数量。
`providerTemplateAdaptation.ts` 的 26 条 receipt 是不同范围的证据记录，不是 26 家都真实生成成功：

| 范围 | 现有 receipt |
| --- | --- |
| 已登记调用/适配证据，仍以每条 receipt 的具体边界为准 | NVIDIA、OpenRouter、You、Exa、ChatGPT Codex OAuth、Tavily、Linkup、WebSearchAPI、Jina、Poe、Groq、Cohere、Gemini API、Gemini Web |
| 用户批准的余额受限状态，不代表生成通过 | OpenAI、Anthropic、DeepSeek（402） |
| 文档/本地合同或没有有效账号，不代表真实生成 | Azure、Mistral、xAI、Perplexity API |
| 有媒体/HTTP 范围限制 | Gemini Canvas（不含音乐/视频）、Google AI Studio Web、ChatGPT Web（特定账号/模型）、FlowMusic Web（音乐及 URL，不含视频）、Stitch Web（Chat→HTML，不含全工具/附件/逐 token） |

`flow-web` 暂停，不算完成。先前增加的 17 个模板仍需按
`docs/progress/2026-11-06-provider-channel-expansion-status.md` 的当前 checkpoint 继续验证；不因本次盘点自动升级为完成。

## 截图 42 项逐项对照

“未接”指本轮盘点没有找到对应产品的完整专用入口；不是说通用 custom API 永远不能调用它。
“部分已有”指已有可复用实现，但截图所列认证/产品路径仍有缺口。

| 截图名称 | 社区插件包 | 本轮前状态与差异 |
| --- | --- | --- |
| Agnes AI | `agnes` | 未接 |
| AtomCode | `atomcode` | 未接 |
| GitHub Copilot | `copilot` | 未接；需要 GitHub token→Copilot token 生命周期，不能只换 URL |
| JoyCode | `joycode` | 未接 |
| Kimi Work | `kimi-work` | 未接；Moonshot API hostname 不等于 Kimi Work |
| OpenCode Zen Free | `zen-free` | 未接；有客户端/工具/提示词及免费层限制，不作为普通公共 Key API 冒充完成 |
| ZCode | `zcode` | 未接 |
| Qoder | `qoder` | 未接；COSY 协议 |
| Devin | `devin` | 未接；Connect/protobuf |
| Zed | `zed` | 未接；envelope/NDJSON |
| Factory | `factory` | 未接 |
| Grok Build | `grok` | 部分已有 `grok-oauth`；本轮补原生 device 管理入口，手工授权/刷新/Responses 保留；真实 device 账号及完整产品验收未完成 |
| Command Code | `commandcode` | 未接 |
| MiMo | `mimo` | 未接；FreeBuff 模型名不是服务商接入 |
| Cursor | `cursor` | 未接；双向 Connect |
| Kiro | `kiro` | 部分已有 Key/OAuth、sender/probe 和协议转换；完整产品/Server/跨应用验收未完成 |
| MiniMax Code | `minimax` | 未接；FreeBuff 模型名不是此产品接入 |
| Warp | `warp` | 未接；protobuf |
| WorkBuddy | `workbuddy` | 只有软件指纹/上下文，不是账号服务商接入 |
| Cline | `cline` | 未接 |
| Trae | `trae` | 未接 |
| Open Design | `opendesign` | 未接 |
| CatPaw | `catpaw` | 未接 |
| Loomy | `loomy` | 未接 |
| Nous Portal OAuth | `nous` | 未接；refresh token 有单次使用语义 |
| 商汤小浣熊 | `raccoon` | 未接 |
| SenseNova Token Plan | `sensenova` | 未接 |
| InkStone | `inkstone` | 未接 |
| AutoClaw | `autoclaw` | 未接 |
| CodeArts / OfficeAce | `codearts` | 未接；DPoP→STS→签名链 |
| GCP AI | `gcp-ai` | 部分已有 Vertex OAuth/Express Key；ADC 导入/发现没有接入 |
| Cloudflare Workers AI | `cloudflare-workers-ai` | 本轮新接，见下文；未作真实账号验收 |
| River | `river` | 本轮新接，见下文；未作真实账号验收 |
| SAP AI Core | `sap-aicore` | 未接 |
| OrcaTerm | `orcaterm` | 未接 |
| AStudio 讯飞 | `astudio` | 未接 Astron Studio；现有讯飞 MaaS/WebSocket 与 Google AI Studio 都不是这个产品 |
| Providers Hub | 外部 `magpie-providers-hub` | 未接；11 个子 provider 包含重复产品，不应全算新厂商 |
| Agnes Code | `agnes-code` | 未接；与 Agnes AI 分开 |
| Tabbit | `tabbit` | 未接 |
| Codar | 外部 `magpie-plugin-codar` | 未接 |
| MClaw | `mclaw` | 未接 |
| Antigravity CLI | `agy-cli` | 未接 local agy ACP；现有 direct HTTP OAuth 不是此实现线 |

外部包另见 [Providers Hub](https://github.com/Gyl1219/magpie-providers-hub) 和
[Codar](https://github.com/MisonL/magpie-plugin-codar)。这里只核对了来源，不把未固定版本的外部包当已实现依据。

追加核对：同一固定 `packages/qoder` 已含 `QwenWorkAuthPlugin` 与 enterprise 产品参考。
红框里的两套 QwenWork 插件不是两家厂商，也不等于现有普通 Qwen workspace Key API。

## River 原生实现

- 预设：`river-api`；profile：`river`；feature：`line-river-official-api`。
- 固定 API base：`https://api.river.ai/v1`；普通 Bearer API Key，使用现有账号增删改与 SecretPatch 持久化。
- `GET /models` 仅导入账号目录，不发送生成探测；保留手工模型，目录失败不清空旧值，不写入 verified 状态。
- Chat、图片输入、tools 和 Chat SSE 复用 OpenAI family；Responses 的非流式和 SSE 复用现有 Responses 编解码/传输。
- 不硬编码默认模型、价格或余额；自动账号测试默认关闭。启用测试前应主动选定一个当前账号可用模型，不批量测试收费目录。
- 按[官方 API controls](https://docs.river.ai/guides/inference-controls)，状态型
  `store:true`、`background:true`、`previous_response_id`、`conversation` 不在支持范围；Gateway 不伪造其状态或成功结果，上游错误正常传播。
- 运行时不启动 magpie，不读取它的账号目录，也不隐式导入现有凭据。

协议来源：[River quickstart](https://docs.river.ai/guides/inference)，
参考包：[river](https://github.com/magpie-community/plugins/tree/93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/river)。

## Cloudflare Workers AI 原生实现

- 预设：`cloudflare-workers-ai`；profile：`cloudflare_workers_ai`；feature：`line-cloudflare-workers-ai`。
- 创建服务商不填 Account ID 或 Base URL，可先保存空池。同一池可添加多个账号；在账号表单填写 32 位小写十六进制 Cloudflare Account ID 和 API Token，网关由 ID 生成该账号的官方 URL。
- 模型目录、推理和额度查询使用同一条选中账号记录。更换 Account ID 必须录入该账号的 token，不沿用旧值；已有配置中没有账号 URL 的记录仍可使用原池地址。
- 账号使用 Workers AI API Token，不支持旧式 Global API Key + Email。推理通常需要 `Workers AI Write` 权限，目录支持 Read/Write。
- 推理令牌还需目标账号的 `Account Analytics: Read`。Gateway 在发送推理前通过 Cloudflare GraphQL Analytics 核对该 Account ID 的 UTC 当日 `sum.totalNeurons`，覆盖 Cloudflare 已纳入统计的其他应用用量；不使用模型目录或本地 Token 估算冒充账户额度。查询参考 `hefy2027/cf-manager` 的 `getAiUsageToday`，不引入该项目运行时。
- 有请求时最多复用 30 秒的成功查询；空闲时不轮询。达到 9,000 神经元后，同一 Account ID 下所有模型和令牌暂停到下一次 UTC 00:00（北京时间 08:00），随后由首个请求重新查询。权限不足、HTTP/GraphQL 错误或无该账号数据时，不发送推理；重启会在发送前重新查询，不复用旧快照。目录查询不消耗这条推理门禁。
- 这是保护阈值，不是 Cloudflare 的硬费用上限。分析数据可能滞后，其他应用也可继续消耗该账号额度；Workers Paid 仍可能产生超额费用。未完成真实账号计量验收前，不用本功能承诺零超额。
- Chat/SSE 和 Embeddings 复用 OpenAI family。Responses 只允许
  `@cf/openai/gpt-oss-120b`、`@cf/openai/gpt-oss-20b` 且 `stream:false`；其他组合在网络请求前拒绝。
- 目录来自账号下官方 `/ai/models/search?task=Text%20Generation&per_page=100&page=1`。
  只导入文本生成目录；Embeddings 模型按官方名称手工添加，不把所有任务模型误当 Chat 可调用。
- 目录复用 30 秒全局期限、累计 2 MiB 和 2048 个唯一模型上限，最多 21 页；不跟随跳转。
- 最终发送再次核对目标、路径、协议限制和选定 token，丢弃冲突的 Host/Cookie/Global Key 头，不向跳转目标重放 token。
- 不依赖 `models.dev`；不硬编码 Paid 列表、不推算 Neurons 余额、不把共享免费配额当免费模型；不自动测试收费目录。

协议来源：[OpenAI compatibility](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/)、
[Model Search](https://developers.cloudflare.com/api/resources/ai/subresources/models/methods/list/)。
参考包：[cloudflare-workers-ai](https://github.com/magpie-community/plugins/tree/93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/cloudflare-workers-ai)。

## Grok Build 原生设备授权入口

- 在既有 `grok-oauth` 实现线上增加 `authorizationFlow:"device"`，未传时仍走原 code/manual callback。
- Client ID 和 scopes 由用户明确填写，不内置或借用官方 CLI 身份，也不读取本机登录材料。
- 控制台显示官方验证链接、用户码、有效期和轮询倒计时；每次明确点击只尝试一次 token poll。
  GET 只读会话状态，不后台交换 token 或自动保存。合法长间隔和 `slow_down` 在 HTTP 入口提前返回，
  不把等待塞进管理请求、突破前端 30 秒预算，也不缩短上游要求的节拍。
- 复用同一个 16 会话容量、过期/取消、一次性 receipt、显式保存、SecretPatch 和 refresh generation owner。
  用户仍须明确点击“保存到凭证池”；取消或丢弃在途交换不会伪造成功，保存结果不确定时不盲目重试。
- device flow 没有 code-flow nonce；`identityVerified=false`，不把 token endpoint 交付冒充为已验签用户身份。
  Consent 预选与独立的 required principal 约束保持区分。
- 真实 client 注册权限、设备登录、续期和订阅推理未在本轮验证；不能因此将 Grok Build 整卡标记完成。

## 验证与未完成边界

本批测试使用合成 token、loopback 上游及隔离本地数据，不使用真实 `.ng` 或付费账号。
测试入口：`RiverCatalogTemplate.test.tsx`、`CloudflareWorkersAi.test.tsx`，Rust `river` / `cloudflare` 定向过滤器，
以及集成目标 `console_magpie_api_catalog_contract`。实际结果和当前 Rust 编译阻碍见
[首批 checkpoint](../progress/2026-11-08-magpie-first-batch.md)，原始日志保留在本轮调查目录 `gateway-magpie-expansion-20261108`。

新增两条实现线不登记 `TEMPLATE_ADAPTATION_RECEIPTS`，不点亮“新调用流程已验收”。
未做真实账号目录/推理、付费请求、完整桌面前台验收、Server/跨应用验收、devtest 包或正式发布。
不代表其余截图条目已完成，也不代表旧有 17 项扩展的验收债务已经关闭。

后续按用户选择，优先 OAuth／订阅账号接入，具体顺序见第三方来源文档；普通 Key API 排在其后。
按产品而非品牌推进：Copilot 等凭据交换独立做生命周期，Connect/protobuf 与 ACP 独立做协议/进程边界。
Zen Free 的限制必须先确定支持合同，不能靠仿冒客户端/工具来绕过限制。
