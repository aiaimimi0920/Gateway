# Perplexity Web 单轮文本 owner

> 2026-11-04；P37–P39 底层/材料/来源前置之上，P40 已接正常模板、材料表单、专用 dispatcher 和精确 probe。离线验证状态见 [P40 回执](progress/2026-11-04-provider-channel-expansion-p40.md)；R12 / W08 仍不是完整渠道或真实账号验收。

2026-11-08 增量：流式入口已接同一 HTTP/严格 decoder owner 的实时可替换快照；这不是标准 token 追加。标准正文/最终来源仍等合法应用终态后输出，见下节及[快照子批回执](progress/2026-11-08-provider-channel-expansion-perplexity-snapshots.md)。

## 产品与来源

消费端 `www.perplexity.ai` 与既有 `perplexity-api` / `perplexity-search` 分离，不提供平台 API Key 认证，不假定订阅、额度或模型互通。请求和答案格式按[固定公开参考合同](plan/provider-perplexity-web-contract.md)实现；版本 `2.18`、`turbo`、`concise` 均是 pinned 观察值，不是官方长期稳定或账号有权调用的承诺。

Gateway 自行实现 request/parser/HTTP owner，没有运行或引入参考项目、Python/browser CLI、邮箱注册或第三方账号服务。[参考 MIT 许可与版权](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/perplexity_web/REFERENCE-LICENSE.txt)随源码保留。开源许可不替代厂商自动化使用授权；未来出包仍须将适用 notice 纳入发布载荷，本批未出包。

## 已实现的精确子集

- `protocol/perplexity_web/request.rs`：Chat/Responses 的一个 user message、一个纯文本 part，选路后精确策略为 `turbo`，允许 caller alias 映射到该策略；输出 pinned auto/concise profile 的完整 body和两个新的 UUIDv4，不改写 caller 请求或拼接多轮，不透传 caller extra。其他模型策略、Messages、tools、reasoning、附件、上下文续接和未支持生成参数在网络前拒绝。
- `upstream/perplexity_web/Session`：只接受显式的未分片 `__Secure-next-auth.session-token` 值、可选 `cf_clearance` 值和非空 UA。拒绝 Cookie 注入与控制字符，Cookie 标记 sensitive，类型不实现 Debug；仅 challenge 不能构造 session。构造成功只证明形状有效，不证明登录或权限。
- 普通 `execute` 必须经 `Session::from_payload` 复验完整材料/目的地/绝对 expiry，总预算取全局 timeout 与本地120秒的较小值。低层 `execute_single_turn` 仍要求显式正值且不超过120秒。生产入口固定 `POST https://www.perplexity.ai/rest/sse/perplexity_ask`，使用现有 HTTP client，不另起转发服务；禁止 client 默认 API headers、caller Authorization/Cookie/UA 和所有重定向。没有可配置 URL 或环境变量测试后门，loopback 只在私有模块测试中使用。
- `decode.rs`：复用共享 `BoundedSseDecoder::new_strict`，支持 UTF-8 分片、CRLF、多行 data 和开头 BOM；答案采用 `ask_text.markdown_block.answer` 完整替换，或严格嵌套的 legacy `FINAL.content.answer` JSON。不将未知 markdown、顶层任意 answer/text 或所有字符串累计成回答。
- auto/legacy profile 只在 `event: end_of_stream` 且最后一个 JSON 数据事件含非空答案时完成。`final_sse_message` 单独出现、提前 EOF、事件截断都不算成功。终态只接受空 data 或空 JSON 对象；携带其他数据的终态明确拒绝，不能跳过错误再用旧 partial 结算。
- 来源仅从最终答案同一事件中的直接 `web_result_block.web_results` 取 `name/url/snippet`；不继承旧快照来源，不声称已证明 citation marker 一一绑定。URL 只允许不带用户名/密码的 HTTP(S)，不自动抓取来源。未知普通元数据、`read_write_token` 和上下文 ID 不进入返回值。

`Answer` 只包含文本和上述来源列表；不捏造 usage、剩余额度、最终模型身份或登录成功状态。非流式仍聚合最终答案；流式可提前交付下述产品快照，但标准正文不宣称实时 token streaming。

## 实时快照与标准正文

原生答案是完整替换，不保证仅增长。消费者必须整体替换当前临时答案及其来源，不能拼接快照字符串，也不能把临时内容写入不可撤回的标准正文。

Chat SSE 使用命名事件 `gateway.perplexity.snapshot`，data 形状如下。这是 Gateway 显式扩展，不是 OpenAI 标准 delta；普通兼容客户端可忽略。

```json
{"gateway_perplexity_snapshot":{"version":1,"sequence":1,"answer":{"text":"临时答案","sources":[]}},"choices":[]}
```

- `sequence` 从1开始逐次递增，受原4096事件预算限制；`answer: null` 表示当前事件无答案、清除旧临时显示。合法空字符串也保留为空快照，但不能据此成功结束。
- Responses SSE 使用同名命名事件，data 为 `{"type":"gateway.perplexity.snapshot","sequence_number":3,"response_id":"resp_...","snapshot":{...}}`；其中 `snapshot` 就是上述 version/sequence/answer 对象。标准 `sequence_number` 和产品快照 `sequence` 是不同序列。快照先关联本次已创建的 response，不创建标准 message/output_text。
- 标准 Chat `delta.content` / Responses `response.output_text.delta` 只交付最终答案一次，最终来源只取该答案同一原生事件。缩短、改写、清空与旧来源不污染最终正文，不将 `read_write_token`、context ID 或原生任意 metadata 暴露为扩展。
- 原合法 `end_of_stream` 是应用完成边界，**不是实际 TCP EOF**；完成后释放 HTTP body，不等保持连接的服务器主动关闭。没有该终态、最后快照为空、未支持 patch/profile、坏帧、网络或超时都失败。已交付 headers 后以 body 错误中断，不再伪造 HTTP 错误状态或成功 terminal。
- Responses 转换器复验载体形状、顺序及答案/来源预算；标准正文/工具/usage/终止已开始后不能插入快照，只有快照就 EOF 或 DONE 也不能伪造空成功。
- 原请求独占一个有界 decoder/HTTP body/总 deadline，不 spawn producer，不后台排队或重放。每次只取一个完整帧；总公共序列化预算32MiB，最终标准帧在交付正文前整体准入，待发队列最多3帧。此预算是本地安全限制，不是官方额度或底层解压器进程内存承诺。

2026-11-12 增加下述有限入站 markdown patch profile；未增加 multi-turn、Research、附件、其他模型策略、目录或会话恢复。最终标准正文保持聚合语义；仅理解这个扩展的客户端能实时显示可撤回结果。

### 有限 schematized markdown profile

`schematized.rs` 为独立响应状态 owner，识别 `ask_text_0_markdown`。不修改现有 concise/turbo 请求，不主动加入 `use_schematized_api` 或宣称支持所有 schematized 产品。公开依据见 [固定协议调查](plan/provider-perplexity-schematized-contract.md)。

- 接受 `field=markdown_block` 的根 `replace`（字符串 chunks）与严格连续 `/chunks/N` 的 `add`。不支持任意 JSON Patch、覆盖式 add、稀疏索引、未知 op 或非字符串值；最多4096 chunks/单事件 patches，累计正文仍限256KiB。
- PENDING 只输出可替换 snapshot，根替换和最终正文允许缩短/改写。`final:true`、`text_completed:true`、EOF 或 `end_of_stream` 均不能完成该 profile。
- 仅在 COMPLETED 同事件有完整 materialized markdown 且正文非空时成功。通常为 chunks，可选 answer 必须与 chunks 一致；chunks 字段确实不存在时，也接受256KiB内的字符串 answer。显式 null/错误 chunks 不走兜底，PENDING 的 answer-only 不作为 chunk 初始化。成功沿既有 `Update::Complete` 释放 HTTP body，不等待 socket EOF。
- 一旦出现 auto 答案，不允许转入该 profile；该 profile 不混入其他 ask_text track、legacy text 或 workflow。markdown owner 继续拒绝混入 workflow；独立 workflow owner 的有限支持见下节，不通过“选最长文本”判断答案。
- 来源仅取当前事件原有直接 web_results；未知 usage/模型身份保持未知，访问 token 不输出。没有跨事件来源拼接或原生上下文缓存。

### 有限 workflow 最终文本 profile

2026-11-13：依据用户授权的两轮官网观察增加独立 `workflow.rs`，记录见 [脱敏观察合同](plan/provider-perplexity-workflow-contract.md)。首轮没有完整 wire；第二轮已观察续接字段和完整 workflow 终态。测试是结构合成，不是保存原始 token 的抓包回放。

- 只接受 `workflow_root` 的 version 1.0 完整终态：顶层 COMPLETED 且 final/text_completed/final_sse_message 均为 true，workflow 与全部 steps 均 WORKFLOW_COMPLETED，snapshots 为空。
- 全树必须恰有一个 `WORKFLOW_ITEM_TEXT`，item 与 text_payload 的 variant 均为 answer、is_streaming=false；非空 text 与字符串 chunks 精确一致。诊断 title/headline 不成为答案；不支持多答案、非文本 item、引用或混合 legacy/markdown。
- PENDING 仅检查已观察 patch 操作/path 的外壳，不应用、不保留、不输出 patch 中的内容，返回空 snapshot。PENDING/final、EOF 和 end_of_stream 均不能完成工作流。首次识别即锁定 profile，禁止前后切换。
- blocks/steps 至多128，单步 items 至多1，单事件 patches/chunks 至多4096，答案256KiB；沿用总响应、帧、事件和 deadline 预算。成功沿原 Complete 路径释放 body，不等待 socket EOF，不增加后台任务、重试或账号轮换。
- 请求仍是单轮 concise/turbo；未主动切换成真实观察中的 copilot/schematized，不保存 token 或会话 ID，不声称 Gateway 多轮续接或账号验收已完成。

## 本地安全预算与失败语义

| 项目 | 上限 |
| --- | --- |
| 请求文本 | 64 KiB UTF-8 |
| 总 deadline | 显式正值，最多 120 秒，覆盖 send 与完整 SSE 读取 |
| 总解码响应 | 4 MiB |
| 单 SSE 事件 | 512 KiB |
| JSON 数据/命名事件 | 4096 个 |
| 单答案 | 256 KiB |
| 单事件 blocks / legacy steps | 128 个 |
| 最终来源 | 64 项；每项 URL 2048、名称 1024、摘要 8192 UTF-8 字节 |

这些是 Gateway 本地预算，不是平台官方配额。HTTP client 本身可能先产生一个已解码 chunk；owner 在复制/解析前准入，不把这项检查宣称为底层解压器的总进程内存上限。

401/403、429、重定向、200 HTML/可能的 challenge、非 SSE、HTTP 失败、协议/资源错误、网络与 deadline 分别有固定错误码。403 只能证明会话/访问被拒绝，不据此断言具体 challenge 类型；200 HTML 也不猜登录页内部 schema。本 owner 返回的错误全部标记为不可 retry/Abort；自身不换账号、渠道、模型或改 body 重提。P40 公共 pipeline 已用仍有可用备用候选的合成配置验证401/429/不完整答案各只发送一次。错误不回显 Cookie、上游正文或网络 URL。

交换不 spawn 后台任务；成功终态、解析错误、超时及 future 取消均通过正常 ownership 释放 HTTP stream。测试使用新建随机回环端口和合成材料，不读 `.ng` 或真实账号，不需要 Redis。详细运行结果见[P37 回执](progress/2026-11-03-provider-channel-expansion-p37.md)。

## 正常入口与未实现项

P40 注册独立 preset `perplexity-web`、profile/special `perplexity_web`、adapter `perplexity_web_compatible` 和 leaf feature `line-perplexity-web-reverse-api`，不依赖或改指 Sonar/Search。feature 关闭时真实 transport 不进入该 binary，识别与材料/受众保护仍存在。通用 request planner、raw JSON/binary passthrough 和 compact 快捷路径仍拒绝，不删除安全 gate 后落入 generic fallback。

正常 Console 模板创建独立 provider 与空凭据池，没有默认模型、账号、权益、自动目录探测或蓝色验收状态。材料入口复用普通 apply、暂存绑定和 SecretPatch/CAS；不是另造保存协议。只有显式 model test 才调用所选账号，同一 dispatcher 不注入不支持的 `max_tokens`，不路由换号；无 request 的通用 probe 返回 Unsupported，不伪造 `/models` 或悄悄生成。

真实 auth→filter→route→send→finalize 的 Chat/Responses buffered 与聚合SSE已有P40合成loopback证据；2026-11-08 流式入口继续增加产品快照观察与迟到错误/取消的定向验证，具体通过范围见新回执。答案、来源、alias→turbo、精确credential审计和未知usage保持分离；不据此宣称标准实时 token streaming、Server/PG、原生包或全部HTTP断连时序已验。

仍须继续：

1. 完整 W08 所需能力与真实协议漂移调查。有限单轨 markdown patch 和上述 workflow 最终文本已接线；通用 JSON Patch、多轨/其他 workflow、text_completed-only、其他终态、主动 schematized 请求、多轮、Research、附件和其他模型策略仍未支持；无证据不猜 patch path/op。
2. 当前没有可用的模型目录、session refresh 或自动挑战解决流程；不把 session endpoint 的存在当生命周期已完成，不估算订阅余额。
3. P50 无凭据集成及打包、P60 集中录制核实实际 terminal data/最后快照/来源分布/Cookie/challenge、P70 精确账号与额度验收继续。当前不立即要求用户登录。

Sonar/Search 原有 preset、Key、URL 与调度逻辑未修改。R12/W08、P50/P60/P70 与总目标仍执行中。

## P38 专用材料与后端保护

`perplexityWebMaterial.ts` 已由普通 `providerSessionMaterial.ts` 精确分派，并由正常账号材料控件调用。只接受包装 JSON 的 `product="perplexity-web"` 和原会话 `userAgent`，以及以下一种材料模式：

- 显式未分片 `token`，可选 `cfClearance`；或显式 `cookieHeader`（可附与其主 Cookie 一致的 token）。这是操作者声明，**不能证明 Cookie domain/path/secure 属性或真实登录**。不接受 header-only 分片；其他 Cookie 不进入规范化输出。
- `storageState`：Playwright 形状的 `cookies` 数组，可含 `origins`。只选择精确匹配 `www.perplexity.ai`、`.www.perplexity.ai` 或 `.perplexity.ai` 的认证 Cookie；要求 `path="/"`、`secure=true`，拒绝 partitioned Cookie、重复认证身份及过期材料。裸 apex host-only Cookie 不冒充 www Cookie。只支持明确完整的 `.0` 起连续分片（最多16片），按数字顺序拼接，不 URL decode；未分片与分片不能混用。

文件128KiB、浏览器 Cookie256项、origins64项、单值及拼接后 token16KiB、UA1024个可打印ASCII字符。只有 session 和可选 `cf_clearance` 进入规范化 Cookie；localStorage、其他 Cookie、浏览器目标、刷新端点等不投影。已知浏览器/分片/clearance有效期与显式RFC3339 `expiresAt` 只取最早值；`expires=-1` 保持未知，不假造登录/额度或刷新证据。

后端固定身份为 preset `perplexity-web`、adapter `perplexity_web_compatible`、auth mode `perplexity_web_session` 和 typed special `perplexity_web`；P40 已注册 preset/profile/line 与前端 selector，模板可添加不等于真实账号可调用。

原声明→合并后的parent/credential→`call_flow::prepare`→`Session::from_payload` 均验证消费端归属。原 provider 中任一账号声明 Web 时，整池都不能混入 API/OAuth 账号；要求显式凭据池，禁止继承 provider 级共享 token、UA、expiry、session、refresh/discovery/keepalive。规范化 Cookie 只允许未分片主 Cookie 和 clearance，主值必须等于账号秘密 `api_key`；UA/Cookie 的重复大小写身份拒绝。仅固定HTTPS origin、专用adapter/auth、auto/session指纹与固定ask路径，不允许额外body/headers或通用API身份。空账号可保存，不可构造发送Session。

绝对 expiry 与 `session_auth.expiresAt` 必须一致；过期材料可存储但 `from_payload` 不准发送，不续期或自动刷新。普通 dispatcher 使用该入口，而非只检查值形状的 `from_tokens`。Cookie仍使用现有秘密槽，HeaderValue标记sensitive。前端暂存绑定另要求原始UA恰一份、原值不变且不继承provider级headers，已知expiry在普通提交阶段保持最早值。

普通 SecretPatch Keep 在完整 resolved document 上比较原产品、完整目的地/继承消费者，并额外绑定原UA和绝对expiry。删除标记、改投API、改UA或延长expiry不能保留原秘密；需要正常Secret Grant下明确replace/clear。该保护不是新secret schema，也没有修改旧存储version或补缺省字段。

材料/目标前置见 [P38回执](progress/2026-11-03-provider-channel-expansion-p38.md)，来源前置见 [P39回执](progress/2026-11-03-provider-channel-expansion-p39.md)，正常入口和实际检查结果见 [P40回执](progress/2026-11-04-provider-channel-expansion-p40.md)。快照流是后续限定增量，不是完整渠道或真实账户验收；不立即要求用户登录。

## P39 来源列表公共保真

最终 `Answer` 已可整体移交独立 canonical `sources`。Chat/Responses JSON、聚合 Chat SSE 终帧、Responses SSE terminal response 以及反向解包/聚合采用明确的 `gateway_sources` 扩展，保留 `name/url/snippet`、顺序和重复项；空时省略，不改写正文，不虚构 inline citation offsets。详细形状、预算及 native passthrough 边界见[来源合同](provider-response-sources.md)。

来源终帧冻结答案；等待 usage 期间也拒绝后续正文/工具修改，但允许同一终帧的最后 delta 与 usage-only 尾帧。反向流不再吞掉已声明来源扩展的解包失败。usage 不存在或 null 时保持未知，合法 object 的原有语义不变。P39证明来源前置，P40继续正常dispatcher及公共HTTP接线；两者都不宣称实时 token streaming。
