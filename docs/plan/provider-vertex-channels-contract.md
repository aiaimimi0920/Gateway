# Vertex 分渠道实施合同

日期：2026-11-05。对应 [总计划 R11/W07](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。
本合同区分 Express API Key 与 Full Vertex OAuth，不用某一子链代替另一渠道，也不把协议合成测试当作真实权益验收。

## 1. 产品身份

| 渠道 | 模板方向 / preset | 材料和目标 | 状态边界 |
| --- | --- | --- | --- |
| Full Vertex | `vertex-oauth` 正常空池模板 / `google-agent-platform-official-api`；保留旧 aliases，不自动转换 generic 旧配置 | 独立 Google 用户 OAuth 或经明确合同导入；project/location/publisher/model | 独立wire/context、原schema/秘密管理、持久刷新/publication/replay/cache与Bearer/quota已接；正常管理/UI已连真实TCP/Local SQLite，显式client/target、稳定ID重授权/模型Keep、原scheduler刷新、两次runtime重建和普通Key/Chat JSON/聚合SSE已有本地合成证据。typed身份贯穿卡片/下拉/表格/同版本summary；三协议JSON/聚合SSE、Token Key、严格响应准入/坏流/取消退款，以及完整工具/有序签名turn/Preview顺序构造子集已有产品独立本地pipeline证据；共享算法不共享source/凭据/额度，完整decoder/模态、实际Redis/PG/Hosted、跨进程receipt/原生EXE和真实验收继续，不能借用Antigravity client/token |
| Vertex Express | `vertex-express-api` / 同名独立 preset；profile `vertex_express_api` | 平台 Express Key；固定 global publisher-model REST | 后端身份/最终发送、正常 Console/暂存/精确 probe、公共 router/SQLite 结算/错误/取消及正常 UI→真实 TCP/SQLite/同进程 runtime 重建已有默认栈定向离线证据；完整工具、Preview 参数构造子集及显式有序签名 turn 分批实施，完整 decoder 和真实验收继续 |

两者额度关系为 `unknown`。Google 同邮箱、相同 Gemini 模型名、相同 JSON 格式或“升级到完整产品”的入口，都不证明余额互通。
Express 不并入 Generative Language/AI Studio Key 池，不迁移现有 Vertex aliases 或任意旧 custom-path 配置。

## 2. 公开合同来源

2026-11-05 联网只读核实，无实际 Key、授权、生成或刷新请求：

1. [Express REST reference](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/start/express-mode/vertex-ai-express-mode-api-reference)：页面更新于 2026-11-04 UTC；
   明确 Express 使用 `x-goog-api-key` header 或 `key` query，固定全局 `aiplatform.googleapis.com`，路径不含 `projects` / `locations`。
   页面当前称 Gemini Enterprise Agent Platform in express mode，标注 Preview；不据此批量改写存量产品名。
2. [v1 generateContent](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/publishers.models/generateContent)：
   `POST /v1/{model}:generateContent`，publisher resource 为 `publishers/google/models/MODEL_ID`。
3. [v1 streamGenerateContent](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/publishers.models/streamGenerateContent)：
   同资源的流式方法，返回 `GenerateContentResponse` 实例流；不能外推所有模型、模态、免费额度或成功终态的实际样本。
4. [Express overview](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/start/express-mode/overview)：产品子集与升级背景；不是本批额度共享的证明。
5. [GenerateContentResponse](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse)：同日联网只读核实，
   `finishReason` 为空表示尚未停止，candidate index 区分多个候选；total 包含 prompt/candidate/tool-use/thought tokens。
   终态枚举与报告总量准入依据该页；不因此宣称已支持完整多 choice、reasoning 或全部模态。
6. [thought signatures](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/thinking/thought-signatures)、
   [Content REST](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/Shared.Types/Content) 及
   [v1 content.proto](https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1/content.proto)：
   同日联网只读核实 Part 级 thought/signature、原 Part 回传、不合并签名 Parts、流式尾部空 text 签名。
   未定义签名与 Preview partialArgs 的精确关联，不推断账号/Key/project 绑定、TTL 或全部 Express 模型资格。
7. [Google native app OAuth](https://developers.google.com/identity/protocols/oauth2/native-app)、
   [Vertex authentication](https://docs.cloud.google.com/vertex-ai/docs/authentication) 及
   [Full Vertex generateContent](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/projects.locations.publishers.models/generateContent)：
   同日联网只读核实desktop public client、loopback/PKCE、code/refresh表单与Bearer，当前client_secret为Optional；
   `refresh_token_expires_in`可表示time-based期限。完整native-app页面及提取文本另存
   `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-20261105/`，不以截断摘要替代完整合同。
8. [Deployments and endpoints](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/learn/locations) 与
   [Set the quota project](https://docs.cloud.google.com/docs/quotas/set-quota-project)：同日联网只读核实 global/regional endpoint 与
   `x-goog-user-project`/调用主体权限；只核语法与目标合同，不外推完整区域/模型资格或额度。
   原文另存 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-context-20261105/public-sources.json`。
   补查 [canonical locations](https://docs.cloud.google.com/gemini-enterprise-agent-platform/resources/locations) 明确 `LOCATION-aiplatform`、global无前缀，以及
   `us`/`eu` 的专用 `.rep.googleapis.com` multi-region endpoint；原文在同目录 `canonical-locations.json`，不套普通区域公式。

本批方法参考抓取原文保存在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-20261105/public-sources.json`。
响应参考另存 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-pipeline-20261105/public-response-sources.json`；
同批 `/Content` 页面为另一种 union schema，未作为本批旧 parts 合同的实现依据。
同时尝试的旧 authentication URL 返回 `CRAWL_NOT_FOUND`，未作为实现证据。不复制第三方实现、不新增依赖。
后续签名 turn 子批使用上列明确的 Part schema，原始来源另存
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-signed-turn-20261105/public-sources.json`；
旧 Gemini 签名 URL 重定向后的材料另存 `public-sentinel-source.json`，不把它冒称当前 Cloud 哨兵合同。

## 3. Express 的最小后端合同

- preset：`vertex-express-api`；内部 `auth_mode=vertex_express_api_key` 只保存产品身份，实际认证仍是 API Key，不是 OAuth。
- 继续使用 `gemini_api_compatible`、普通 `direct_http` 和现有三适配器准备链；不新增专用转发服务。
- base 固定为 `https://aiplatform.googleapis.com/v1/publishers/google`（允许唯一末尾 `/`）。
  复用 Gemini 默认 `/models/{model}:generateContent` / `:streamGenerateContent`，无需修改共享 Gemini planner 内部。
- 模型是明确的单个 resource segment，沿已有 2048-byte 总结构预算；拒绝完整 resource、斜杠、URL 编码、query、fragment、wildcard。
  不凭 Key 格式猜产品，不提供猜测默认模型、跨产品模型 union 或自动 `/models` 探测。
- 空池和显式清空 Key 可以保存；发送要求非空、有界、合法 HTTP header 材料。不得回退 OAuth Bearer 或其他账号 Key。
- 原始 preset 在合并前锚定；父级和各账号最终 payload 均复核。拒绝 OAuth refresh/client/context、session、keepalive、discovery、
  借用目录、custom path/header/extra-body、非 DirectHttp 和不支持的指纹/调用格式。
- 普通 Keep 复用完整 destination/consumer 比较。metadata/model 编辑可保留 Key；无 secret access 的 Keep 不能换产品、受众、
  profile/flow、refresh 目标或新增继承消费者。Replace/Clear 沿原 SecretPatch owner，不创建新的秘密存储。
- 最终发送再次验证完整 URL、method、模型 segment、query 与材料，再由 Express owner 重建认证头。
  移除 caller/provider 的 Cookie、Bearer、Host、Key 覆盖及其他 attribution header；Key 不进入 URL/query。
- 禁止跟随重定向；无凭据测试只在所有产品校验之后替换最终 transport 为非转发 loopback server。
- 本批只开放已有 Gemini canonical 文本调用路径。媒体 JSON/binary 捷径在网络前拒绝，不借 AI Studio 媒体 sender 发送 Express Key。
  共享 Gemini 输出仍是先完整聚合再桥接 SSE，不能描述为实时 token-by-token。
- Express 自己的响应准入复用现有 64 MiB bounded reader，要求 SSE/UTF-8/JSON、完整 frame 与单 candidate 的真实 finish；
  空流、仅 `[DONE]`、坏尾部、晚错误、finish 后新内容均不能被 EOF 补成成功。
  请求多个/非法 candidateCount 在网络前拒绝；响应多 choice 显式拒绝，不静默合并为一条答案。
- HTTP 错误保留原分类与恢复提示，但公共/audit 不透传任意上游诊断或 code；不完整响应 Abort，不重放未知消费。
  上游报告的 total/显式零/cache-read 保留，不以 prompt+completion 重算总量；缺 usage/total 按已有未知 usage 预留策略处理。
  `thought=true` 不混入普通答案；完整工具 part/过滤已由独立 projection 接管。
  有序 text/完整 functionCall 的 thought/signature 由显式 `gateway_vertex_turn` / `gateway_vertex_history` 承载，
  不冒充其他公共协议原生 reasoning，不猜签名与 Preview 分片归属；多 choice、其他 Part/模态及完整 decoder 仍需完成。
  固定 v1 的工具历史由 Express 独立 owner 按公共 ID 关联并聚合/排序，不把 v1beta1 的调用 ID 直接发送到 v1。

Express 与旧 Full Vertex 复用 `line-google-agent-platform-official` 编译开关，不因此共享运行凭据、额度或产品验收。
旧 manifest 的 bearer/models 能力描述不能直接视作 Express 的能力声明；正常模板已接入手工模型，不调用或借用旧目录。

## 4. 下一步及不可省略的边界

1. Express：正常空池模板、Key 添加/metadata Keep/清空、受众绑定暂存、手工模型与精确账号测试已有
   [Console 子批证据](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-console.md)。
   该批管理保存使用真实 router+合成 backend，浏览器最终保存明确模拟；后续
   [真实本地 UI 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-local-ui.md)
   已连接正常 main UI→管理 TCP→SQLite，完成模板/Key/metadata Keep/清空、两次同目录同端口 runtime 重建与公共 JSON/SSE，余额 4→3→2，清空后 400 且不多发或扣费。
   真实管理 JSON 的 null 可选字段误拒绝已红测修复，7 项新增回归保留非空 override 限制；Rust 两项及前端 27 项定向通过。
   这是同测试进程 AppState 重建，不是原生 EXE/OS 重启或完整后台 scheduler 验收；不增整卡 offline_ready。
   不复用 AI Studio 官方目录，不通过导入任意 URL 绕过固定目的地。
2. Express：公共 router→route→发送→finalize 的三协议 JSON/聚合 SSE、正常预付费消息与 Token Key、SQLite usage/audit、
   caller Drop/source close/refund、坏流/HTTP 错误已有 [pipeline 子批证据](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-pipeline.md)。
   这是生产 local bootstrap + 正常管理 Key + router oneshot + loopback 上游，不是真实 Gateway TCP/UI、真实进程重启或 Google 账单。
   [工具/过滤子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-tool-decoder.md)
   已补完整工具 part 的独立身份/参数校验、九类过滤和生成失败分类、Messages refusal，以及 v1 同轮结果聚合/反序关联；
   三公共协议 JSON/SSE 两轮与真实 SQLite 结算、坏生成/坏参数 Abort、非法历史网络前拒绝已有默认栈证据（47 passed/1 ignored）。
   后续 [Preview 参数分片子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-preview-arguments.md)
   已接 Express 独立 assembler、嵌套/连续数组/转义成员路径、两层 continuation/空结束片，以及三公共协议的显式 boolean 开关，保留标准 tool_choice。
   65 passed/1 ignored（18新增）、相邻111项、三协议JSON/SSE两轮2→1→0、不完整/冲突/取消退款已有无management-ui运行栈证据。
   该 Preview v1/v1beta1 可用，不等于 v1beta1 call ID；不全局开启。仅单位置/顺序调用构造子集，未知重赋值、可识别身份冲突及空容器片段等明确拒绝，不承诺无 ID 同名多调用交错归属。
   后续 [有序签名 turn 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-signed-turns.md)
   已接 text/完整 functionCall 原 Part 顺序、空 text、thought/signature 与公共工具 ID，三公共协议 JSON/SSE 的显式扩展输出与历史校验回输。
   Raw 状态绕过和有效 contents 覆盖拒绝；Responses 保留原 extra 白名单，忽略的 contents 不能覆盖 typed 历史。
   Chat 签名参数不做 HTML entity repair 或字面 tool markup 注入；SSE 以有界 witness 检查实际已输出内容与 terminal 状态一致。
   最终运行栈87 passed/0 failed/1 ignored（22新增），同EXE相邻协议218 passed，去重303项；审查红测及Responses测试预期修正的失败证据保留。
   客户端必须显式保留扩展；忽略签名可能令后续工具请求被 Google 400 拒绝，不只是推理性能下降。
   参数分片与签名并存仍因关联未知而拒绝。当前子集不等于完整 decoder；多 choice、未支持 Preview 表示/Part、全部声明能力和实时参数流继续。
   单独启用官方线的 feature-off 编译仍有共享 Gemini Web 的15项缺符号失败；启用所需 Web core 的代表性组合通过，不报完整矩阵绿。
3. Full Vertex OAuth：[独立desktop OAuth wire子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-wire.md)
   已实现显式client/可选secret、固定Google授权与token origin、cloud-platform、PKCE、一次callback/code/refresh、
   access/refresh绝对到期与不延长旧refresh期限、bounded HTTP/取消/错误脱敏；不读取ADC或借用Antigravity材料。
   默认栈24项（23新增）、同EXE相邻OAuth102项与代表性关闭14项（1项专属关闭）通过，按名去重127项；来源/差异边界见回执，不冒充完整feature矩阵。
   不因此开放新模板或增加整卡offline_ready。
   后续 [typed context/目标子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-context.md)
   已实现独立closed context、显式client/grant/requested scope、project/location/quota-project、global/regional/显式multi-region base和只缩短不延长的RT期限/rotation比较。
   默认栈42项（18新增）、代表性关闭30项、同EXE相邻OAuth161项通过，按名去重204项；完整边界见新回执。
   该子批当时尚未注册到schema；后续 [credential-only/持久刷新与发送子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-lifecycle.md)
   已接原声明/最终payload、DB stripping、nested SecretPatch/Keep、原durable scheduler/publication/replay/cache与最终Bearer/quota。
   scope-report与refresh绝对期限按合法rotation贯穿，缺省extension保留旧hash；Local重建/Result磁盘重试和四类目标buffered/聚合SSE有合成运行证据。
   新typed渠道在网络前拒绝legacy `auth_token`并用当前access重建认证，不改变或迁移旧generic Gemini配置。
   后续[正常管理后端子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-management.md)已接真实loopback、单次callback/手动complete、正常create/status/cancel/import、显式client/target、原SecretPatch/原revision-CAS保存和bounded/joined生命周期。
   新账号的quota从捕获context派生，稳定ID重授权核完整target；callback不自动保存，未知保存不重交。默认83项、代表性关闭40项和生产非测试编译通过，去重457项（22新增）；5 ignored不算通过。
   后续[正常UI/模板子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-ui.md)已开放独立空池模板、专用API client/显式client和target、稳定ID重授权、metadata Keep与同管理身份unknown保存关闭重开；保存前会话回收可明确重新授权，尝试保存后不可用仍保留防重复栅栏。
   前端76文件最终去重556 passed/0 failed/1 skipped（74新增），类型/隔离构建/合成浏览器与静态检查通过；外部package/lock路径漂移保留并分类，v2选定902输入无漂移、22个最终产物与浏览器版本逐项同hash，不冒称clean install。
   上述UI子批当时的模拟保存不等于本地业务闭环；后续[真实本地管理/UI子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-local-ui.md)已用正常main连接真实TCP/SQLite，完成授权/明确保存、模型Keep、原scheduler一次刷新、稳定ID无secret重授权与两次同目录同端口runtime重建。
   正常message-prepaid Key的公共Chat JSON/聚合SSE三次200、余额4→3→2→1；不等于进程/OS重启或真实Google。默认栈溢出在外层复用同任务聚合分支修复，不增栈或另起task。
   卡片/下拉/表格与同revision summary的typed身份透传通过；旧generic不自动改OAuth。Rust170 passed/2 ignored、显式浏览器fixture另1 passed；前端18文件187 passed，类型/最终隔离构建与静态/来源审计通过，截图版本边界见回执。
   Full Vertex三协议/Token Key/坏流/取消退款、完整decoder/工具/签名/模态、Server/Worker实际Redis、PG/Hosted、跨进程receipt/原生EXE与真实验收仍缺，不借Express成果标整卡完成。
4. Full Vertex 的区域/global 支持和实际 publisher/model 范围须根据官方合同与最终材料验证；不从 Express 固定 global 推断。
5. 真实 Key、OAuth client/账号、刷新、模型和额度留到集中录制验收；本批不请求用户提前录制，也不读取历史凭据或真实 `.ng`。

### Full Vertex 公共 pipeline 后续（2026-11-05）

[三协议与响应准入子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-pipeline.md)
已完成 Full Vertex 自己的生产 Local bootstrap→正常管理 Key→公共 router oneshot→typed OAuth target/Bearer/quota→真实 SQLite audit/余额。
三协议 JSON/聚合 SSE、消息/Token Key、报告 total=0/17 与 cache-read、unknown 预留语义、坏流/取消/source close/退款，以及 Chat HTTP 错误分类/脱敏有定向证据。
与 Express 共享的是同一个 Vertex REST 的完整 SSE/frame/单 candidate/真实 finish/usage 校验，不共享产品身份或 ordered-turn 投影。
共享 SSE usage tap 重算总量的问题已实测失败后修复；保留 native 分段计数和 provisional message_start。
默认栈184 passed/2 ignored，受影响回归按名去重524 passed/0 failed/2 ignored（19新增）；生产非测试和代表性关闭编译、静态/来源审计通过。
这一后续替代上方第3项末尾当时“三协议/Token Key/坏流/取消退款尚缺”的状态，不删除历史结果。
该 pipeline 子批当时 Full Vertex owner 只投影文本；完整 functionCall、签名 text 当时明确拒绝，现由下列工具子批承接，其他 Part 仍按实际支持范围准入，不冒称完整 decoder 或所有 Gemini 模型可用。
完整工具/history/签名/Preview/模态、多 choice、跨候选/完整feature矩阵、实际Redis/PG/Hosted、跨进程receipt/原生EXE及真实模型/额度继续，累计模板仍15，不标整卡或总计划完成。

### Full Vertex 工具与有序签名后续（2026-11-05）

[工具/history/签名子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-tools.md)
已补完整 functionCall、同名调用独立公共ID、v1并行结果关联/重排/同Content，以及原Part顺序/空text/thought/signature的三公共协议JSON/聚合SSE两轮往返，真实SQLite余额2→1→0。
Express的projection/Preview/turn/history状态机提取到共享Vertex owner，两者用显式source/profile/error合同绑定；Full turn source为`vertex_user_oauth`，Express仍为`vertex_express_api`，历史回传两边均校验source，不因共享算法而混池。
旧Express-only全局gate仅新增typed Full Vertex通道；旧generic不自动转换，其他产品仍拒绝历史扩展。
Full Vertex显式Preview boolean开关和既有顺序构造子集、tool_choice保留、Raw状态/有效contents覆盖/未知结果拒绝、坏上游不重放/不成功扣费、过滤丢工具/签名及取消source close/退款已有产品独立证据。
默认栈195 passed/2 ignored，受影响三协议/Gemini按名去重390 passed/0 failed/2 ignored（11新增），非测试库和代表性关闭编译通过；2998输入同hash，静态/来源回执见新文档。
这替代上方当时“完整工具/history/签名/Preview尚缺”的对应子集，不扩张Preview、全部模态或多choice支持；仍是聚合桥接，signed Preview关联未知继续拒绝。
完整decoder/未支持Part和Preview/模态、多choice/跨候选/完整feature矩阵、实际Redis/PG/Hosted、跨进程receipt/原生EXE、其他原定渠道和真实模型/额度继续，模板仍15，不增整卡offline_ready或总目标完成。

本批具体通过/失败记录以 [Express 后端回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express.md) 为准。
未完成项不从原 R11/W07 范围移除；不标总计划 P50、整卡 offline_ready 或整体完成。
