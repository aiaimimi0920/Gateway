# Luma 渠道合同：App Web、OAuth/MCP、Agents REST 与 LLM Inference

最新源码增量（2026-11-20）：[MCP 任务恢复](progress/2026-11-20-luma-mcp-recovery.md)保留旧任务 revision，通过原配置归档和已发布的显式刷新记录核对当前账号。Local 的旧任务查询／取消／结束及素材复用可跨无关配置变更和已证明的 token 轮换；换账号、缺记录或未发布刷新结果仍拒绝，未知提交不重放。没有再次刷新真实账号或收费生成；分层布局、run 交互及 Server 身份仍未关闭。

最新源码增量（2026-11-20）：[MCP 全目录模型](progress/2026-11-20-luma-mcp-models.md)取得全部 18 项模型详情，创作扩至 15 个图片／3 个视频模型。共享配置约束每个模型的参数、比例和参考数量；新接 Seedream、Nano Banana、Ideogram、GPT Image 1.5／2.5 与 Ray 3.14／HDR，沿用费用确认、持久任务及素材复用。用户批准的一次 OAuth 刷新保存成功，没有收费生成或重新出包。分层布局、run 交互、Server 身份和配置变更恢复仍未接。

最新源码增量（2026-11-20）：[MCP 素材复用](progress/2026-11-20-luma-mcp-workspace.md)接入同账号／revision 已保存任务的 ready 素材：UNI-1 编辑／参考图、GPT Image 2 reference medias、Ray 3.2 重绘／扩画。新的任务在原工作区打开独立会话，使用自己的预算／幂等键；返回工作区不匹配时不生成。旧任务和请求摘要兼容，连续复用保留根工作区。当前包不含此增量，没有执行真实收费生成。

最新源码增量（2026-11-20）：[MCP 参数与编辑操作](progress/2026-11-20-luma-mcp-options.md)接入 UNI-1 图片编辑／参考图、GPT Image 2 参数、Ray 3.2 重绘／扩画／关键帧，复用原显式费用确认和持久任务。三种模型之外的目录模型、分层布局、run 交互、Server 与真实业务仍单独待完成。下文“第三方参数／编辑未接”按本段限域更新。

最新源码增量（2026-11-20）：[显式 MCP 创作](progress/2026-11-20-luma-mcp-creation.md)接入 Local 模式 UNI-1 图片／Ray 3.2 视频、持久任务、同 run 查询、取消／结束、ready 资产链接和会话费用。原 read-only profile 仍禁止通用推理；真实收费生成继续暂停，当前 EXE 尚不含此次新增代码。下文早期“创作未接”描述按此限域更新。

最新增量（2026-11-20）：用户完成新 MCP 账号保存后，[真实只读验收](progress/2026-11-20-luma-mcp-live-contract.md)取得当前 26 项完整输入／输出 schema，正常管理 API 返回 18 个模型，并成功读取 UNI-1／Ray 3.2／GPT Image 2 参数。合同、模型参数与回执已归档到仓库；不再等待 MCP 账号或生成输出 schema。尚未接通创作、业务取消、资产和费用链，Web 当前协议仍独立待验，没有收费生成。

最新增量（2026-11-20）：[任务查询与审计展示](progress/2026-11-20-provider-channel-expansion-luma-task-lookup.md)接入运维记录中的任务 ID／费用事实，以及已保存 Agents 账号内按 UUID 的单次显式状态／credits 查询。可在取消或原截止后手工读取同一任务，不回填历史审计、不改账本、不重提生成；自动追账、进程恢复与 cash 仍未关闭。后续[正常网页联合验收](progress/2026-11-20-provider-channel-expansion-luma-task-browser.md)已通过真实管理 TCP／合成上游的六次显式查询、供应商错误隔离与宽窄屏，原取消审计和余额不变。

当前接手入口：[剩余工作与验收边界](providers/luma-development-status.md)、[仓库开发工具](../tools/luma/README.md)、[长期证据归档](evidence/luma/README.md)。本页按时间保留历史增量；下方早期临时来源目录本次已不存在，不能作为当前可打开的证据入口。

最新增量（2026-11-20）：[Files 签名上传](progress/2026-11-20-provider-channel-expansion-luma-presigned-files.md)接入 ≤64 MiB 的 presign/PUT/complete 与 TTL/user_id。前述小文件历史限制由本子批限域扩展；64 MiB 以上、真实供应商摄取／复用／删除、MCP 创作及完整费用恢复仍未关闭。

最新增量（2026-11-20）：[取消审计事实保留](progress/2026-11-20-provider-channel-expansion-luma-cancellation.md)将当前进程中已验证的 UUID／模型／操作／状态／credits 接入原公共和管理取消审计，超时及资产失败同样保留；额外 `mediaVerified` 区分供应商完成与本地媒体验证。取消仍停止原 future、不重提、不将未知记为零，普通 Key 退款不变。下方历史“取消中未上交 receipt”缺口由此限域更新；截止后追账、强制崩溃／跨进程恢复、cash 和真实账号仍未关闭。

最新增量（2026-11-20）：[Files 小文件管理](progress/2026-11-20-provider-channel-expansion-luma-files.md)已接正常 Agents 账号内的 ≤8 MiB 上传、分页、状态查询、确认删除与 ready ID 复制；大文件预签名和真实账号链仍未验。此前[图片分层](progress/2026-11-18-provider-channel-expansion-luma-layering.md)补齐字体数字精确保留与后端回归；[视频参数界面](progress/2026-11-19-provider-channel-expansion-luma-video-controls.md)接扩画位置、重绘 HDR／conditioning／guide frames，并绑定逐次费用确认。独立 [LLM Inference](providers/lumalabs-inference-contract.md)使用 inference 域名、独立 Key 与 token 后付费，不共用 Agents／Web／MCP 材料。真实生成／账单、MCP 创作与完整交付仍未关闭。

最新操作增量（2026-11-17）：Agents REST [视频扩画](progress/2026-11-17-provider-channel-expansion-luma-video-reframe.md)与[视频重绘](progress/2026-11-17-provider-channel-expansion-luma-video-edit.md)已接公共视频入口的显式 `video_reframe`／`video_edit` 与正常账号媒体测试表单。重绘仅 Ray 3.2，单源视频，支持自动／九档强度；API 另提供细粒度 conditioning、guide frames 与有限 HDR。两种操作沿原任务／资产／费用链执行，不开放 App/MCP 创作；真实源视频、供应商生成／费用及播放仍待验收，未开放参数见各子批。

最新增量（2026-11-17）：Agents REST [晚到扣费补查](progress/2026-11-17-provider-channel-expansion-luma-agents-settlement.md)在媒体验证后有界读取同一任务的实际 credits，并进入原公共结果和审计；[失败任务费用审计](progress/2026-11-17-provider-channel-expansion-luma-failure-audit.md)进一步保留正常 failed 终态的已验证任务 ID 与实际费用，接入原公共请求及管理 probe 审计。截止或查询失败仍记未知，失败不改成成功、不改变原本地退款；取消中未上交 receipt、截止后/跨进程追账仍未完成。它不读取 App/MCP 用量，不换算 USD，不代表真实账号或新包验收。

2026-11-14：[模型浏览入口回执](progress/2026-11-14-provider-channel-expansion-luma-model-browser.md)。正常已保存账号可显式读取模型列表／详情，固定两项只读 tools/call；已通过本地协议、管理、UI 和 Web devtest 包验证。此前“完全没有 tools/call／模型发现”的历史描述由此限域更新；真实账号模型读取与创作任务仍未验收。

核实日期：2026-11-07（Asia/Shanghai）。本页只冻结有来源的渠道身份与后续开发边界，
不把营销模型名称、旧 wire、匿名 discovery 或离线测试当作真实账号生成成功。

## 1. 四个不能混同的执行 surface

| 渠道 | 身份与受众 | 额度证据 | 当前处理 |
| --- | --- | --- | --- |
| App board Web | `lumalabs-web` / `lumalabs`；现有 `wos-session`、realm、board worker | 与开发者 REST `unknown` | 保留现有入口；当前 board wire/会话仍需合法的新证据 |
| 官方 OAuth/MCP | `lumalabs-mcp` / `lumalabs_mcp_readonly`；`https://agents.lumalabs.ai/mcp`，普通 Luma 登录并选择 team | 官方明确同 team App credits、相同费率、无独立套餐；只在该维度 `shared` | 正常授权/显式刷新、空池/工具目录及模型读取入口已接；创作未接，不自动合池 |
| Agents REST Key | `lumalabs-api`；`luma-api-*`，`https://agents.lumalabs.ai/v1` | 同 client 多 Key 共享；与 Web `unknown` | 独立 sender/模板保留；不拿此渠道成功证明另两者 |
| LLM Inference Key | `lumalabs-inference-api`；`https://inference.lumalabs.ai/v1`，独立销售开通 Key | 官方明确独立 token 后付费；不使用 Agents credits | 独立模板／文本 Chat 与无状态桥接，真实账号／账单待验；见[合同](providers/lumalabs-inference-contract.md) |

旧 `api.lumalabs.ai/dream-machine/v1` 是另一个历史 surface，既不替代 App，也不证明
当前 Agents REST 与 Web 的额度关系。2026-11-07 的累计新增模板为 16；2026-11-19 另增独立 Inference 模板，不改变历史统计或整卡完成数。

## 2. 本轮联网来源与证据

本轮仅匿名 GET 官方公开页面与 discovery；未登录、未发送 MCP POST、未注册 client、
未调用 `initialize`、`tools/list` 或生成，也未执行下载的远端 JavaScript。
原始页面、JSON、实际获取 UTC 时间、HTTP 状态与 SHA-256：
[来源目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-20261107-public-agent/sources.json)。
该目录是临时调查证据，不是发布 payload 或长期测试夹具。

1. [官方产品信息](https://lumalabs.ai/llm-info)与[App](https://lumalabs.ai/app)：
   App 是 `app.lumalabs.ai` 创作 workspace；品牌展示 UNI-1.1、Ray3.2 及第三方模型。
   展示名不能自动翻译为 action 类型或账号权益；开发者 API 为独立 surface。
2. [Cookie 政策](https://lumalabs.ai/legal/cookie-policy)：`wos-session` 维护认证会话，
   `accessToken` 用于 App API，`refreshToken` 用于自动更新；`render_token` 用于 shared board SSR。
   这不提供 exchange/refresh wire，也不证明任意生成仅需一个 Cookie。
3. [MCP 产品页](https://lumalabs.ai/mcp)与
   [官方 Claude Connector Guide](https://lumalabs.ai/learning-center/articles/luma-claude-connector-guide)：
   指南发布于 2026-10-19，页面结构化数据最后更新 2026-11-04。原文明确：
   `The connector uses your Luma team's credits at the same rates as the app. There is no separate subscription.`
   普通 Luma 账号登录、选择 team、无需 API Key；结果进入该 team workspace/Asset Library，
   并非自动进入某个 board。应查询未完成的视频而非重跑，以免重复付费。

### 2.1 匿名 OAuth discovery

[资源 metadata](https://agents.lumalabs.ai/.well-known/oauth-protected-resource/mcp)返回：

```json
{"resource":"https://agents.lumalabs.ai/mcp","authorization_servers":["https://agents.lumalabs.ai"],"bearer_methods_supported":["header"]}
```

[AS metadata](https://agents.lumalabs.ai/.well-known/oauth-authorization-server)声明：

| 字段 | 公开值 |
| --- | --- |
| issuer | `https://agents.lumalabs.ai` |
| authorization_endpoint | `https://agents.lumalabs.ai/mcp/oauth/authorize` |
| token_endpoint | `https://auth.lumalabs.ai/oauth2/token` |
| registration_endpoint | `https://auth.lumalabs.ai/oauth2/register` |
| device_authorization_endpoint | `https://auth.lumalabs.ai/oauth2/device_authorization` |
| code challenge | `S256` |
| grants | authorization code、refresh token、device code、token exchange |
| scopes | `email offline_access openid profile` |
| token endpoint auth | `none client_secret_post client_secret_basic` |

还声明 `client_id_metadata_document_supported=true`。以上是服务端声明，不是 Gateway 已有
client、适用 scope、可自行注册或 refresh/device 业务已完成的证明。
匿名 GET `/mcp` 为 401，并通过 `WWW-Authenticate` 指向上述 metadata；其通用错误
`Missing or invalid API key` 不能推翻官方普通用户 OAuth 说明，也不证明 MCP 必须使用 REST Key。

## 3. 当前 board 实现与未证合同

2026-11-13补充：官方OAuth/MCP独立调查已完成一次真实授权及26项tools/list；详见本文第5节。它不证明下面board wire，也不替代正常Gateway产品实现。

当前代码默认 action 为 `create_image_uni_1`、`create_video_ray3_14`、
`text_to_music_elevenlabs_v1`；执行 signature → events SSE → actions → artifact URL。
它们是源码事实，**本轮没有取得登录后的当前 board 业务 chunk 或受控 wire**。
匿名访问 App 进入 AuthKit 注册入口，不足以冻结新 board 请求。

因此尚未确认当前 actions/event schema、全部状态、模型/参数、完整 Cookie/JWT 组合、
refresh、真实费用与签名资产来源。旧默认、营销名称、Agents REST 或 MCP 工具名不能补作证据。
本轮可先修已证的本地生命周期与不重放问题，但不能据此宣称当前业务合同已完整实现。

后续 [Web图片公共子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-luma-web-images.md)
已把 URL/b64 都接到无凭据公网 HTTPS、有界读取与完整 PNG/JPEG 解码，正常公共调用/Local
次数结算、失败取消退款有离线证据；这是资产内容和本地业务结果验证，不证明当前 board wire、
原任务归属、签名资产的精确发行方或真实供应商费用。音视频、精确 probe/UI 与完整渠道仍未闭合。

后续[原任务结果子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-luma-web-tasks.md)
已对现有board worker增加POST唯一output ID→completed事件绑定、完整任务上下文透传与取图前核对，
拒绝finalizing/partial SSE/旧URL-only；这只是内部fail-closed准入，不证明当前官网状态/schema或资产发行方。
actionId仍可null，Rust依赖可信browser executor提供POST→event关联，自动发现不冒称精确模型绑定。

### 3.1 本轮匿名当前 wire 调查

2026-11-07再次匿名GET App根页和`/board`，均重定向官方sign-up，最终auth页403；仍没有新App业务chunk。
官方[Dream Machine](https://dream-machine.lumalabs.ai/)匿名200，取得公开业务JS，含`section_*` WebSocket、
generation状态、复数`artifacts`合入本地`artifact`和`/api/photon/v2/generations/{id}`；未执行这些JS或连接WS。
[来源/时间/状态/SHA](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-task-public-20261107-agent/sources.json)
与[逐字片段/字符offset](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-task-public-20261107-agent/evidence.txt)
已保留。**另一个官方surface也不能证明新App使用相同wire**，本批未将其协议移植到现有board。

## 4. 接力顺序

2026-11-12 [单账号图片任务测试子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-luma-image-probe.md)
已接通配置模型/别名、prompt-only 图片、逐次费用确认和普通 sender 的管理测试及独立表单。
成功只证明任务上下文与图片字节，返回中性结果，不恢复模型健康；失败也不对未经绑定的所选模型或账号施加健康反馈。
这承接上文尚未接通的 probe/UI 代码缺口，不替代真实浏览器联合验收、当前 board wire 或模型/action 合同。

1. 保留三个渠道的精确身份与凭据池；`shared` 仅限定同 team App/MCP credits，
   不共享秘密、健康状态或自动路由回退。
2. OAuth/MCP 先核 Gateway client 身份/注册许可、redirect 与资源绑定，再冻结实际
   initialize/tools、任务查询/取消、资产/额度合同。复用原 grant、SecretPatch、revision/CAS、
   refresh publication、管理 probe/audit；不另起代理服务，不猜 tool schema。
3. board 继续合法取得当前公开业务源码或集中录制 wire，补会话、原任务身份/终态、
   有界且无凭据资产验证、公共结算、精确媒体测试与逐次费用 UI；不因 MCP 存在撤销 Web。
4. 实际额度、跨进程/Server、原生包、P50/P60/P70 分别验收。候选发现不能增加模板数或
   标记整卡 `offline_ready`，上游不可达也不能把占位当功能完成。

## 5. 2026-11-13 官方MCP目录实测

用户授权的新public client，PKCE S256、openid、精确loopback redirect及MCP resource，第三次有限等待收到有效回调。仅执行initialize/notifications/initialized/tools/list，协商版本2025-03-26，返回26项工具。前两次超时与MFA过程、目录哈希和退出证据见 [回执](progress/2026-11-13-provider-channel-expansion-luma-mcp-registration.md)。token仅存在调查进程内存，未保存；退出不等于服务端撤销。

| 类别 | 目录中公布的工具 |
| --- | --- |
| 会话及工作 | luma_session_open、luma_session_run_capability、luma_session_send_brief、luma_run_await、luma_session_await、luma_run_respond、luma_session_cancel |
| 工作区 | luma_workspace_view_asset、luma_workspace_upload_asset、luma_workspace_complete_upload、luma_workspace_list_assets、luma_workspace_list |
| 团队素材库 | luma_library_list_directory、luma_library_make_directory、luma_library_rename、luma_library_move、luma_library_save_asset、luma_library_upload_asset |
| 生成 | luma_generate_image、luma_generate_video、luma_generate_layering |
| 发现及用量 | luma_capability_list、luma_capability_describe、luma_model_list、luma_model_describe、luma_usage |

以下是已取得的schema/描述，不是这些工具的业务调用验收：

- session_open要求idempotency_key；workspace_key省略会使用团队共享默认工作区，不能作为不同Gateway调用者的隔离策略。返回session_id/workspace_id/state/expires_at/resource_uri/replayed。
- 图片、视频和layering均要求model/prompt/idempotency_key/session_id；返回run_id/kind/state，不是同步生成完毕。已发现的模型展示名不能代替实际model_list结果。
- run_await要求run_id，timeout_s为1–30秒；输出status/timed_out/outputs/pending/error/settled_with等。等待超时不能重新提交生成。brief/capability的后续以session_await为另一合同，不能混用为所有生成的统一成功标记。
- session_cancel输入schema的文字约束要求run_id/session_id恰一，end只随session_id；JSON required列表为空不表示空请求有效。取消单run不等于关闭整个session或撤销既有产物，这些描述仍需后续验证。
- usage区分团队、session和asset的credit口径；asset费用可为null，不能当0，也不能将planner用量伪装为render费用。本批未读取余额、实际模型或团队/作品信息。
- readOnlyHint/idempotentHint等annotations与工具说明只作参考数据，不是执行批准、可安全自动重试或账单保证。团队素材库写操作、生成、取消与上传均不能因tools/list可见就自动开放。

下一最小产品链为独立OAuth/MCP身份与管理授权、显式只读模型发现，然后才是经批准的单任务生成与结果/取消/费用验证。复用原grant、SecretPatch、revision/CAS等机制；调查客户端不硬编码为发布默认。刷新、重新登录、持久保存、实际业务终态、真实费用、完整Gateway公共调用与包验收仍未完成。

## 6. Rust授权底座增量

2026-11-13 [实现回执](progress/2026-11-13-provider-channel-expansion-luma-oauth-core.md)：`protocol/luma_oauth`已实现显式已登记client、S256/openid/resource授权准备、严格回调、非Clone的一次性code grant及固定目的地form交换。14项本地定向测试和非测试编译通过；不是再次真实账号调用。

请求有20分钟授权期限、最多15秒send/read总预算及64KiB已解码响应上限，禁止跳转/继承默认headers，本owner不重试。未来调用者须传受控OAuth HTTP client，并先validate_callback再移交Authorization，避免错误state消费待授权状态。TokenMaterial不序列化、不声明身份，expires_in缺失保持未知，id_token/refresh_token不采纳。

本增量未新增管理API、保存/重新授权UI、动态注册入口、refresh、MCP transport或模型发现，也未修改原App Web/Agents REST的凭据和调度。正常管理与SecretPatch/revision/CAS接线仍为下一步，不将底座或调查client当可交付的完整渠道。

## 7. Rust只读MCP目录链路增量

2026-11-13 [实现及验证回执](progress/2026-11-13-provider-channel-expansion-luma-mcp-core.md)：`protocol/luma_mcp::inspect_tools`承接TokenMaterial，仅访问固定RESOURCE，完成initialize、initialized和分页tools/list。12项本地定向测试通过；本批没有再次真实上游调用。

这是有限的只读inspection profile，不是完整MCP客户端。支持JSON/SSE与精确请求关联、独立session/header隔离、整体30秒及累计4MiB读取预算，分页失败不返回部分目录。目录及schema只是数据，不执行其中说明或引用。调用方须提供专用无Cookie jar、无自定义retry的HTTP client。

取消释放本地HTTP body，但未实现远端session DELETE、撤销、GET/SSE监听或恢复。管理保存/UI、动态注册入口、refresh、模型发现与实际工具调用仍未接通；不修改原Web/REST账号，不标记整渠道完成。最新编译与静态检查状态以本节回执为准。

## 8. 授权材料持久化前置合同

2026-11-13 [实现回执](progress/2026-11-13-provider-channel-expansion-luma-context.md)：新增非秘密AuthorizationContext，固定issuer/resource/token endpoint，保留client、精确redirect和原绝对expiry；与单独秘密重组后仍使用原TokenMaterial/inspect_tools入口。未知expiry不补默认寿命，过期metadata可读取但不能恢复使用，正常往返不延长期限。Luma OAuth本地定向17项通过。

这不是实际账号保存或防篡改容器。未来必须把context（含expiry）纳入正常Keep目的地绑定和秘密批准，不能宣称任意metadata与bearer的关联已被验证。现有Agents静态Key对整个host的ownership与/mcp冲突，后续须同时实现独立MCP准入、保存/CAS和专用消费者；不简单放宽既有Key保护、不交给通用sender/probe。本批不新增路由schema、管理API或UI。

## 9. 只读MCP账号的正常保存与专用管理接口

2026-11-13 [实现回执](progress/2026-11-13-provider-channel-expansion-luma-management.md)：新增credential-only `luma_oauth`字段，独立`lumalabs_mcp_readonly`存储profile；仅在配置归属层与Agents区分，不放宽原协议层静态Key保护。正常console route-config保存走SecretPatch/编译/revision CAS，context（含expiry）进入Keep完整绑定；DB合并不继承父账号context。

账号强制disabled、显式credential池、固定MCP resource，无模型/refresh/自动probe/杂交授权。空池、清空秘密和过期metadata可管理保存；普通推理、通用HTTP probe、modelsPath发现和balance查询不得消费该profile的token。没有新增builtin preset或业务模板。

`POST /v1/internal/gateway/console/luma-mcp-tools`从已保存账号恢复材料，仅接已有只读tools/list链路；前后管理身份/secret consent/revision检查、并发有界、所有响应no-store，不接受用户提供的token/URL/tool参数，不写模型健康或配置。129项相关测试通过，2项需独立浏览器的专项未执行；非测试编译状态以回执为准。

本节承接上节的schema/Keep/正常保存及专用消费者缺口，不代表OAuth授权会话、一键保存UI、动态注册、refresh、生成业务或真实账号联合验收已完成；独立DB账号完整产品流程亦不在本批范围。

## 10. 后台授权会话与显式保存

2026-11-13 [实现及验证回执](progress/2026-11-13-provider-channel-expansion-luma-login.md)：新增`/console/luma-auth-sessions`创建/查询及cancel、complete、import动作。运行实例先绑定已登记loopback端口，再返回授权URL；正确回调一次性交换code后停在Ready，仅显式且经管理身份、secret consent批准的import才能正常保存。重授权保留credential ID、账号元数据和client/redirect约束，不断言同一上游用户身份。

会话有owner/revision隔离、20分钟窗口、固定容量、取消与shutdown/join；错误state不消费授权，交换中断不自动重放。保存由运行实例持有，20秒预算及publication lock后期限检查、CAS防止陈旧覆盖；重复import不重复提交。全部响应no-store，不回显token，不自动调用工具目录或生成。

相关本地测试119项通过、0失败，2项独立浏览器专项未执行；实际使用管理Router、loopback socket与synthetic token peer，非测试编译及最终静态检查见回执。修复了保存future提前释放guard的问题并保留失败证据。

本节补齐后台授权会话，不代表桌面一键授权/保存UI、动态注册、refresh、真实账号联合验收或生成业务完成。loopback属于Gateway所在机器，跨机器浏览器/远程Server流程未验收；不增加业务模板、不出包。

## 11. 已保存MCP配置的桌面授权入口

2026-11-13 [UI实现及验证回执](progress/2026-11-13-provider-channel-expansion-luma-ui.md)：现有MCP配置的新增账号与重新授权已接通后台会话；显式登记client/redirect，重授权锁定原账号登记信息，只有明确保存才提交。专用Dialog不展示该profile不支持的通用Key/模型编辑器；既有Web/Agents入口不变。复用正常管理权限、revision与保存回执机制，不自动读取工具目录或生成。

前端与原生浏览器仅允许固定MCP授权端点。前端相关77项去重测试、原生浏览器11项及真实前端入口构建通过；类型/静态检查和本地模拟截图见回执。视觉夹具使用真实组件和模拟回执，不当原生整包或真实账号验收。

仍需已保存的MCP服务商配置和已登记client；目录创建空池、动态注册、工具目录UI、refresh、真实账号联合验收及业务调用未完成。跨机器授权未验收，不将此增量称完整渠道或无需配置的一键接入。

## 12. MCP 空池创建与只读工具目录界面

2026-11-13 [后续实现回执](progress/2026-11-13-provider-channel-expansion-luma-catalog-ui.md)：正常服务商目录已有独立 `lumalabs-mcp` 创建入口，默认 ID 为 `lumalabs-mcp-oauth`；输出固定 resource、无模型／自动任务的显式空池，经原配置保存流程管理。现有账号界面可显式读取 tools/list 并以纯文本展示 schema；管理身份、grant、账号与 revision 变化隔离旧结果，关闭时取消本地读取，不调用任何工具。

本节退役上节的空池创建和工具目录 UI 缺口，不等于动态 client 注册或模型发现。refresh、业务调用、远端 session DELETE、真实账号完整联合、跨机器授权及原生包仍未完成；本批窄屏 override 未生效，窄屏验收保留。

## 13. 动态 public-client 注册协议底座

2026-11-13 [注册底座回执](progress/2026-11-13-provider-channel-expansion-luma-registration-core.md)：本轮重新联网读取公开授权元数据确认注册端点；新增固定目的地的一次性注册请求所有者、精确 public-client／redirect 响应绑定及有界 HTTP 处理。无真实注册 POST，不采用调查 client 作为默认身份。

本增量尚未接管理操作回执、listener 所有权与 UI，动态注册产品入口仍未完成。响应丢失、超时或取消不代表服务器未注册；后续必须显式保存／查询操作结果，不自动再次注册。已存在的手工登记授权路径不变。

## 14. 注册管理回执与显式界面入口

2026-11-14 [管理／UI 回执](progress/2026-11-14-provider-channel-expansion-luma-registration-management.md)：动态注册已接正常管理入口。先准备本地 callback listener 和 owner-bound receipt，明确确认后由运行实例持有单次远端请求；HTTP 响应丢失不重放。既有只读账号界面可查询登记结果并显式将 client／redirect 填入原授权流程，不自动登录或保存。

此增量承接第 13 节的管理／UI 接线缺口；真实动态注册与账号联合、回执跨进程持久恢复仍未验收或未实现。只在尚未远端提交的准备过期或 revision 改变时允许重新准备；已接受或未知结果只能查询原回执。完整状态与验证边界以本节回执为准，refresh／模型／业务工具／session DELETE 等继续保留。

## 15. 只读目录的远端会话收尾

2026-11-14 [会话收尾回执](progress/2026-11-14-provider-channel-expansion-luma-mcp-cleanup.md)：目录成功、失败或读取超时后，对本次 initialize 返回的合法 session 尝试一次 HTTP DELETE；无 session 不发送。目录读取最多 30 秒，清理另有最多 2 秒预算，合计最多 32 秒；不重试、不跳转、不读取清理响应正文，不继承默认凭据或 Cookie。

目录返回类型和管理响应保持兼容，清理不覆盖原目录结果。仅在脱敏 debug 日志中区分 Closed、AlreadyClosed（404）、Unsupported（405）与 Unconfirmed；202、其他错误及超时不声称关闭成功。令牌已知过期则不发送清理请求，状态保留为 Unconfirmed。

本节替代此前“完全没有 DELETE”的缺口，但不保证服务端实际释放资源。调用方强制取消／runtime 关闭会释放本地 future，不创建额外持有令牌的后台任务，远端关闭仍未保证；真实 Luma DELETE、取消路径受监督清理与完整账号联合仍待验收。未新增 tools/call 或生成／取消业务工具。

## 16. 注册回执的同数据目录恢复

2026-11-14 [持久恢复回执](progress/2026-11-14-provider-channel-expansion-luma-registration-recovery.md)：复用 console 持久层及 OS writer lock，在远端请求前原子写入结果未知的注册回执；成功 client 也须先持久化再公开成功。Prepared 不落盘；接受过的未知／成功回执不自动淘汰或重放，最多 16 条。新文件为原 state root 下的 `console/luma-client-registrations.json`，不改变已有数据库版本或原账号材料。

GET 在内存缺失时按原 id／owner 读取；新界面准备操作按 owner／provider 找回原回执，不创建远端请求。恢复出的未知状态可在另一实例随后落盘成功时升级为成功，不被旧内存状态遮挡。损坏、不支持版本、重复身份、锁冲突或写入失败都不能当作空存储继续注册。

恢复限相同 state root 和原管理 token fingerprint／Host／client IP，不放宽原 owner 隔离，不承诺换 token、换端口、跨机器／多节点恢复。两个实例同时准备不同候选时只有持久 claim 胜者可发请求；失败候选保持未知，不冒充胜者的 id／redirect。需重新进入准备流程取得权威回执。实际验证范围见本节回执，不将本地重建测试称为真实平台／完整 EXE 崩溃验收。

## 17. 单次刷新协议底座（尚未启用账号刷新）

2026-11-14 [刷新底座回执](progress/2026-11-14-provider-channel-expansion-luma-refresh-core.md)：匿名联网核实官方 OAuth 元数据含 `refresh_token`、`offline_access` 和 public-client `none`。新增消费自身的 `RefreshGrant`，固定 token endpoint／resource／client，单次 15 秒、64 KiB 响应预算，不跳转、不继承默认凭据、不重试，也不额外申请 scope。

刷新成功后新 refresh token 替换旧值；仅响应省略该字段时保留旧值，显式 null、空值、重复字段及非法值均拒绝。access 仍走原有 token parser，旧 access 过期不阻止显式刷新，未知新 expiry 不伪造；结果与 Debug 不提供可序列化的秘密对象。

这只是协议底座。当前注册仍仅声明 `authorization_code`、登录仍仅申请 `openid`、登录响应仍丢弃 refresh token、MCP 账号仍 disabled 且配置禁止 refresh 字段；没有接入正常账号刷新。后续须明确获准的 offline 授权及 client 能力，经正常 SecretPatch 保存 refresh，并把 Luma context 纳入已有持久消费 guard／轮换结果／revision 发布链。调用本底座前必须先持久占用旧 grant，失败／取消不得直接重放。

## 18. 显式离线授权与正常秘密保存（自动刷新未开放）

2026-11-14 增量：管理创建请求可显式设置 `offlineAccess: true`，授权 URL 才申请 `openid offline_access` 并要求 `prompt=consent`。默认普通授权保持原行为，桌面尚不提供离线开关。已知 Gateway 注册回执中的 client 仍仅支持 `authorization_code`，按 client ID 跨 owner 拒绝离线请求；回执读取失败关闭准入，不自动升级注册或借用身份。外部已登记 client 是否实际有权限仍由官方授权服务决定。

首次离线交换必须收到合法 refresh token；缺失、null、重复、非法或超长值直接失败，不静默降级。普通登录仍忽略响应中的 refresh token。非秘密 context 新增默认 false、false 省略的 `offline_access`，旧格式保持可读；RefreshGrant 要求该标志并在结果中保留，不能从普通 context 直接构造刷新。

显式保存通过原 SecretPatch／revision 路径同时替换 access 与 refresh，普通重新授权清除旧 refresh，保留账号 ID 和其他元数据。Keep 的完整 destination 绑定覆盖 offline 标志。配置仅允许 offline context 携带合法 refresh；允许清空秘密后保留 context，不将其当成仍有可用离线凭据。账号仍必须 disabled，refresh endpoint／client／lifetime、自动 probe 和推理闸门不变。

本节替代第 17 节中“没有离线管理入口／无法保存 refresh”的源码缺口；不代表自动刷新已启用。持久防重放占用、轮换结果落盘／revision 发布、桌面离线授权及兼容 client 注册、真实账号与整包验收仍未完成。本轮不访问真实账号、不注册、不刷新、不生成或下载。验证结果见 [本轮回执](progress/2026-11-14-provider-channel-expansion-luma-offline-save.md)。

## 19. 刷新发送意图的持久防重放登记

2026-11-14 增量将 offline MCP 账号接入已有 `GrantRequest`／`GrantMaterial` 和永久 `Sending` 记录。固定 token endpoint、client、offline context、access／refresh 与 context 中的绝对 expiry 构成精确材料；允许 access 已过期，未知 expiry 保持未知。其他产品 context 混入、修改 endpoint/client、剥除 offline context 或破坏 expiry 一致性均拒绝。

可选 `lumaOauth` 扩展在缺省时不序列化，旧材料 ID 与记录版本不变。Luma context 进入材料 ID 和等价比较，但不改变 endpoint／client／refresh 的 family ID，因此修改 access 或 callback context 不能把同一刷新令牌当成未使用的新 family。复用原本地文件所有者、writer lock 和权威 revision；重复登记返回原记录，已改变但未确认的同 family 被阻止，旧 revision 失去准入资格。

本节仅支持发送意图登记。Luma 的普通 Result 构造、持久 Result／Published 读取及发布资格均明确关闭，不能借通用刷新结果流程绕过尚未完成的接收回执合同；未修改后台刷新枚举、通用 refresh_config、管理刷新入口或数据库刷新资格。下一步仍是保存收到的 successor 响应、解释／隔离无效结果及轮换发布；实际网络发送尚未接入。测试与边界见 [本轮回执](progress/2026-11-14-provider-channel-expansion-luma-grant-storage.md)。

## 20. 刷新响应回执与解释前保存

2026-11-14 增量新增消费自身的 `RefreshGrant::receive`，仍使用固定端点／resource、单次 15 秒与解码后 64 KiB 上限。完整成功 HTTP 响应先捕获为 `ReceivedRefresh`，不预先解析 token；重复 JSON 字段、非法 expiry、非 JSON 或非 UTF-8 响应都可作为有界证据保留。调用者仍须在发送前持久登记 Sending；非成功 HTTP、传输失败、取消、超时或超限不能作为释放先前登记或授权重发的理由。

回执保存精确 bytes 的 canonical base64、原发送时刻，以及完整 context／旧 refresh 的无歧义 hash 绑定；读盘与解释时重查长度、编码、时间格式和 grant 绑定。base64 不是加密，hash 不是文件真实性证明；秘密保护及存储完整性继续由原持久层负责。解释沿用已有 token 校验和原发送时间，不能通过晚读／重启延长 expiry；缺省 refresh 仍保留原值，显式畸形 refresh 不降级。

新增 `GrantReceipt::LumaOauth`，只在原 Sending 上附加回执，不改变 locator、版本或阶段。相同回执写入幂等，不能替换正文或时间 anchor；即使 authority 随后改变，仍可保存原请求已收到的响应，但只有精确原 revision／credential／material 能恢复该回执。原 family 仍保持防重放约束。

本节接通协议回执和现有持久保存合同，不代表正常账号后台执行器已接通。兼容 `exchange` 仍是调用者负责持久保护的低层入口；后续执行器必须使用 receive→持久确认→解释流程。Luma Result／Published 继续关闭，通用 worker 对新回执明确拒绝发布；未新增 worker 构造器、刷新调度、UI 或真实上游请求。结果接纳、轮换发布及其恢复仍待完成。验证范围见 [本轮回执](progress/2026-11-14-provider-channel-expansion-luma-received.md)。

## 21. 正常账号显式刷新与本地恢复

2026-11-14 后续 [完整流程回执](progress/2026-11-14-provider-channel-expansion-luma-explicit-refresh.md) 替代第 18–20 节中相应的源码缺口：桌面明确 offline 选项；独立 refresh-capable client 注册；正常账号 prepare／import／resume 管理入口；永久 Sending、原始 receipt、稳定 Result、有限 SecretPatch 投影、原 revision 事务发布及本地恢复均已接通并通过本地定向验证。

新注册能力字段缺省 false，旧回执仍为 code-only；同 owner／provider 的两种注册能力各自保留独立回执，不修改旧 client。后端按 client ID 与回调约束拒绝把已知 code-only client 用于 offline 授权。只有明确确认注册才发送一次注册请求。

刷新准备不发送上游；确认任务由现有 Registry JoinSet 持有，使用 `/v1/internal/gateway/console/luma-refresh-sessions` 及其 `/:id`、`/:id/import`、`/:id/resume` 入口。import 的新发送必须赢得持久 Reserved；resume 不允许发送。收到但未存盘的响应在有界 session 中保留；已持久 receipt／Result 在重建后可恢复；无响应证据的未知 Sending 不重发。历史结果校验不依赖当前时钟，而工具调用仍拒绝过期 bearer。

有限投影仅更新原 credential 的 access、refresh、Luma context，保留 disabled 与其他策略；复用正常秘密保存、原事务 journal 和 publication locator。未启用定时刷新、推理或通用 refresh_config。两种后端的持锁恢复接口避免本地锁重入；只读 Luma 不需要推理缓存修复。

随后同日真实验收已追加在上述完整流程回执：正常注册/授权/保存、一次显式刷新、前后 26 项目录及同目录正常原生重启通过。真实模型读取、强制崩溃、远端 Server、跨机器、MCP 创作及覆盖后续源码的新包仍须分别验收；不得用目录读取或本地测试将整个模块标为完成。

## 22. 当前目录合同快照

2026-11-20 接续核实[官方插件仓库](https://github.com/lumalabs/luma-plugins/tree/e02d83de796d56525ef64376fd812b1f2cce036b)：该 commit 的完整 tree 只有连接配置、说明、license 与图标，没有工具 schema 或生成实现；其 `luma/.mcp.json` 仍指向固定官方 resource。公开连接指南也未提供生成结果的完整合同，不能把旧 API Key 版 `luma-api-mcp` 的 Photon/Ray2 合同当成本渠道的 OAuth 工具合同。

当前目录读取保留 `annotations` 对象，与输入／输出 schema 一起作为不可信数据；仍受原分页、工具数与响应总量限制，不据 hints 自动执行或重试。正常已保存账号面板增加显式 JSON 导出，复用成功结果的账号／授权／revision 范围，记录浏览器读取完成的 UTC 时间。导出只保存目录字段及固定协议元数据，不包含连接身份和 transport；既不额外访问上游，也不保存秘密、写配置或执行工具。缺省 annotations 为 null，旧服务端响应仍可读取。

快照是规范化目录，未保留顶层 `_meta`、额外扩展或原始传输字节；其中供应商 schema／说明保持原文，归档前仍需检查。即使目录包含输出 schema，仍须核实会话隔离、等待／取消、资产与费用的业务语义。具体使用见 [持续开发入口](../tools/luma/README.md#保存-mcp-当前工具合同)，本批验证见[目录快照子批](progress/2026-11-20-provider-channel-expansion-luma-mcp-snapshot.md)。
