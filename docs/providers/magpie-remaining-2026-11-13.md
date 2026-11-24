# 服务商剩余功能块清单（2026-11-13）

## 结论与计数口径

以下 67 项是 2026-11-13 按 Magpie 原始来源卡去重形成的 A–C 全范围基线，不是实时待办数。首批三家已按调整后的范围[完成本轮开发](first-three-closure-2026-11-13.md)，不等于满足原卡的全范围，也不重算其他卡。D 的渠道扩展含重叠项，不能直接与 67 相加：

| 分类 | 数量 | 含义 |
| --- | ---: | --- |
| 已有不同范围本地实现，仍需补齐或验收 | 13 | 不是从零开发，也不是仅剩真实账号测试；各项仍有具体功能缺口 |
| 主清单中待原生实现或资格／合同核实 | 43 | 包括已有相邻产品、参考代码或合同，但原目标尚未接通的项目 |
| 聚合来源额外待实现／差异核对项 | 11 | 不把聚合外壳或底层模型品牌重复计算 |
| **尚未完整关闭合计** | **67** | **13 + 43 + 11；本任务口径完整关闭 0 项** |

这不是 67 家不同公司，也不是 67 个等量开发任务，更不是 Gateway 全部既有服务商的完成情况。
原始 42 项＋23 张红框卡，去掉 2 个聚合外壳并合并 7 张重复卡，得到 56 个主项；
再加 11 个聚合来源额外项，共 67。国内／国际站与原卡已有产品分支保留在同一行内跟踪。
ModelScope msmoli 是辅助能力而非推理服务商，但仍保留在用户原范围中。

相对 11 月 11 日盘点，Copilot 从待实现转为已有受限本地实现，因此分类从
12／44／11 更新为 **13／43／11**；没有任何整项因此直接关闭。

本清单依据当前仓库工作板与各批回执整理，不是本日重新运行全部产品或重新联网核实资格。
资格限制引用已有日期记录；历史构建阻碍没有新解除回执的，仍标待复核，不断言当前必然失败。

## A. 已有本地实现，但仍需补齐或验收（13）

下表编号沿用原 67 项清单。仅列已记录的缺口，不将所有产品强制扩成同一套功能。
真实账号、费用、原生包、Server 等分别验收；不要求为不支持的范围虚构成功。

| 原编号 | 功能块 | 已有基础 | 剩余功能／验收 |
| --- | --- | --- | --- |
| 1 | Agnes AI | 国际站显式 Key、管理／目录／UI、公共文本及工具切片 | Models API 真实合同与账号权限；真实调用／费用；国内站、网页登录、媒体；原生包、Server／Redis；独立 feature 构建阻碍待复核 |
| 3 | GitHub Copilot | 专用身份保存、SDK owner、受限公共 Chat／Responses 单轮文本、额度审计、在途禁用验证 | 真实 CLI／订阅账号及模型调用；公共 Key 撤销、token 轮换、断连取消；未确认 stop 后隔离策略的产品收尾；原生手测包与 Server 支持边界。流式、多轮、工具、媒体、现金当前不支持，不能算已完成，扩展前明确目标范围 |
| 12 | Grok Build | 产品 owner、device 接线、runtime-defaults 编译及定向回归 | 真实账号与公共调用验收；完整产品交付证据 |
| 13 | Command Code | [首批公开 Provider API 已收口](first-three-closure-2026-11-13.md#1-command-code) | 真实账号／费用未验；OAuth 签发、远端 Responses 生命周期及 System One 现金延期。 |
| 16 | Kiro | 既有 Key／OAuth 部分接入 | 完整产品与跨应用验收；不能按部分接入宣布整项关闭 |
| 25 | Nous Portal OAuth | 管理／目录／UI、持久刷新、双目标文本工具、Embeddings 等本地切片 | 获准 client／真实账号；浏览器视觉、原生包、Server／Redis、媒体及完整产品验收；不借用 hermes-cli 身份 |
| 27 | SenseNova Token Plan | [首批订阅 Key 与三协议已收口](first-three-closure-2026-11-13.md#2-sensenova-token-plan) | 真实账号／费用未验；视频、控制台 OAuth／积分池延期。 |
| 31 | GCP AI | 既有 Vertex OAuth／Express | ADC 导入／发现；对应产品完整验收，不以现有 Vertex 等同全部 GCP 路线 |
| 32 | Cloudflare Workers AI | 首批实现与定向合同 | 真实账号／推理与完整交付验收；不与 Cloudflare AI Gateway 合并 |
| 33 | River | 首批实现与定向合同 | 真实账号／推理与完整交付验收 |
| 34 | SAP AI Core | [首批原生协议与受控持久化 Responses 已收口](first-three-closure-2026-11-13.md#3-sap-ai-core) | 后台 SSE 仅有 Local 源码回执；未知提交对账／人工处置延期；真实租户／费用未验。 |
| 44 | DIM | 两站隔离、授权刷新、管理／UI／目录、多 wire 文本工具 | 操作者提供获准 client 注册；真实账号、媒体、桌面／Server／跨应用验收 |
| 53 | LobsterAI | 授权刷新、管理／UI／目录、普通与虚拟模型公共推理、专属测试／调度／运行时重建 | 真实账号／订阅／费用与自有身份头；跨请求持久绑定、特殊 profile／Kimi、reasoning／媒体；Server／Redis、独立 feature、原生包与原生进程重启验收 |

## B. 主清单待实现或资格／合同核实（43）

“待实现”指原目标尚无完整接通回执，不表示仓库绝对不存在任何相关类型或代码。

| 原编号 | 功能块 | 主要剩余内容／阻碍 |
| --- | --- | --- |
| 2 | AtomCode | 独立客户端／签名资格；原生 OAuth 与 CodingPlan。已有记录表明 signer 限官方构建，不复制 master key |
| 4 | JoyCode | 原生接入与产品验收 |
| 5 | Kimi Work | 专用产品接入；Moonshot Key API 不抵扣 |
| 6 | OpenCode Zen Free | 免费路线接入资格与实现；11 月 12 日记录的官方客户端限制待解决，不用付费／Go 替代 |
| 7 | ZCode | 原生接入与产品验收 |
| 8 | Qoder | 原生接入；CN、QwenWork／enterprise、COSY 分支分别核对 |
| 9 | Devin | Connect／protobuf 原生接入 |
| 10 | Zed | envelope／NDJSON 原生接入 |
| 11 | Factory | 原生接入与产品验收 |
| 14 | MiMo | 原生接入；其他聚合渠道同名模型不抵扣 |
| 15 | Cursor | 双向 Connect 原生接入 |
| 17 | MiniMax Code | Agent OAuth 客户端资格、授权及推理接线；已有 M Plan 订阅 Key 是相邻产品，不抵扣本项 |
| 18 | Warp | protobuf 原生接入 |
| 19 | WorkBuddy | 国内／国际专用账号接入 |
| 20 | Cline | 原生接入与产品验收 |
| 21 | Trae | 原生接入与产品验收 |
| 22 | Open Design | 原生接入与产品验收 |
| 23 | CatPaw | 原生接入与产品验收 |
| 24 | Loomy | 短信／session 原生接入；不伪造 refresh |
| 26 | 商汤小浣熊 | 专用原生接入，不与 SenseNova Token Plan 合并 |
| 28 | InkStone | 原生接入与产品验收 |
| 29 | AutoClaw | 国内／国际隔离的原生接入 |
| 30 | CodeArts / OfficeAce | PKCE／DPoP／STS／签名；各产品身份分开 |
| 35 | OrcaTerm | 原生接入与产品验收 |
| 36 | AStudio 讯飞 | AstronStudio 专用接入；不是 Google AI Studio 或讯飞 MaaS |
| 37 | Agnes Code | 独立原生接入，不以 Agnes AI 抵扣 |
| 38 | Tabbit | 原生接入与产品验收 |
| 39 | Codar | 原生接入与产品验收 |
| 40 | MClaw | 原生接入与产品验收 |
| 41 | Antigravity CLI | 本地 agy ACP owner；既有 direct HTTP 不抵扣 CLI 路线 |
| 42 | Muse Web | 独立浏览器 owner，不按普通 OAuth／公开 API 接入 |
| 43 | Magicrew | 国内／国际原生接入；停维护、WAF／ESA405 历史阻碍需复核 |
| 45 | DeepSeek Web | Web token／session／PoW／SSE；官方 API 不抵扣 |
| 46 | OpenCSG | 网页登录换 Key；不假定长期 OAuth refresh |
| 47 | Notion | Cookie／私有 AI API；文本与工具能力边界 |
| 48 | TeleAgent | 手动 session／签名；不自动解密客户端，不伪造第三方 refresh |
| 49 | ModelScope msmoli | 辅助功能实现；签到／额度等副作用须明确授权，非推理厂商 |
| 50 | Marvis | 浏览器账号／refresh／业务签名 |
| 51 | EdgeOne | 匿名 agent session／RPC／events.mux，不是普通腾讯 Key API |
| 52 | AMD Radeon Cloud | 11 月 12 日资格记录限制生产、第三方转供与账号池；共享接入受阻，个人开发范围另定；实现尚未完成 |
| 54 | DuMate | Cookie 换临时 gateway token 与 Chat；Cookie 不伪造续期 |
| 55 | Cloudflare AI Gateway | BYOK／provider 路径原生实现；不复制未许可参考源码 |
| 56 | HF SSH Chat | SSH／PTY／host-key／队列 owner；不是 HF HTTP Router，不默认建 tunnel |

## C. 聚合来源额外工作项（11）

| 原编号 | 功能块 | 主要剩余内容 |
| --- | --- | --- |
| 57 | Accio Work | 核对 Hub 产品与既有 Accio 的身份、协议及功能差异，再确定需要补齐的实现 |
| 58 | MonkeyCode | 国内／国际两站原生实现，按一个产品项跟踪 |
| 59 | 七牛云 MaaS / Modelink | 产品分支合同与原生实现 |
| 60 | 百智云 | Hub 源码合同核对与实现；固定 npm 发布未包含该源码 |
| 61 | 灵犀 | 固定快照缺实现文件，先取得可用协议合同 |
| 62 | 豆包工作 | 固定快照缺实现文件，先取得可用协议合同 |
| 63 | Dola | 固定快照缺实现文件，先取得可用协议合同 |
| 64 | CodeBuddy IDE | 原生接入与产品验收 |
| 65 | Kilo 免费池 | 匿名免费入口已接入，见 [`kilo-free`](kilo-free.md)；不含付费池 |
| 66 | EAC 授权聚合网关 | 原生 owner，不下载执行原 runtime 代替实现，不重复统计底层模型厂商 |
| 67 | Gemini Code Assist / Antigravity | 核对既有 Cloud Code 路线及缺口；不等同 Gemini Web，也不以 direct HTTP 关闭第 41 项 CLI 路线 |

## D. 本对话渠道扩展的剩余项（R01–R16）

以下补齐 [渠道扩展计划](../plan/2026-11-02-provider-template-expansion.md) 的 R01–R16 及实际产品分支。这里按渠道登记，不新增一个可与 67 相加的品牌总数：Grok Build 对应 A/12、Kiro 对应 A/16、Vertex 属于 A/31；Accio OAuth 与 C/57 Accio Work 是否同一产品仍须核对。其余行补充本对话范围，不表示已审计 Gateway 所有既有服务商。

“已有基础”仅表示回执覆盖的有限链路，不代表完整产品通过。下列资格、平台限制和真实账号结果均沿用已注明日期的记录，本次只做文档盘点，没有重新联网确认、授权或调用供应商。

### D1. 文本、编码与云 API 渠道

| 需求／渠道 | 已有基础 | 仍未完成或受阻 | 依据与原清单关系 |
| --- | --- | --- | --- |
| R01 Accio OAuth | 授权与管理、DB 兼容读写／刷新、后台 worker、实际 Server 管理 TCP 与三公共协议限定链 | unified key／bundle／prepaid／额度及运行模式联合；全部取消边界；旧 account writer／deletion 锁前副作用；005 迁移经 Platform owner 审核及现有服务器迁移；完整后台任务退出、独立进程强杀／恢复、原生包；真实 client／wire／模型／额度 | [Server 回执](../progress/2026-11-09-provider-channel-expansion-accio-db-server.md)、[授权恢复](../progress/2026-11-12-provider-channel-expansion-accio-kiro-recovery.md)；与 C/57 的身份及产品差异未决 |
| R02 Kiro OAuth／CLI Key | 显式 profile runtime、正常 OAuth UI、公共 TCP／Local 重建与事件完整性；失效授权 UI 恢复已修 | 最小 scope／profile 发现／订阅与客户端资格；旧 route／DB 自动兼容；完整参数、decoder、取消阶段；Server、跨应用回执、独立原生包及真实账号验收。两种身份分别验收 | [总计划](../plan/2026-11-02-provider-template-expansion.md)、[恢复回执](../progress/2026-11-12-provider-channel-expansion-accio-kiro-recovery.md)；细化 A/16，不重复计数 |
| R03 Grok Web | 管理 UI→本地 TCP、会话保存／独立进程重启、公共 Chat／Responses 流与非流、普通余额 Key 本地合成结算 | 多轮／system、搜索引用、附件／工具；真实目录、订阅／额度、SSO／challenge 恢复与录制；Server／PG、适用现金与成品包完整联合验收 | [进程／UI](../progress/2026-11-12-provider-channel-expansion-grok-process-ui.md)、[余额 TCP](../progress/2026-11-12-provider-channel-expansion-grok-prepaid-tcp.md)；不以 Grok Build 或 xAI API 抵扣 |
| R04 xAI API | 复用既有 `xai-api` 官方 API 实现与模板，不再建立同义 grok-api | 本计划内适用集成及真实资格、模型、费用验收尚未整体关闭；没有据此认定新的具体实现缺陷 | [总计划](../plan/2026-11-02-provider-template-expansion.md)、[渠道状态](../progress/2026-11-06-provider-channel-expansion-status.md) |
| R05 Grok Build OAuth | device 管理入口、自动 loopback listener、产品 runtime 与定向回归 | 跨进程 reconciliation UI；第三方客户端授权／使用资格；完整 Responses；跨 revision 刷新恢复、永久 Sending、其他 refresh 入口及 durability；最新 listener 数值 IPv6／16 并发 socket 边界；真实业务及新包验收 | [登录合同](../provider-grok-oauth-login.md)、[loopback 回执](../progress/2026-11-12-provider-channel-expansion-grok-loopback.md)；细化 A/12 |
| R06 FreeBuff Web | 正常网页→管理 TCP／Grant→Local SQLite→三公共协议单轮文本活 SSE；有限强杀恢复测试 | 完整参数／模态、实际工具 child、持久完整 history、未知结果对账／安全恢复、远端 lease、probe、新设备目录 UI；Server、全新恢复进程／原生包及真实平台。未获 START／FINISH 查询、幂等及金额合同前不自动重放 | [公共 SSE／UI](../progress/2026-11-08-provider-channel-expansion-freebuff-public-sse-ui.md)、[进程边界](../progress/2026-11-08-provider-channel-expansion-freebuff-run-process.md) |
| R07 Qwen Web | 管理 UI→本地 TCP、会话保存／独立进程重启、公共 Chat／Responses、普通余额 Key 本地合成结算 | signin／MFA／challenge／seed 与 Cookie 刷新 owner；多轮／parent／system、工具／搜索／附件；真实账号目录／额度；Server／PG、适用现金与成品包联合验收 | [进程／UI](../progress/2026-11-12-provider-channel-expansion-qwen-process-ui.md)、[余额 TCP](../progress/2026-11-12-provider-channel-expansion-qwen-prepaid-tcp.md) |
| R08 旧 Qwen OAuth | 保留独立需求与历史资格调查，不用 Web／普通 API 别名抵扣 | 既有 11 月 2 日记录指出原免费 OAuth 停用；须核实获准的新渠道或由用户明确调整范围。资格未解决，不算完成 | [总计划](../plan/2026-11-02-provider-template-expansion.md)；本次未重新联网核实停用状态 |
| R09 Qwen 普通 API | 六区／workspace、native 目录、Key／Keep／probe、公共 buffered／SSE 本地切片 | 其他声明能力完整保真；PG probe principal／source reserve 的动态取消；适用联合链、真实账号／模型／费用及交付验收 | [总计划](../plan/2026-11-02-provider-template-expansion.md)、[身份合同](../plan/provider-channel-identity.md) |
| R09 Qwen Coding Plan | 已有相邻订阅 Key 实现，独立于普通 API | 历史官方合同限制交互编码工具用途；通用后端／转供适用资格与范围待决，不能用普通 API 完成结果抵扣 | [总计划](../plan/2026-11-02-provider-template-expansion.md)；资格限制为既有记录，非本次重核 |
| R10 Bedrock Bearer／IAM | 普通 Local 次数 Key、三公共协议及原生入口的 TCP 合成链；文本／工具／reasoning／signature 历史和限定取消 | 完整参数／模态；其他余额、权限、存储／运行模式；Server／PG／Hosted、跨进程／原生包；真实 AWS Bearer／IAM、模型开通／额度／期限。buffered 目前聚合 converse-stream，不代表新增 converse JSON transport 已验 | [公共 TCP 回执](../progress/2026-11-07-provider-channel-expansion-bedrock-public-tcp.md) |
| R11 Vertex OAuth／Express Key | Full Vertex OAuth 授权／持久刷新／UI、三公共协议与工具签名历史；Express 独立身份路线 | 当前完整聚合后桥接 SSE，非实时 token／参数流；signed Preview、交错表示、全部 Part／模态、多 choice／完整 decoder；Redis／PG／Hosted、跨进程／原生包、feature 矩阵与真实账号／模型／额度。两路线分别验收；GCP 原项另缺 ADC 导入／发现 | [OAuth 工具回执](../progress/2026-11-05-provider-channel-expansion-vertex-oauth-tools.md)；细化 A/31，不等同全部 GCP 路线 |
| R12 Perplexity Web | 单轮文本、Cookie／UA、公共 Chat／Responses、有限 markdown patch／answer-only；完整 workflow 最终文本解析；获准官网两轮观察仅第二轮有完整缓冲证据 | 正常 Gateway 多轮续接；更广 patch／workflow profile、Research／附件；目录／会话刷新／challenge、模型／额度／费用；完整公共／Server／进程／包与真实联合。官网两轮观察不等于 Gateway 多轮已接通 | [最新 workflow 回执](../progress/2026-11-13-provider-channel-expansion-perplexity-workflow.md)、[workflow 合同](../plan/provider-perplexity-workflow-contract.md) |

### D2. 图片、视频与音乐渠道

| 需求／渠道 | 已有基础 | 仍未完成或受阻 | 依据与边界 |
| --- | --- | --- | --- |
| R13 ChatAIBot Web | 公共图片生成／编辑／合成与 multipart、PNG／JPEG 校验、次数结算、精确媒体 probe 与逐次费用确认 UI | 当前会话／challenge／refresh、完整模型／比例／参数、认证与额度关系、正常 Keep 全链；独立 TCP／进程／包及真实生成验收 | [媒体 probe 回执](../progress/2026-11-07-provider-channel-expansion-chataibot-media-probe.md) |
| R14 Luma Web | 单图任务／worker 资源／公共图片；精确图片 probe，正常浏览器→管理 TCP→合成 executor 联合 | 当前官网 board wire／session、任务／资产发行方合同、可靠模型／action 绑定、有效音视频、费用／额度、完整 Keep；远端取消／lease／跨请求 exactly-once；真实 browser worker、Server／PG／跨进程／原生包 | [图片 probe／UI](../progress/2026-11-12-provider-channel-expansion-luma-image-probe-ui.md)；模型绑定未验证，不恢复健康，供应商扣额未知 |
| R14 Luma Agents API | 独立 Key、模板／保存／Keep／清空／本地重建、图片／图片编辑／视频；扩画、重绘、layering 的公共 API 与显式账号测试；扩画位置、重绘 conditioning／guides／有限 HDR 的高级 UI；同 UUID 轮询、资产验证及本地结算；成功与正常失败终态的晚到 credits 有界补查、任务事实及原审计接线；取消／超时前已验证任务及费用快照；已保存账号的 ≤8 MiB 内联／≤64 MiB 签名 Files 上传、TTL/user_id／分页／状态／确认删除与 ID 复制 | 超过 64 MiB 的 Files 上传及真实验收、其他官方操作、重绘 EXR／duration、取消后未知费用、补查截止后及跨进程追账／cash、真实分层／扩画／重绘／晚到扣费及视频播放、实际平台权益；Web／REST 额度对照及完整产品交付 | [图片分层](../progress/2026-11-18-provider-channel-expansion-luma-layering.md)、[高级参数 UI](../progress/2026-11-19-provider-channel-expansion-luma-video-controls.md)、[失败费用审计](../progress/2026-11-17-provider-channel-expansion-luma-failure-audit.md)；不以 Web 或 MCP 授权替代 API Key 权益 |
| R14 Luma LLM Inference | 独立产品／Key／固定目标、空池模板、正常账号保存／Keep／清空与暂存受众；Chat／无状态 Responses／Messages、function tools、JSON／SSE、原审计结算与显式账号测试 | 真实销售开通账号、模型资格、限速／供应商账单、Server／PG／Redis、跨进程／原生包；不猜 models 或余额接口 | [独立合同](lumalabs-inference-contract.md)、[本地实施回执](../progress/2026-11-19-provider-channel-expansion-luma-inference.md)；不使用 Agents Key／credits，不新增整卡完成 |
| R14 Luma OAuth／MCP | 正常管理授权、显式刷新、工具目录与创作执行链已有交付回执 | MCP 分层暂缓；各运行形态、真实账号及费用验收边界见[四渠道现状](luma-development-status.md) | [渠道合同](../provider-luma-channel-contract.md)、[真实目录回执](../progress/2026-11-20-luma-mcp-live-contract.md) |
| R15 Suno Web | MP3 字节解码、公共 Local 次数结算、native 模型绑定；直连／BrowserBacked 精确 probe、费用确认、正常网页→管理 TCP→合成 remote 联合 | 当前模型／feed wire、device／Clerk／受众／challenge／refresh、实际额度与下载权益、完整生成参数／其他格式；远端硬取消／lease；完整公共／Server／进程／原生包与真实平台验收 | [浏览器 probe 回执](../progress/2026-11-08-provider-channel-expansion-suno-browser-probe.md)；有效 MP3 和 probe UI 不再作为完全未实现项 |
| R15 Suno OAuth／MCP | 11 月 12 日独立 client 注册成功，用户尝试官方授权 | 用户当时收到 `Connecting AI apps to Suno isn't available for your account yet.`；未交换 token／读取 tools/list。待资格改变或用户明确选择其他账号后继续工具合同、授权刷新、产品入口与业务；不绕过资格或重复注册 | [注册回执](../progress/2026-11-12-provider-channel-expansion-suno-mcp-registration.md)；资格受阻，不推断账号永远不可用 |
| R16 Udio Web | worker／模型绑定、公共音乐元数据与本地结算、精确 probe／费用 UI；图片 URL／base64 安全准入及 PNG／JPEG 完整解码 | 当前 wire／session／模型／额度与图片权益、平台内有效音乐播放、完整参数／Keep；本地 worker／远端 lease／取消、Server／独立进程／原生包与 feature 边界。既有无公共 API／下载限制合同不得通过绕过限制补验收 | [音乐任务 UI](../progress/2026-11-11-provider-channel-expansion-udio-probe-ui.md)、[图片资产](../progress/2026-11-11-provider-channel-expansion-udio-image-assets.md)；音乐元数据成功不代表音频验证或模型健康恢复 |

### D3. 共通交付与验收边界

- **P50 尚未整体关闭**：上述限定本地链不能代替全范围集成；仅对各产品承诺的能力、运行模式和结算类型补验，不强加所有渠道都必须支持相同功能。
- **P60 尚未整体关闭**：集中验收材料及覆盖最新源码的成品包仍需收尾。11 月 12 日等既有包不能冒充包含 11 月 13 日后续源码；Luma 于 11 月 14 日已补 Web devtest 包及本地授权刷新真实验收，不代表其他渠道或独立 desktop launcher 已交付。
- **P70 尚未整体关闭**：真实资格、账号、模型、业务结果、额度／费用与适用联合验收分项记录。合成 executor、模拟界面、工具目录读取、编译成功都不能替代真实业务成功；需要消耗额度或生成作品的操作另遵守授权。
- 旧 Dream Machine API 仅为对照／范围待定，不新增为已批准必做渠道；Together 不在本次范围，Flow Web 的既有暂停不因本次登记而解除。

### D4. Luma 收尾标准（2026-11-14 状态复核）

以下是历史快照，不是当前待办；现状以[四渠道状态页](luma-development-status.md)为准。

2026-11-19 接手复核：[四渠道当前剩余工作](luma-development-status.md)区分实现缺口、待验收及未承诺范围；[开发工具](../../tools/luma/README.md)已迁入仓库，后续不依赖旧临时脚本。

**结论：Luma 整体未完成；OAuth/MCP 正常账号注册、授权保存、一次真实显式刷新、26 项工具目录，以及 Web devtest 包正常原生重启恢复已通过。创作业务和其他适用模式仍未关闭。**
最新依据为 [完整显式刷新回执](../progress/2026-11-14-provider-channel-expansion-luma-explicit-refresh.md)，不再把正常账号执行器、Result／Published 或桌面离线入口列为未实现。
本节按用户流程合并收尾项，不新增供应商计数，也不撤销 D2 中已批准的其他渠道范围。

| 收尾单元 | 当前缺口 | 关闭条件 |
| --- | --- | --- |
| MCP 账号授权与刷新 | 本地正常管理 API 真实联合已通过：独立兼容注册、用户官方授权、保存、一次刷新及前后 26 项目录读取；完整网页点击／窄屏未验 | 本地管理链已关闭；不借调查客户端、不静默升级已有注册权限，不将 API 验收冒称完整界面手测 |
| 刷新中断与恢复 | 本地真实持久层的 receipt／Result 恢复、提交后 Published 修复和保存失败恢复已通过；完整原生进程崩溃／远程模式仍未验 | 继续按适用运行模式验收，不把进程内重建当作完整原生崩溃测试；保持未知 Sending 不重发、新授权不被覆盖、历史结果不过期失读 |
| MCP 创作业务 | 模型列表／详情本地完整入口及 Web 手测包已接，真实模型读取未验；生成、查询结果、取消和费用业务未闭合 | 按批准能力完成真实入口到结果；付费／生成调用先取得对应授权；真实业务验收与本地合成验证分别记录 |
| 成品交付与适用模式验收 | Web devtest 包已通过构建／完整性检查及 Windows 本地 SQLite 真实账号联合、正常原生重启后目录读取；远程／跨机器及 Server 仍有缺口 | 本地 Web 手测包已交付；未覆盖模式继续标为未完成，不能以正常重启代替强制崩溃恢复 |

实施顺序：本地完整显式刷新流程、真实账号与 Web 手测包验证已完成；下一步按 MCP 创作业务和 Web／Agents REST 的剩余用户流程分别推进，不重复登记上述已通过项为待实现。自动定时刷新不得因显式刷新完成而默认标为支持。

实现约束：复用现有 Luma Registry 的任务持有与 shutdown/join，以及现有刷新登记存储和正常秘密保存机制。未为刷新启用 MCP 推理账号，也未另建平行持久层。`commit_refresh_ticket` 已通过窄投影接口接纳显式 Luma ticket，恢复同样验证完整有限投影。

复核依据：最新实现与测试见上述完整流程回执；此前底层分批记录仍作为历史保留。MCP 账号支持只读工具目录和模型列表/详情，不因刷新接通而获得创作能力；不减少供应商剩余计数。

## 如何更新完成数

1. 每项按批准范围记录产品入口、凭据／生命周期、目录与准入、实际协议、错误／取消／额度。
2. 分开记录本地合成验证、真实账号与费用、原生包、Server／跨应用等适用验收结果。
3. 明确不支持、资格受阻、未测试和已通过，不能用新增 manifest、模块或测试数量减项。
4. 只有 A–C 中整项批准范围关闭才减少 67；D 按需求／渠道单独更新，并同步重叠项，不另加重复总数。若用户调整范围，应显式记录范围变更，不静默删除阻碍项。

依据：[原始去重盘点](magpie-scope-count-2026-11-11.md)、
[持续实施看板及各批证据链接](magpie-workboard-2026-11-08.md)、
[Copilot 受限公共接线](../progress/2026-11-13-copilot-public-routing.md)、
[Copilot 公共在途禁用](../progress/2026-11-13-copilot-public-revocation.md)。
