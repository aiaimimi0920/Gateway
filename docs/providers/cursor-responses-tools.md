# Cursor 客户端协议与工具链路

## 本阶段收尾决定（2026-11-28）

用户已接受生成 token 上限缺少可信原生映射这一限制，本阶段按下述已验支持范围收尾。
不填写虚假的默认值或极大值，也不把提示词、截断或超时声称为上游生成上限。
Anthropic Messages 与 legacy Completions 仍明确拒绝；`allProtocolsImplemented=false`。
这不是所有参数兼容、官方独立推理授权、实际底层模型或请求级 Free 扣额的验收结论。
源码验收、提交推送、新包生成与 4200 切换分别记账，以下历史包不自动代表最终部署版本。

## 能力与证据（2026-11-27）

| 验收对象 | 结论 | 证据与限制 |
| --- | --- | --- |
| `127.0.0.1:4200` 管理台 | **实际通过：账号管理** | 已保存账号的调度开关与列表均为启用；账号及池的通用探测返回 `unsupported`，未知账号返回 404。Edge 操作没有发出推理请求，不是上游失败，也无需重新登录。 |
| Cursor 官方 Cloud Agents | **公开的 Agent 接口；本轮未调用** | 可创建 Agent、发送后续任务、读取 Run 事件流；`GET /v1/models` 的模型 ID 用于 Agent。工作区、工具和会话由 Agent 管理，不是 Gateway 可以直接转发的 Chat／Responses 模型接口。 |
| Gateway Chat／Responses JSON／SSE 与 Responses WebSocket | **真实通过：2026-11-27 19:36–19:41** | 原生 HTTP／WS 客户端经 4200 到指定 Cursor 账号；13 场景、25 个成功阶段，含 PNG／JPEG 图片和原生 thinking。SSE 是阶段缓冲输出，不是 Cursor 实时 token 流。具体范围见下方协议扩展验收。 |
| Gemini GenerateContent／Cohere Chat v2 子集 | **真实通过：2026-11-27，隔离 4317** | 新 API devtest 的文本 JSON／SSE、各两轮 function 往返、NONE、Gemini PNG／JPEG；共 20 个成功阶段、8 次工具调用。一次 TLS 前置失败保留，诊断后新任务补齐；不是 4200 已部署或全部参数兼容。 |
| Bedrock Converse／DashScope text／multimodal | **真实通过：2026-11-27 22:55–23:01，隔离 4317** | 11 个文本／PNG／JPEG 正例、9 次控制参数零发送拒绝；JSON、AWS EventStream／SSE 的真实 token 用量与指定账号审计一致。仅计量子集，无工具和生成控制参数，尚未部署到 4200。 |
| Anthropic／legacy Completions | **仍未实现** | 生成 token 上限缺少可信的原生映射；JSON／流式零发送拒绝已实测，不是成功推理。 |
| function／custom text 工具调用、结果回传与多轮 | **真实通过：同批协议扩展验收** | 12 次工具调用（一次故意暂停后断连），HTTP 完整历史与 WS 同连接增量／完整历史交替续轮通过。不是持久化对话，不支持任意新用户消息增量或断线恢复；不包含 grammar／strict 工具。 |
| 固定 Codex HTTP Responses 编程链路 | **真实通过：2026-11-27 18:32** | `codex-cli 0.155.1 → 4200 Gateway → cursor-remote`，三次 `exec_command` 顺序读→改→测，收到工具结果后最终回复；四阶段 exact account 审计 HTTP 200，原自动化与独立只读复核均通过。不扩大为所有工具／协议或 Free 扣额通过。 |
| 常见错误与回收 | **分层通过，非穷尽验收** | 真实实例的 17 个 HTTP 请求及 2 个 WS 旧／错 ID 请求明确拒绝、零次上游发送；WS 断连后新任务成功。错 Key、篡改历史、HTTP body 未消费回收等仍只有本地合成证据；未证明远端关闭的精确时刻。 |
| 当前账号的官方独立推理与 Free 扣额 | **未证明** | 下方非官方实验的文本和工具实测不证明官方接口或请求级 Free 权益；不以付费或本机 Cursor 替代。 |

### Codex 工具续轮兼容修复（2026-11-27）

17:34（北京时间）用户换代理节点后，三个固定主机的证书验证和 `h2` 均通过，
4200 上一次真实文本请求 HTTP 200；原自动化管理审计读回失败，已用只读 SQLite
按 exact audit/Key 独立补核，未覆盖原失败回执或重发请求。

17:48–17:49，固定 `codex-cli 0.155.1 → Gateway → cursor-remote` 的编程验收
`cursor-coding-87b9e648-7e9e-46e0-8208-e7e1e0900719.json` **失败**：
首次上游成功，Codex 沙箱实际读到两个文件；续轮在 Gateway 准入报
`cursor_remote_unsupported_tool_request`，零次上游发送，未修改文件，临时 Key 已撤销。

真实 CLI 离线回显证实 `reasoning` 保留 ID／正文／空 summary，但省略 `status`，
并附带 `encrypted_content:null`。修补仅在 Responses 接受这两个无语义差异，
显式错误状态、非 null 密文、篡改正文／ID／顺序仍拒绝，Chat 不放宽。
相同摘要标记和活 Run 绑定保持不变，无效输入不消费暂停槽。
修补后的 87 项 Cursor Rust 定向测试、11 项 Node 验收测试及真实 CLI 离线两轮回显通过；
这些源码验证与下述新包真实上游验收分开记账。

18:31（北京时间）按用户明确允许有在途请求时切换的授权，旧后端优雅退出 0，
离线私有备份经全文件哈希与 SQLite `quick_check` 验证后启动 desktop devtest 包
`gateway-devtest-cursor-tool-echo-20261127-1805`。双 EXE 哈希匹配，沿用原数据根、
路由修订 `r599-e8235e514f81` 和账号配置；LAN relay 未动，健康与管理页面均为 200。

18:31–18:32，真实固定 CLI 在 Windows workspace-write 沙箱中完成三个独立工具：
读取 `calc.mjs`／测试，只修源码，运行 `node calc.test.mjs`，随后收到非空最终回复。
沙箱及独立可信副本均为三条测试通过；源码精确匹配，测试文件未改。
四阶段审计均命中同一指定 Cursor 凭据、`cursor_remote`、HTTP 200、attempt 1，
交付未知标记为 false；无重连／重试／fallback，临时 Key 已撤销，持久配置不变。
未运行本机 Cursor，也未验证 custom grammar `apply_patch`、实际底层模型或请求级 Free 扣额。

原自动化 `passed=true`：
`target/provider-evidence/cursor-coding-1792a7d4-b47e-44db-ba0a-28c04df3e59a.json`。
独立只读 SQLite 复核 `independentPassed=true`，未补发模型请求：
`target/deploy-evidence/gateway-devtest-cursor-tool-echo-20261127-1805/coding-reconciled-1792a7d4-b47e-44db-ba0a-28c04df3e59a.safe.json`。
同目录 `deploy-result.safe.json` 保存切换与备份证据；此前失败回执均保留。

### 固定 Codex 扩展实测（2026-11-27 18:43–18:45）

沿用上述 4200 desktop devtest，不改 Gateway 产品代码、不重新编译或切换实例。
四个独立工作区／最小权限 Key 串行走真实 `codex-cli 0.155.1 → Gateway → cursor-remote`：

| 场景 | 工具调用／请求阶段 | 实际结果 |
| --- | --- | --- |
| 输出边界与命令失败 | 4／5 | 中文、emoji、引号／反斜杠／转义空白及约 4 KiB 输出匹配；空输出成功；退出 7 的预期失败返回后继续下一工具；最终 JSON 准确引用前后随机标记。 |
| 六轮工具续接 | 6／7 | 六次独立调用逐一返回随机 token，最终 JSON 按原序完整复述，证明跨轮结果未丢失。 |
| 双文件修改与失败测试回传 | 5／6 | 读三文件、原始六条断言全部失败、分别修改两个源码、六条全部通过；测试文件未变，源码精确匹配，独立可信副本也 6/6 通过。 |
| 后续独立纯文本任务 | 0／1 | 前序 Run 结束后新 Key／新工作区返回精确随机 JSON，无工具调用；不等于通用历史会话恢复。 |

合计 15 次工具调用、19 个完成阶段；全部 HTTP 200、指定同一凭据、
`routeAttemptCount=1`、`cursorRemoteDeliveryUnknown=false`，无重连／自动重试／fallback。
四个 Key 均撤销，账号／凭据文件摘要／路由修订及配置未变，原进程继续服务。
原验收与独立只读数据库复核均 `passed=true`：
`target/provider-evidence/cursor-scenarios-3e707e68-499f-428d-9b6d-9629b16708db.json`
及同名前缀的 `-reconciled.safe.json`。

首轮 `cursor-scenarios-37b77904-63d7-465a-9ab4-73f40e56cb8f.json` 保留 **failed**：
Windows PowerShell 默认将 native exit 7 映射成 1，夹具误以为能直接观察 7；
本地对照证实显式 `exit $LASTEXITCODE` 可保真，修正提示后用新任务验证。
该首轮五阶段实际均成功，但管理审计 GET 另有 10 秒超时；未修复或掩盖此管理查询问题。
后续显式选择只读 SQLite 审计，不重放请求补证，也不把旧红回执改绿。

复跑入口为 `tools/verify-cursor-scenarios.mjs --allow-remote-matrix`，沿用编程验收的
`--base/--provider/--credential/--codex` 与管理环境变量；
可显式传 `--audit-database <本地 runtime.sqlite3 绝对路径>` 选择只读审计。
无失败自动重跑，任何失败均停止下一场景。七项新增夹具／证据反例测试、语法检查、
UTF-8 无 BOM／空白及权威有效行数门禁通过；原十一项邻域 Node 测试也已通过。
这批 Codex 验收不包含 grammar `apply_patch`、图片、WebSocket、并发／取消、任意历史恢复或 Free 扣额；图片和 WebSocket 的后续原生客户端验收单列如下。

### 原生协议扩展验收（2026-11-27 19:36–19:46）

沿用同一 4200 实例与路由修订 `r599-e8235e514f81`，本轮是
**原生 HTTP／WebSocket 客户端 → Gateway → cursor-remote**，不是 Codex CLI 调用 Chat／WS。
指定同一 Cursor 凭据，未运行本机 Cursor、修改产品源码、重新编译或切换实例。

| 场景 | 数量 | 真实结果 |
| --- | --- | --- |
| Responses JSON／SSE、Chat JSON／SSE 图片输入 | 4 | PNG／JPEG 的随机八位码仅在图片中，四次最终 JSON 均准确读出。 |
| Responses／Chat 新任务 `tool_choice:"none"` | 2 | 声明工具但不调用，最终随机文本结果精确匹配。 |
| Responses JSON／SSE、Chat JSON／SSE 工具往返 | 4 | function → custom text → 最终 JSON；回传字符串与文本 parts 数组，中文、emoji、`&quot;` 字面量匹配，完整历史回显 thinking。 |
| WebSocket 文本 | 1 | 串行 `response.create` 返回精确随机 JSON。 |
| WebSocket 含图工具续轮 | 1 | function 增量 → custom 完整历史 → function 增量，在同一活 Run 完成；完整历史保留原图。错 ID 被拒后合法续轮仍成功。 |
| WebSocket 工具暂停后断连 | 1 | 新连接不能认领旧 ID，随后独立文本任务成功；不等于断线续传，亦不证明远端关闭的精确时刻。 |

合计 **13 个正向场景、25 个成功阶段、12 次工具调用**，其中一次工具暂停有意不回传结果。
25 阶段均有非空原生 thinking，均为指定账号、HTTP 200、attempt 1、
`cursorRemoteDeliveryUnknown=false`；无自动重试／重连／fallback。
此处 thinking 是远端可见内容的映射，不表示支持任意 reasoning 参数或密文。

另有 **17 个 HTTP 负向请求**：Responses 8 个、Chat 9 个，覆盖 store、HTTP previous ID、
strict／grammar／required 工具、reasoning 控制、结构化输出，以及 Responses compact、
Chat `n:2`／`max_tokens`。全部 HTTP 400，错误码符合契约。连同上述 2 个 WS 拒绝，
共 **19 个预期拒绝**；审计均为 `routeAttemptCount=0`、`upstreamStatus:null`、
`cursorRemoteDeliveryUnknown:null`（显式 null，不是 false）。
负向审计按每个临时 Key 的确切审计集合和错误码多重集复核，不声称重复错误码逐请求一一绑定。

两组原回执及同名前缀 `-reconciled.safe.json` 的独立只读 SQLite 复核均通过：

- 正向：`target/provider-evidence/cursor-protocols-4626599d-dc08-42b4-9e94-7f4880ee3b45.json`
- HTTP 负向：`target/provider-evidence/cursor-protocols-91a5996b-7eb3-4b53-88ab-68c4b84c5595.json`

15 个临时 Key 均已撤销；配置／路由、凭据摘要、关键文件及账号投影前后不变，未重放请求补证。
本地新增 Node 验收器测试 11/11，已有同源 Rust 测试二进制的 Cursor 定向回归 87/87；
后者的错 Key／跨协议／篡改／HTTP body 取消等合成测试不冒充真实上游验收。
验收器未穷尽孤立／越界流事件、Chat 分片身份等畸形输出分支，不据此宣称全协议等价兼容。

此前失败回执保留原判定（均位于上述 `target/provider-evidence/`，前缀为 `cursor-protocols-`）：

- `f3d88211-8420-4375-9a94-215e900e59ad`、`c8d03517-66a0-4338-ba06-cd0595778e1a`：
  分别为夹具误限定 HTTP ID 前缀、按 UTF-16 索引切断 emoji；已修夹具并补本地测试，未放宽产品校验。
- `c9a93ae2-be2a-40f7-81d2-7fa962c0f74a`：真实 custom 输入未精确复述工具结果，仍记失败。
  原因未确定；后续仅明确提示将结果视为 opaque、禁止 HTML decode，未放宽精确匹配断言。
  新任务通过不代表已修产品缺陷或模型始终保真。
- `6c8768e2-ddaa-4aa5-9e9f-62dcf2ba2b20`：上个失败 HTTP Run 的工具暂停占槽，返回
  `cursor_remote_busy`；撤销 Key 不立即取消该暂停，300 秒 TTL 后自然恢复，未重启实例。
- `22ff8943-0163-4c8e-935c-0f850ccfe70d`、`ee726db5-4a4c-418a-9d81-e7f2f66b42a6`：
  分别为 required-tool 错误码预期不符、compact 夹具误带 stream 字段；按现有入口契约纠正。

复跑入口：`node tools/verify-cursor-protocols.mjs --allow-remote-protocols
--base http://127.0.0.1:4200 --provider <id> --credential <id> --audit-database <本机数据库绝对路径>`；
管理 token 仅经 `CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN` 传入，可用 `--cases` 选择场景。
失败即停止，不自动重放；独立复核用 `tools/cursor_remote/reconcile_protocols.py`，
只读数据库且不发推理。成功回执不覆盖旧失败。Anthropic Messages 等未实现项仍见下表，
`actualModel` 与请求级 Free 扣额仍未知，因此不能标记服务商“全协议开发完成”。

### Gemini／Cohere 原生子集与其余入口边界（2026-11-27）

本批新增 `gemini_request.rs`／`gemini_tools.rs`、`cohere_request.rs`、
`native_output.rs`／`native_publication.rs` 及 Cursor 专用路由接线。
原始请求先按自身协议严格校验，typed 结果直接输出自身 JSON／SSE，
不经有损 OpenAI 响应往返；普通供应商保持原鉴权及转换路径。

- Gemini：文本、camelCase `systemInstruction`、末条 user 的 PNG／JPEG `inlineData`；
  function 声明接受有界 OpenAPI `parameters` 或独立 `parametersJsonSchema`（互斥）。
  前者只在 schema 节点转换精确大写类型；`propertyOrdering` 明确拒绝。
  工具调用须回显实际 ID／name／args 和完整历史，结果必须为对象。
  只在此协议对对象参数做递归稳定序列化，未放宽 Responses／Chat 的字节契约。
- Cohere：文本与 function 工具；省略 `tool_choice` 为自动，显式只支持 `"NONE"`，
  不接受 `"AUTO"`／`"REQUIRED"`。字符串工具结果须为 JSON 对象字符串且保留原字节，
  任意纯文本结果可用 `text` parts；不接受图片、custom、`tool_plan`。
- 新协议仍为同一活 Run 的串行工具续轮，绑定 Local Key、账号、修订及原始协议。
  Local Key 独立 scope 分别为 `gemini.generateContent`／`cohere.chat`；
  即使授权 `relay`，也不能跨协议认领暂停槽。
- Gemini SSE 每阶段一个完整 `data` 对象；Cohere 输出原生 message／content／tool-call
  start／delta／end 及 `TOOL_CALL`／`COMPLETE`。均非实时 token 流、无 `[DONE]`。
  完整有界 body 与终态验证后才发布工具暂停；未消费 body 的回收有本地测试。
  两者不发布 Cursor thinking，不伪造 usage、模型身份、thoughtSignature 或 tool_plan。

**新包与真实运行：** API devtest
`../release/Gateway/gateway-devtest-cursor-native-protocols-20261127-2045`，
`gateway.exe` SHA-256 为
`1800843a4a8692d88a7c252182ac1c33863963599ff997f868103e27746bd401`。
构建凭据 `builtAt=2026-11-27T12:56:46.7912348+00:00`，来源指纹与完整包校验通过。
21:00–21:23（北京时间）仅在 `127.0.0.1:4317`、私有隔离数据根运行，
路由修订 `r0-68a0512bca5c`，仅复制原已授权 Cursor provider／指定凭据。
这是 **原生 HTTP → Gateway → Cursor**，不是 Codex CLI 调用 Gemini／Cohere；
未运行本机 Cursor、换账号、启用付费 fallback 或重放旧 Run。

14 个 Gemini／Cohere 选定场景累计完成 **20 个成功阶段、8 次工具调用、
16 个预期零发送拒绝**。两次工具结果含中文、emoji、HTML 实体字面量及随机 receipt，
第二次调用参数精确取第一次结果；Gemini PNG／JPEG 随机码只存在于图片。
所有成功阶段均为指定凭据、HTTP 200、attempt 1、交付未知标记 false，
并按原始协议 family 独立核对 wire 与只读 SQLite 审计。

- `cursor-protocols-0cb7af45-6a46-40b3-95e5-d44519b90473.json` 首批整体仍为 **failed**：
  前九场景、17 阶段通过；JPEG／SSE 在发送前报 `cursor_remote_verified_http2_required`。
  审计为 attempt 0、无上游状态、交付未知 false。无凭据 Rustls 诊断随后确认
  三个固定主机均证书验证及 `h2` 通过，未改代理或关闭验证。
- `cursor-protocols-42f26121-8df9-492c-8593-535f168e0323.json`：
  新 Key／新任务完成 JPEG／SSE 和此前未执行的 NONE／拒绝场景，五项全部通过。
- `cursor-protocols-666d2ca3-25d4-434e-ba4c-80cbf000e8c1.json`：
  Anthropic、legacy、Bedrock、DashScope text／multimodal 的 JSON／流式各一次，
  共 10 次 `cursor_remote_unsupported_endpoint`，HTTP 400、零发送、交付标记 null。
  该负向夹具首轮 `6c7dd383-7408-48a7-9e67-6075ed960b85` 因混淆 canonical enum 与
  审计 family 名称判失败；按真实 `protocol_family_name` 契约修夹具并补测试，
  未改产品准入、断言强度或旧回执。

以上回执位于 `target/provider-evidence/`；有效场景均经同名前缀
`-reconciled.safe.json` 独立只读复核，不重发推理。
汇总为 `target/deploy-evidence/gateway-devtest-cursor-native-protocols-20261127-2045/acceptance-summary.safe.json`：
19 个场景覆盖通过，26 次预期拒绝，**`allProtocolsImplemented=false`**。
21 个临时 Key（含失败批次）全部撤销；隔离实例 drain 后退出 0、4317 已关闭。
现网 4200 原 backend／UI／LAN relay 未改，路由、凭据摘要、账号投影及关键配置前后不变；
没有用缺少 UI 的 API 包替换 desktop 实例，没有提交或推送。

**该批验收结束时的缺口：** 当时原生执行器没有可信的生成 token 上限及请求级 token 用量映射。
Anthropic Messages 的必填 `max_tokens` 与 usage 不能忽略或补零；
legacy Completions 省略上限仍有默认 16，不能解释成无限制。
Bedrock usage／metadata 尚无可靠来源，不能用普通 SSE 代 AWS EventStream；
DashScope 的原生输出／usage 尚未完成可省字段与用量语义映射。
因此本轮只完成上述可忠实映射子集，未将这些缺口伪装为成功。
本日再次核对的契约依据：Anthropic 官方 SDK `messages.create`／`types.usage`、
OpenAI `openai-openapi` 的 `CreateCompletionRequest`、AWS `ConverseStreamMetadataEvent`。
后续须先补齐真实上游能力来源，不能以提示词、字符截断或虚构计数通过验收。

**本地验证：** Rust Cursor 定向 108/108（Nextest
`8a18229b-eaad-4f5b-b86a-dd4ec6c684a7`），同源 EXE 的 auth／Cohere／Gemini 邻域
分别 9/9、11/11、7/7；Node 验收器 16/16，新增负向 family 断言修正后 2/2。
定向 rustfmt、UTF-8 无 BOM、scoped diffcheck 与权威有效行数 ratchet 通过。
全仓 `cargo fmt --all --check` 仍因非本批的 Luma registry、smart routing、
model catalog、network probe 与 queue 文件未通过，未顺带改写他人代码。
包生成后只追加外部验收脚本／回执／文档，不修改已验 EXE 或包内清单；这些后补脚本不在该包内。

### Bedrock／DashScope 计量子集（2026-11-27）

固定第一方 SDK 1.4.2 归档的 SHA-256 为
`38baf2ec3d11c5567b4b885f02146e89b697a4656d462b1c8731e6caba2a12a5`。
描述符确认 `AgentRunRequest.emit_model_call_usage #37`、
`InteractionUpdate.model_call_completed #31` 和 `TurnEndedUpdate #1..#5` token 计数。
源码现请求该事件，在完整成功的 TurnEnded + EndStream 后优先采用终态 input/output；
终态完全缺计数时才汇总去重的 model-call usage，部分 optional 计数保持未知。
负数、溢出、重复冲突和超过 128 个 model-call ID 均拒绝，不以本地估算补齐。
SDK 的 total 为 input + output，缓存计数独立保留；不臆造 OpenAI 缓存包含关系。

- Bedrock：文本、system、末条 user 的 PNG／JPEG；JSON 或带 CRC32 的 AWS EventStream，
  含完整 usage／metadata。`metrics.latencyMs` 为 Gateway 阶段耗时，不是 AWS 服务测量。
- DashScope text：`input.prompt` 或文本 messages；默认 text、显式 message 输出；
  multimodal：文本和 PNG／JPEG data URL，输出单一文本内容块。JSON 与 SSE 都包含真实计数。
- 流式仍是一个有界终态阶段，不声称实时 token 流。三个独立入口 scope 为
  `bedrock.converse`、`dashscope.text`、`dashscope.multimodal`。
- 不支持工具、生成 token 上限、采样和未知扩展；Bedrock guardrail 请求头明确拒绝，
  不能静默忽略。缺 usage 返回 `cursor_remote_token_usage_unavailable`，不伪造成功。
- 工具 Run 的 model-call 与客户端阶段缺少可靠对应关系，任一工具交接后不发布整 Run 用量，
  防止将总量重复计入最终阶段；已有工具协议行为不借此扩张。

验收器按请求严格核对输出格式，对固定非空输入／回复要求正数计量，并强制与指定账号的
独立只读 SQLite 审计一致；usage 缺失不能跳过检查。最终 Node 21/21、Python 2/2 通过。
另修正 Cursor 非原生 JSON 的缓存误投影：只移除 Chat／Responses 的缓存详情，
保留真实 input／output／total 及内部原生缓存遥测；未知缓存也不伪造 `cached_tokens:0`。
新增真实 router＋合成 worker 的 12 请求回归，覆盖两协议、JSON／SSE、三种用量状态，
并核对审计保留原生缓存。最终 Rust Cursor 与 Bedrock／DashScope／Cohere／Gemini／认证邻域
379/379 通过（Nextest `5dd1e234-2545-47c8-bcdf-23917420100c`）。
首轮新协议投影标记遗漏已修；后续一处测试误等鉴权前拒绝的阶段审计，已按实际数据库
及入口生命周期纠正，并加强 HTTP 403、六次成功计量和单次发送断言。
定向 rustfmt、diffcheck、权威有效行数门禁通过；全仓 fmt 仍被非本批文件阻塞，未改写他人源码。
**真实上游验收：** API devtest `gateway-devtest-cursor-metered-20261127-2225`，
EXE SHA-256 `eea30147ec6e03268feb1e395ca1503a4fcc92db9cf4197d7259597e00ebffbe`，
在显式隔离数据根和 `127.0.0.1:4317` 运行，路由修订 `r0-68a0512bca5c`。
22:55–23:01（北京时间）原生 HTTP 客户端完成 Bedrock 四项（文本／PNG／JPEG、JSON／EventStream）、
DashScope text 三项（默认 text JSON／SSE、显式 message JSON）及 multimodal 四项
（文本／PNG／JPEG、JSON／SSE），合计 11 个成功阶段；另三类共 9 个控制参数请求零发送拒绝。
每个成功阶段均命中同一指定 Cursor 凭据、HTTP 200、attempt 1、交付未知 false，
正数 input／output／total 与独立只读 SQLite 审计逐项一致。首个 Bedrock 文本实际为
input=11295、output=171、total=11466；图片随机码只存在于图片，返回精确匹配。
未运行本机 Cursor、重试、换账号或启用付费 fallback；不是 Codex CLI 调用这些原生协议。

原回执与同名前缀的 `-reconciled.safe.json` 均通过：

- `target/provider-evidence/cursor-protocols-55beef70-d5e9-4463-8a89-e3ba837c1138.json`（单例 canary）
- `target/provider-evidence/cursor-protocols-1726fbcb-1d19-4d72-ad85-5b9eed99e44b.json`（剩余 13 项）

14 个临时 Key 均已撤销，隔离路由／凭据摘要／账号及关键配置前后不变。
真实回执仅证实三项总量，不证明非零缓存计数或其与 input 的包含关系。
上述已验包早于随后发现的 OpenAI JSON 缓存修补，不能作为该修补的 EXE 验收；
Bedrock／DashScope 代码未受此修补影响。

**缓存修补配套包：** `../release/Gateway/gateway-devtest-cursor-metered-cache-20261127-2305`
为 API devtest，EXE SHA-256
`3d97a1a7253c6f11cfbc0c7cb99d2a70f25dda9a5057f463fd579635fd7be0ac`。
增量编译实测 2m21s；构建前后源码指纹、包校验及 artifact smoke 通过。
23:09–23:10 同一隔离端口真实完成 Responses JSON、Chat JSON（均 `tool_choice:"none"`）
及 Bedrock JSON 三个新任务；验收器明确拒绝两类 OpenAI 缓存详情，三个响应均通过。
回执 `cursor-protocols-71c12f29-f34f-440e-a5fe-6c1d4b1d2cdf.json` 与独立对账通过。
23:11 在同包再次核对 Anthropic／legacy 的 JSON／流式四次零发送拒绝，
回执 `cursor-protocols-4fb1f8fe-a00f-4213-bc43-8bad5c20086c.json` 及独立对账通过；
这四次拒绝仍不是协议支持。

上述两包的隔离实例均 drain 后退出 0，4317 已关闭；共 19 个临时 Key 全部撤销。
4200 后端、桌面壳及 LAN relay 未切换，路由、账号、凭据摘要及关键配置前后不变。
两组汇总分别在 `target/deploy-evidence/<包名>/acceptance-summary.safe.json`，
均显式记录 `allProtocolsImplemented=false`。未提交或推送。
最终包生成后仅追加本段验收文档与外部回执，未改已验产品源码、EXE 或包内清单。
Anthropic 必填 `max_tokens` 与 legacy 默认生成上限仍无可信映射，不能以提示词、
字符截断或客户端超时替代，因此不能声明服务商全协议完成。

### 新版 4200 实验文本验证与连接阻塞

2026-11-27 16:18（北京时间）独立读回确认，用户已启动
`gateway-devtest-4200-cursor-review-20261127-1545`，双 EXE 哈希匹配，
沿用原数据目录、路由修订 `r599-e8235e514f81` 和已保存账号；健康与管理网页均为 200。
16:19 的一次 `/v1/responses` 文本测试返回 503，精确审计
`gw-audit-5e841e62-0283-4258-ae69-53bc06ca432e` 为
`cursor_remote_verified_http2_required`、零次推理发送、无未知送达；临时 Key 已独立确认撤销。
这是已授权的非官方实验路径失败，不是官方接口或 Free 扣额通过。

随后无凭据诊断复用本机已编 Rustls 依赖和生产信任配置，仅做 CONNECT、证书验证与 `h2`：
66 个原生根成功加载，`api2.cursor.sh` 与 `agentn.global.api5.cursor.sh` 通过，
`cursor.com` 在 CONNECT 200 后以 TLS `UnexpectedEof` 结束；独立 Node TLS 对照同样为
`ECONNRESET`。尚不能区分代理、出口或远端关闭，未关闭验证、修改代理或重发推理。
诊断源为 `tools/cursor_remote/tls_diagnostic.rs`；只接受显式回环代理端口，
不含账号读取、应用层 HTTP 或推理请求，总时限 60 秒。
本机证据在 `target/provider-evidence/cursor-tls-native-20261127.log`、
`cursor-tls-node-control-20261127.json` 以及
`target/deploy-evidence/gateway-devtest-4200-cursor-review-20261127-1545/cursor-acceptance-reconciled.safe.json`。
验收脚本已修复含数字错误码的过滤／截断问题；HTTP 与 CLI 均保留完整 `http2` 错误码，
不输出上游错误正文，相关六项本地回归通过。此脚本修正不需要替换 Gateway EXE。

**是否需要另备环境？** 账号导入和 Gateway 直连远端都不需要用户准备虚拟机。`cursor-local/auto` 只是拒发的保留名，不会启动本机 Agent；另行试验的 `operator-cursor-auto-fixed-probe` 仅存在于显式启用的操作员测试夹具，不是生产模型入口。该实验中的 CLI 即使在 Ask 模式仍能搜索代码、发现 MCP，因此必须限制 Agent 能访问的资源；虚拟机只是一种手段。固定题的 CLI 直跑成功和隔离 Gateway 夹具的验收结果分别见[本机 Auto 实验记录](cursor-sdk-bridge-contract.md#独立的个人本机-auto-候选2026-11-27)。通用路由仍关闭，账号不必重新导入。

官方依据：[API 总览](https://cursor.com/docs/api) 明确区分 Agent 与独立推理；[Cloud Agents 端点](https://cursor.com/docs/cloud-agent/api/endpoints) 定义 Run 与事件流；[TypeScript SDK](https://cursor.com/docs/sdk/typescript) 的 `local.customTools` 只适用于本地 Agent。[套餐对照表](https://cursor.com/pricing) 将 Agent 列在 Free 内，但未包含 Cloud Agents；不能用前者证明后者免费。本次仅查阅公开文档，未向厂商发送账号材料或业务请求。

**复核入口：**隔离 HTTP／WS 测试 **22 passed / 0 failed**（HTTP 为 router `oneshot`，WS 为真实回环连接）。完整测试名见本机 `linshi/cursor-offline-surfaces-4d6c37824ca2409d9c70a1dd8f3cb889/gateway-http-ws.log`；复用测试 EXE 的 SHA-256 为 `f1da611f58936ac10c3062510673c94120563a377e2e2ea61a0b0e3810151932`。从源码复跑：

```powershell
cargo nextest run --locked --offline --lib --no-default-features --features runtime-defaults -E 'test(protocol::cursor_remote::http_tests::)'
```

4200 的既有管理回执见本机 `linshi/gateway-devtest-4200-cursor-probe-20261127-012310/` 下的 `live-cursor-management.safe.json` 和 `edge-cursor-card.safe.json`。本次导入提示改动的 5 个前端测试文件共 **28/28** 通过，包含目录、凭据读回、导入流程与通用探测说明；`tsc --noEmit` 通过。这是源码验证，未打包或部署到 4200。下文源码矩阵及历史记录不扩大以上结论。

## 历史验收（2026-11-26，按测试对象区分）

- **运行实例**：`cursor_remote` 有一个已启用且已配置凭据的账号。`store:true`、`/v1/responses/compact` 和 Chat `n:2` 三个请求均返回 400，审计均为零次发送、无上游状态；临时 Local Key 已撤销。该实例尚未部署本次修正，Chat `n:2` 仍会误报为工具错误。
- **当前源码**：真实 Gateway 路由和 SQLite 配合合成 worker 的 22 项 HTTP／WebSocket 回归通过（包括 JSON、SSE、工具续轮、图片、thinking 和断连）。其余 Cursor remote Rust 回归 57 项、相邻 Responses 流错误回归 7 项、Cursor Node 回归 193 项通过。
- **编程客户端（独立于 4200）**：22:03（北京时间），固定 `codex-cli 0.155.1` 对回环模拟 Responses 服务发出 4 次请求，沙箱分别读源码和测试、只修改 `calc.mjs`、运行 `node calc.test.mjs`。模拟服务在每次续轮核对 `call_id` 和实际工具结果后才继续，三次均匹配，无重连。修前 3 条断言均失败；修后沙箱和独立可信副本均为 `pass 3 / fail 0`，源码字节精确匹配，测试文件未改。复现：`node tools/cursor_remote/capture_coding_workflow.mjs <codex-exe>`；本机回执：`linshi/cursor-client-workflow-fbb77cc5-056e-41f1-addb-27ec7ebf7038/result.json`。工具是客户端真实执行的，指令由夹具预设；没有请求 Gateway 或 Cursor。
- **真实上游**：本轮未验证 Cursor 模型、账号额度归属或端到端编程。个人登录不能替代厂商对第三方客户端使用私有接口的授权；不以付费或本机 Cursor 替代。参见 [Cursor 支持答复](https://forum.cursor.com/t/does-using-oh-my-pi-s-cursor-provider-or-an-openai-compatible-proxy-to-the-same-endpoints-violate-cursor-s-tos/167778/5)。以下各批历史回执保留原判定。
- **官方入口（独立于 4200）**：本机未附带凭据直连 `api.cursor.com`，`GET /v1/me` 和 `GET /v1/models` 均返回 401。这证明官方元数据接口可达，不是账号登录失败或推理结果。[官方 API 总览](https://cursor.com/docs/api) 区分 Cloud Agents 工作流与独立模型推理；Cloud Agents 的 API Key 不等同于此处导入的登录凭据。

## 范围与实现

目标：`codex-cli → Gateway /v1/responses → Cursor 原生 MCP 调用 → Codex 本地执行
→ function_call_output／custom_tool_call_output → 原 Cursor Run 继续`。Gateway 不执行用户工具，不从文本猜测工具调用，
不启动 Cursor desktop、CLI、SDK runtime、Docker 或本机 Node 子进程；远端 CONNECT+TLS `h2` 传输、Connect/Protobuf 编解码与工具续轮均在 Gateway 进程内由纯 Rust（`src/protocol/cursor_remote/upstream/`）执行。

- 请求声明 `type:function`、`strict:false`（或省略 strict）的工具；保留完整 JSON Schema。
  原生 MCP 请求转换为 `function_call`，`call_id` 与客户端输出逐项匹配。
- 新增源码支持 `type:custom` 的纯文本工具：`format` 省略或精确为 `{"type":"text"}`。
  将其映射成只有必填字符串 `input` 的 MCP 对象，回程严格剥离该对象后输出
  `custom_tool_call.input`；不修剪空白、不解码 HTML、不改写内层文本。空字符串有效。
  工具类型来自已绑定的声明，不接受 worker 自报类型；function 不按参数内容猜成 custom。
- 同一个 HTTP/2 Run 跨 HTTP Responses 请求或同一 WebSocket 的串行 create 停放，
  而非断流后新建 Run 重放。
  Chat Completions 同样支持自己的串行工具往返，不可与 Responses 互相续接。
  续轮绑定 Local Key、provider、credential、路由修订、入口 scope、工具定义、instructions 和完整历史前缀。
  WebSocket 活工具暂停另支持同连接的 `previous_response_id` + 单条工具结果；
  规范输出追加到既有前缀，不另建历史／图片缓存，不重开 Run。
  错账号、错 Key、修改历史、错工具结果类型、重复输出不能认领槽；
  每轮重新核验授权和请求预算。
- 每实例一个槽，整个 Run 最长 300 秒、最多 64 次串行工具调用。TTL 不因续轮重置；
  本地写背压、远端 EOF、HTTP 未交付、取消和关停均有回收路径。
- HTTP 发布 guard 在完整 JSON 或 SSE 终态交给 body consumer 时发布续轮；
  这是服务端交付边界，不是 TCP ACK。未消费或部分消费后丢弃 body 会取消未发布的 Run。
  Chat 工具阶段先完整收集不超过 1 MiB 的有限响应体（30 秒上限），检查对应工具终态、
  SSE `[DONE]` 与严格 EOF，再整块交给 body consumer；不会提前泄出工具成功终态。
- WebSocket 的 `response.create` 传输包络仅在 Cursor 分支投影；原始 wire 保留，
  续轮额外绑定服务端生成的连接 ID。HTTP、另一 socket 不能认领该 Run。
  严格 EOF、工具 guard 及当前等待槽有效性检查通过后才发送 `response.completed`；
  发送成功后再次检查并发布。连接断开取消当前暂停，旧 guard 不取消后续 epoch。
- `continued` 只证明工具结果交给本地传输层，不冒充远端已执行。
  每个 HTTP 阶段独立记审计，禁止自动重发、换账号／模型或付费 fallback。
- 保留本人身份、个人 Free、按量关闭、其他账源拒绝与目录／选路预检。
  `actualModel` 和请求级 Free 扣额仍未知，不制造 usage。

## 当前协议矩阵与支持边界

以下区分源码支持与真实验收，**不是全部协议完成**。Responses／Chat／WS 运行证据来自
4200 的 `gateway-devtest-cursor-tool-echo-20261127-1805`；Gemini／Cohere 来自隔离 4317
的 `gateway-devtest-cursor-native-protocols-20261127-2045`；Bedrock／DashScope 来自同一隔离端口的
`gateway-devtest-cursor-metered-20261127-2225`。均限定指定账号和已列场景；
未列分支不能由模拟测试推定通过，新协议尚未部署到 4200。

| 能力 | 当前源码状态 |
| --- | --- |
| HTTP Responses 纯文本 JSON／SSE | 已实现；固定 Codex 与原生客户端真实验收分别见上方 |
| HTTP function／custom text tools | 已实现声明、原生 MCP 转换、客户端输出和同一活 Run 串行续轮 |
| WebSocket 纯文本 | 已接入串行 `response.create` 和 Responses JSON 事件 |
| WebSocket function／custom text tools | 已接入同连接、同一活 Run 的串行工具续轮；完整历史与下述增量可交替 |
| WebSocket 活工具增量 | 新增 `previous_response_id` 精确引用当前已交付工具阶段，`input` 仅一条 function／custom text 结果 |
| 断连／交付失败 | 回收当前暂停和自有 worker；不重连、不重放 |
| Chat Completions 文本 JSON／SSE | 已接入独立原始请求校验；保留原 endpoint、鉴权和响应格式 |
| Chat Completions function／custom text | 已接入同协议、完整历史、同一活 Run 的串行往返；不支持旧版 `function_call` |
| Gemini GenerateContent JSON／SSE | 文本、PNG／JPEG、function 两轮往返与 NONE 已实现并在隔离新包真实通过；不支持生成控制／签名／远端媒体 |
| Cohere Chat v2 JSON／SSE | 文本、function 两轮往返与 NONE 已实现并在隔离新包真实通过；不支持 custom／图片／tool_plan／强制工具 |
| 工具结果纯文本数组 | Responses 接受 `input_text` parts，Chat tool content 接受 `text` parts；原序无分隔拼接 |
| 新 Run 的 `tool_choice:"none"` | 校验声明与历史后走无工具 worker，不注册 MCP、不把工具结果重写成 prompt |
| 活 Run 续轮改成 `tool_choice:"none"` | 明确拒绝；当前原生 Run 不能撤销已声明的工具 |
| 内联图片输入 | Responses `input_image`、Chat `image_url` 的 PNG／JPEG data URL 已接线；原生 HTTP 四种输出及 WS 含图工具任务真实通过 |
| 远端可见 thinking 文本 | Responses JSON／SSE／WS 的 `reasoning.content` 和 Chat JSON／SSE 的 `reasoning_content` 扩展在 25 个真实阶段均观察到；不代表 Cursor 提供独立 reasoning API |
| Anthropic Messages | 未实现；必填输出 token 上限等语义尚无可靠原生映射，不以忽略参数换取兼容 |
| Bedrock Converse／DashScope text／multimodal | 文本／PNG／JPEG 计量子集已在隔离新包真实通过；JSON／原生流编码与三项 token 总量对账通过，不支持工具或生成控制 |
| legacy Completions | 未实现；默认生成上限尚无可信原生映射，JSON／流式零发送拒绝已实测，不是成功推理 |
| `store:true`／通用 `previous_response_id` | 明确拒绝；仅同连接活工具增量是窄例外，未提供持久化 retrieve／delete／cancel 生命周期 |
| `/v1/responses/compact` | 明确拒绝 `cursor_remote_compact_unsupported`，不冒充普通推理 |
| WebSocket 新用户消息增量／多路并发／断线恢复 | 未实现；不接受 `stream_id`，不可跨 socket／HTTP 续轮 |
| grammar／严格或强制工具选择／其他媒体／reasoning 控制与密文／结构化输出 | 未映射，不能声明支持 |

以下混合工具及小写 tool_choice 规则仅适用于 Responses／Chat：两种工具可以混合声明；工具输出接受字符串或上述协议规定的纯文本数组，
最多 128 段、合计最多 65,536 UTF-8 字节，保留空字符串、中文、CRLF 和 HTML 实体原文。
有工具声明时允许省略／`"auto"`；`"none"` 仅限没有旧工具历史的新请求。
`parallel_tool_calls:true` 只是允许并行，当前始终串行产出。
SSE 在每次原生工具交接或合法 TurnEnded + EndStream 后缓冲输出，非实时 token 流。

以下明确拒绝，不静默降级：custom grammar tools（包括以 grammar 声明的 `apply_patch`）、
function `strict:true`、custom 的任何 `strict` 字段、异步工具、强制 tool_choice、
远程图片 URL、非 PNG／JPEG 内联图片、音视频、reasoning effort／summary 控制、`store:true`、非上述活工具引用的 `previous_response_id`，
以及已结束 Run 后带旧工具历史的新用户任务。此版本不是完整 Responses API 等价实现。
初始 instructions／角色历史是带角色标签的 transcript，不声称 Cursor 原生角色优先级。
Chat 的 `max_tokens`／`max_completion_tokens`、采样、非文本 `response_format`、
`n>1`、usage 请求及未映射扩展均拒绝，不虚构 token 使用量或输出限制。
Local Key 必须授权真实入口的 `chat.completions`／`responses`／`gemini.generateContent`／`cohere.chat`／
`bedrock.converse`／`dashscope.text`／`dashscope.multimodal`（或显式 `relay`），
不能借用另一个入口的 scope；工具续轮即使用 `relay` 也不能跨协议认领。

## 验证

2026-11-26 源码验证：

- Node 协议／生命周期定向原有 173 项通过；补充后 MCP identity/state 测试文件 8/8，
  编程证据测试文件 4/4。后两项为相关测试文件结果，不与原计数重复累计。
- Rust Nextest `48c1a770-7c7d-4b94-bc7a-e87da77009dc`：27/27；
  覆盖真实 Gateway router + SQLite + 合成 worker 的 JSON/SSE 续轮、错 Key、重复输出及 body 取消。
- 相邻直调测试二进制：90 passed、1 个 Windows SQLite 测试目录清理失败、8 个历史 Docker 用例 ignored；
  该清理失败用例独立重跑通过，未修改产品或放宽断言。
- `cargo fmt --all --check`、权威有效行数 ratchet、scoped diffcheck 通过。
- 单独原生 MCP 远端探针已有工具结果 nonce 匹配证据；它不是 Gateway/Codex 编程验收。

真实编程验收入口（需同时具备适用的厂商接口授权与操作者显式许可，不加入 CI）：
`tools/verify-cursor-coding.mjs --allow-development-run --base http://127.0.0.1:<port>
--provider <id> --credential <id> --codex <固定 0.155.1 exe 绝对路径>`。
管理材料只从 `CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN` 环境读入。

验收创建五分钟最小权限 Local Key，并在 `linshi/cursor-development-<uuid>` 内让真实 Codex
分别读文件、只修改 `calc.mjs`、运行三条测试。要求客户端成功事件按读→改→测顺序出现，
测试文件不变、源码精确匹配可信结果、全部审计命中指定账号且单次发送，无重连或 fallback，
最终撤销 Key 并独立读回。父进程只在另一个临时目录执行可信常量，不执行模型可写路径。
回执仅保留事件分类、哈希、退出码和 exact audit，不保存任意命令／正文／凭据。

本段源码验证不代表新 EXE 已部署或真实编程已验收；实际结果须以带时间的运行回执为准。

## 首轮真实验收与修补

2026-11-26 14:25–14:26（北京时间），API devtest 包
`gateway-devtest-cursor-responses-20261126-1420` 在原隔离数据根、4315 端口启动；
4200 未切换。真实固定 Codex 顺序执行三次工具调用，四轮 Responses 均为指定账号、
HTTP 200、单次发送，无重连；源码精确正确、测试不变、独立三条测试通过。
但完整验收判定 **failed**，回执
`target/provider-evidence/cursor-coding-e7407f7a-3185-4e4a-9c3e-7df0c745ed04.json` 原样保留。

- 客户端 `node --test` 失败：无上游的同一 Codex 沙箱探针复现 `spawn EPERM`；
  直接 `node calc.test.mjs` 运行同一 `node:test` 文件通过，不派生测试子进程，不关闭操作系统沙箱。
  长期复现入口 `tools/cursor_remote/capture_coding_test.mjs <codex-exe> [--same-process]`。
  新验收仍要求 TAP 三项成功、独立可信副本测试、原源码／测试哈希检查，不以 CLI exit 0 判绿。
- SSE 审计遗漏：直接 ctx 路径有 `routeRevision`、`cursorRemoteDeliveryUnknown`，
  收尾快照未保存。修补快照复制冻结修订与三态交付标记，不从当前路由补历史；
  新增三态和真实 router JSON/SSE 持久审计断言。验收现要求交付未知标记明确为 false。

以上是本轮独立任务的诊断与修补，不自动重放已结束 Run 或修改旧审计。

## 审计修补回归与出包停点

2026-11-26 14:35–14:53（北京时间）：

- Rust Nextest `58865bb9-b69d-4438-8c55-3f6baede929b`：46/46，
  覆盖 `cursor_remote` 与 `pipeline::stage_finalize`。独立只读复审未发现阻断。
- 本批文件定向 rustfmt、scoped diffcheck、UTF-8 无 BOM 和权威有效行数 ratchet 通过。
  当前全仓 `cargo fmt --all --check` 未通过：另一路测试整理中的
  `bedrock_and_cloud_contracts`、`runtime_and_storage`、`web_and_media_contracts`
  目录缺少所引用的 `support.rs`；本轮未修改这些目录。
- API devtest 编译完成，耗时 14m32s；来源门禁随后报告
  `Source changed during build; no devtest receipt published.`。编译窗口内，路由、
  Responses、Gemini 与传输层等其他工作范围仍在改动，不能把此 EXE 当作已冻结的新包。
  计划目录 `gateway-devtest-cursor-responses-20261126-1440` **未发布**，未绕过来源检查。
- 4315 仍运行首轮 `gateway-devtest-cursor-responses-20261126-1420`，
  4200 仍运行 `gateway-devtest-cursor-runtime-20261126-125000`，两者未切换。
  修补后的第二轮真实编程验收尚未运行；首轮 failed 回执仍是原始事实。

后续按用户最新要求仅继续源码修改，由用户统一编译与回归；本轮不要求其他会话停写，
不出包、不切换 4315／4200。将来若再次执行真实验收，仍需新任务、新 Key、新 Run，
并先通过构建来源与包完整性门禁。

## custom text 源码增量（2026-11-26，未运行验收）

- 工具声明、MCP 字符串映射及精确历史校验分别归属 `tool_schema.rs`、
  `tool_request.rs` 与 `runtime/tools.rs`；不改 Node IPC 或共享 Responses translator。
- Cursor 专用缓冲 SSE 适配器保留 custom 原始文本，并输出相应 input delta／done 事件；
  空 input 不伪造非空 delta，但仍需 item added、input done、item done 和 response completed。
- 修复显式非法输入 `type` 被当作普通消息的问题：只有字段缺省才能默认 `message`，
  `null`、数字、布尔、数组、对象及未知类型均拒绝。
- 已补声明与剥壳校验、文本保真、空文本、混合串行调用、错类型不消费续轮槽、
  JSON/SSE 生命周期及重复输出拒绝的回归源码。fixture 是合成 worker，不是 Cursor 实际响应。
- 本批仅做定向格式化、静态调用点复审、UTF-8 无 BOM／空白及有效行数检查；
  **未编译、未执行新增测试、未生成 EXE、未运行真实上游验收**。
  上文 46/46 结果仅属于更早的审计修补，不覆盖本批 custom 代码。

## WebSocket 源码增量（2026-11-26，未运行验收）

- `responses/cursor_websocket.rs` 管理 Cursor 传输投影、连接绑定和 publication；
  `responses/websocket_delivery.rs` 管理有界逐帧交付，其他服务商仍走原有转发语义。
  每次 create 重新进入现有鉴权、精确账号、修订、预算及审计流程。
- Cursor 终帧暂存至严格 EOF；终态后的多余事件、非法 UTF-8、截断、
  缺失或已失效的工具 guard 都不能发送成功终态。
  收尾时不消费下一轮客户端帧；不支持并发 create，不缓存并发工作。
- 有效性检查与网络发送无法原子化：发送期间恰逢 TTL／worker 失效时，
  终帧可能已交给 socket，但发布检查失败后会关闭连接，不开放续轮、不重放。
  发布成功也不延长原 Run 的 300 秒 TTL，不等于永久可续接或客户端已确认收包。
- 补充真实 loopback WS／Gateway router／SQLite 加合成 worker 的回归源码：
  文本连续轮、function/custom 工具续轮、原始文本保真、跨 socket／HTTP 认领拒绝、
  重复提交拒绝、断连释放槽位、存储与 compact 拒绝。
  独立生产 relay 夹具覆盖坏后缀／缺失或失效 guard 时客户端看不到 `response.completed`，
  并检查等待槽回收；runtime 夹具覆盖旧 epoch 不误取消后续暂停。
- 本批仅定向格式化和静态复审，不编译、不运行上述测试、不生成 EXE，
  不更改 4200／4315 运行实例。当前完整编程验收、实际执行模型和请求级 Free 归属仍未证明。
- 静态收口：三路只读复审未发现阻断；定向 rustfmt、scoped diffcheck、
  UTF-8 无 BOM／尾随空白检查与权威有效行数 ratchet 通过（6362 文件，受管源码 >700 为 0）。
  本批 11 个 Rust 文件均不超过 386 有效行；这不是编译、测试或真实上游通过的证明。

## Chat 与工具输入源码增量（2026-11-26，未运行验收）

- `ingress.rs`／`chat_request.rs` 只在 Cursor 内部投影原始 Chat JSON，不采用通用解析器
  修复过的工具参数；不改写 pipeline endpoint、原始 wire、scope 或审计归属。
  嵌套 function／custom 声明和调用映射到既有原生 MCP owner，
  助手前导文本与调用按同一顺序进入完整历史，工具结果绝不进入新用户 prompt。
- 冻结身份增加入口 scope；首发与工具结果发送前重新核同一个 Local Key、精确账号、
  当前 token 和配置修订。Responses 与 Chat、HTTP 与 WebSocket 均不能互认暂停。
- Chat JSON 复用现有 response pack；SSE 直接输出 Cursor 专属 Chat delta，
  纯工具首 delta 也包含 `role:"assistant"`，不误套 Responses 事件外壳。
  Chat HTTP body owner 只包装拿到 Cursor publication 的响应，不影响其他服务商或 legacy completions。
- 新 Run 的 `none` 真正不向 worker 传工具定义；旧工具历史／活 Run 策略变更仍拒绝。
  `text_output.rs` 在输入边界规范纯文本数组为精确字符串，历史匹配和 IPC 使用同一结果，
  不修剪、不加分隔符、不解码实体，不接受媒体、未知字段或超限。
- 回归源码覆盖两种 HTTP 格式、function/custom 混合三轮、纯工具角色重建、
  字符串／数组历史等价、错 Key／跨协议／重复输出拒绝、none 不发送定义、
  body 未消费／读取中丢弃／错误／截断／坏后缀／非法 UTF-8／超限、
  命名 `[DONE]` 及 guard 失效。失败取消断言不先强制关闭 runtime 掩盖泄漏。
- 三路只读复审发现的纯工具角色遗漏与命名 `[DONE]` 提前接受已修补。
  本批按用户要求仅修改源码及回归用例，不编译、不运行测试、不出包、不切换实例。
  这些合成 fixture 源码不是 Cursor 实际响应或完整编程验收证明。
- 静态收口：三路复审修正后未发现新增阻断；定向 rustfmt、scoped diffcheck、
  21 个文件的 UTF-8 无 BOM／尾随空白检查通过。20 个改动 Rust 文件最多 283 有效行；
  权威 ratchet 扫描 6370 文件通过，受管源码 >700 为 0；上述检查不代替编译和运行回归。

## 内联图片源码增量（2026-11-26，未运行验收）

- Responses `input_image.image_url` 和 Chat `image_url.url` 仅接收规范 base64 的
  `data:image/png;base64,...`／`data:image/jpeg;base64,...`；`detail` 只允许省略或 `auto`。
  不下载 URL、不接受 `file_id`，不把图转成文字，不改变 ASK 权限；工具结果仍只允许文本。
  仅最后一条 user 消息可含图，图片顺序映射到原生 `selected_images`。
- 固定第一方 SDK 1.4.2 描述符和构造代码已确认
  `UserMessage.selected_context #3 → SelectedContext.selected_images #1 →
  SelectedImage.uuid #2 / mime_type #7 / data #8`。`data` 是原始 bytes，
  不同时发送 blob／upload oneof 字段。文本、function/custom 与新 Run `none` 共用编码器。
  SDK 构造代码证实 AGENT 图片字段，不证明 ASK2 在当前账号可接受；
  本实现保持 ASK2，**尚无远端图片成功证据**。
- 以下是 Gateway 门禁，不是 Cursor 官方额度：最多 4 张，单张解码字节 ≤1 MiB，
  合计 ≤2 MiB；单边 ≤4096、像素数 ≤4,194,304、解码器分配预算 32 MiB。
  首发完整解码并拒绝损坏／动画 PNG，同时最多两个阻塞解码任务，不排队；
  取消后解码 permit 留在后台任务直至结束，外层预算与取消仍覆盖请求。
- Cursor 原始 JSON 上限 4 MiB；剥图后的文本／声明保持 512 KiB，prompt 保持 64 KiB。
  初始 IPC 的嵌入源码、凭据、请求及 LF 合计 ≤4 MiB，在 spawn 前预检。
  现有 HTTP／WS 外层配置若更小仍优先，不自动放宽全局请求或帧上限。
  授权命令仍 128 字节，工具结果仍 128 KiB；追加最终 JSON 转义大小前检，
  防止原始字符串合格却在发送授权后才被 Node 判超限。
- 工具签名额外绑定每图的 MIME、字节 SHA-256、消息／part 索引以及图前文本分段摘要。
  暂停槽不保留 base64。完整历史续轮必须重带同一图片，只比较有界字节摘要，
  不重复完整解压、不把图再发给活 Run；删换图片、调序、挪动图前文字或文本伪装不能认领槽。
  下述 WebSocket 活工具增量无需重传图片，继续沿用原历史和图片签名。
- 同步修复工具 prompt 的正文保真：固定工具政策在构造前缀时选择，
  不再对整份 transcript 执行字符串替换。
- 新增 Rust 图片准入／解码、HTTP 两协议与同连接 WS 工具往返、篡改后正确续轮、
  Node 原生 wire 与初始／授权／工具 IPC 边界回归源码。
  所有 worker／上游数据均为合成 fixture，**未编译、未执行这些测试、未出包、未切换实例**。
- 静态收口：三路只读复审完成，发现的正文替换、图片相对文字位置绑定、
  WS 测试错误码断言与 Node 测试局部变量问题均已修正。
  定向 rustfmt、Node `--check`、29 个文件 UTF-8 无 BOM／空白检查通过；
  28 个代码文件最多 278 有效行。权威 ratchet 扫描 6378 文件通过，
  受管源码 >700 为 0；这些结果不等于类型检查、运行回归或真实上游验收通过。

## WebSocket 活工具增量（2026-11-26，未运行验收）

这是同一活 Run 的工具结果投递接口，不是通用 Responses 历史存储或新消息续聊。
首轮仍发送完整 input、tools 和 instructions；拿到工具阶段的
`response.completed.response.id` 后，可在**同一 socket** 发送：

```json
{
  "type": "response.create",
  "model": "cursor-personal",
  "store": false,
  "previous_response_id": "resp_当前工具阶段的实际ID",
  "input": [
    {"type": "function_call_output", "call_id": "实际call_id", "output": "工具执行结果"}
  ]
}
```

custom text 使用 `custom_tool_call_output`；结果仍可为 `input_text` 数组。
增量 body 除上述字段外只允许 `stream:true`，其余字段必须省略。
尤其 tools、instructions、tool_choice、parallel_tool_calls 即使与首轮相同也不重传：
原 Run 的政策保持冻结，不把省略解释为撤销工具或重置 instructions。
`input` 必须恰好一条当前工具的结果，不接受混入消息、完整历史或多个结果。

- 连接只保存最新已成功交付并发布的 response ID 与原 publication。
  typed capability 只有原 registry 的弱引用和 epoch，不保存图片、不拥有取消权；
  原始 wire、canonical input 和审计中的 previous ID 不改写、不拼回历史。
- 每轮继续走鉴权、过滤、预算、exact 账号、修订与发送前授权；
  remote 配置失效、旧／错 ID 或能力缺失不能转 ACP、新 Run 或其他账号。
- 原子认领前复验 published、registry、epoch、冻结身份、call ID／类型、
  TTL、通道与 IPC／累计历史上限；输入不匹配不消费暂停。
  合法认领后若发送前授权失败，则清理原 Run，不重新挂回或自动重试。
- Session 保留原工具定义、政策／图片签名及有界规范前缀。
  增量输出追加后可再切回完整历史续轮；完整历史仍必须带原图片和政策。
  续轮不刷新 300 秒 TTL，最多 256 历史项、384 KiB；发布下一次工具暂停时为结果预留历史项。
- 严格 EOF、guard 预检、终帧写成功、二次发布均成功后才记录 ID。
  无工具的最终成功清空映射；旧 ID、文本终态 ID、另一连接和 HTTP 不能认领。
  断连仍取消当前暂停，不提供跨连接恢复、多路并发、steering 或 `generate:false`。
- 新增回归源码覆盖含图的 function 增量 → custom 完整历史 → custom 增量，
  错 ID／kind／call／政策／超限后正确续轮、跨连接／HTTP、重复输出、
  缺失终态 ID、非 owning capability、过期／关闭／历史上限，以及认领后授权失败零发送与回收。
  过期单测直接设置已过期时间，证明准入拒绝，不冒充真实计时器自动回收验收。
- 本批按用户要求只改源码及回归用例，**未编译、未运行测试、未出包、未切换服务**。
  与之前批次相同，真实 Cursor 图片、完整编程链路、实际模型和请求级 Free 归属仍待验证。
- 静态收口：三路只读复审未发现阻断；定向 rustfmt、scoped diffcheck、
  18 个文件 UTF-8 无 BOM／尾随空白检查通过。17 个改动 Rust 文件最多 396 有效行；
  权威 ratchet 扫描 6383 文件通过，受管源码 >700 为 0；不代表类型检查或测试通过。

### 仍不能宣称支持的契约

固定第一方 SDK／CLI 归档未找到单次请求的 required／指定工具选择器、
助手严格 JSON Schema 输出约束或生成 token 上限映射。
`McpToolDefinition.output_schema_json` 是工具结果结构，不能冒充助手输出 schema；
上下文计数中的 `max_tokens` 也不是生成预算。保留显式拒绝，Anthropic Messages 不借忽略
必填 `max_tokens` 过关。Gateway 对 thinking 事件的文本映射见下节，但通用 reasoning effort
与 encrypted_content 仍未映射。这些缺口不以提示词、截断或伪造响应绕过。

## thinking 事件映射的源码增量（2026-11-26，当时未运行验收）

- 固定第一方 SDK 1.4.2 的 `ThinkingDeltaUpdate`（update tag 4，text 字段 1）
  与 SDK 公开 `thinking-delta.text` 是唯一文本来源。不暴露 ThinkingNote（tag 28），
  不把工作流 summary、thinking_style、narration 或 duration 转成推理摘要或密文。
  未运行本地 SDK；继续使用 Gateway 自有 Node 解码器。
- Responses 使用 `type:reasoning`、`summary:[]`、
  `content:[{type:reasoning_text,text:...}]`，不填 `encrypted_content`。
  独立输出 `response.reasoning_text.delta/done`、content part、output item 事件；
  与正文／工具共用现有序号、输出索引、终态与 publication owner。只在 Cursor
  typed 入口启用，不从其他供应商任意 Chat 扩展字段推断 reasoning。
  字段依据为 OpenAI 官方 Responses retrieve 与 streaming-events 文档的
  Reasoning / reasoning_text 条目（本日通过文档连接器核对）。
- Chat 的 `message.reasoning_content`／`delta.reasoning_content` 是 Gateway
  字符串兼容扩展，不声称 OpenAI Chat 统一标准。无 thinking 时不添加该字段。
  输出仍按阶段缓冲：同阶段 thinking 顺序拼接并独立于正文，不声称实时或保留两者交错时序。
- 原生 MCP 调用被接受时立即快照正文与 thinking；工具之后、即使在同一 TCP chunk
  内到达的文本也归下一阶段。继续保留本地写背压下的 `continued` 优先交付。
  严格 UTF-8，保留首尾空白和字符串内 U+FEFF，不因每个 delta 开头的 BOM 字符丢字。
- Gateway 本地上限：每阶段 thinking ≤16 KiB、整个 Run 累计 ≤64 KiB；
  超限报错、不截断。Rust IPC 缺省兼容旧 worker，错误类型、重复 JSON 键和非法 UTF-8 拒绝。
  非工具 stdout 总上限 512 KiB，可容纳正文 64 KiB + thinking 16 KiB 最坏 JSON 转义；
  工具 Run 的累计 stdout 仍 ≤1 MiB。其他请求／历史／body 上限继续独立适用，
  不保证每个字段各自满额后仍满足总上限。
- 完整工具续轮必须回显实际发布的 thinking。Responses 校验精确 item ID、结构、
  位置和文本摘要；Chat 仅准带工具调用的 assistant 上的原始字符串，并校验位置与摘要。
  暂停槽只保存有界摘要标记；thinking 不拼进 prompt、不进入工具结果 IPC。
  缺项、篡改、重排、伪造摘要不消费有效暂停；同连接增量仍只需一条工具结果。
  最终无工具阶段的 thinking 不持久化，不能带它开启新 Run 或冒充通用历史存储。
- 仅有 thinking、没有最终正文且没有合法工具暂停，仍不是成功。
  完成行之后的任何 stdout、工具暂停期间额外 IPC、WS 坏尾缀仍阻止成功终态或续轮发布。
  既有 `effort:none`／可选 include 准入不新增远端参数映射，也不保证 Cursor 内部关闭思考。
- 已补 Node 阶段／字节边界、Rust 原始 IPC、两 HTTP 协议 JSON／SSE、同连接
  增量→全历史→增量及失败交付的回归源码；数据均为合成 fixture。
  **未编译、未执行这些回归、未出包、未切换服务、未做真实上游 thinking 验收。**
- 静态收口：三路只读复审的问题已修正，定向 rustfmt、9 个 Node 文件及 15 段
  嵌入 worker 的语法检查、scoped diffcheck 通过；36 个文件 UTF-8 无 BOM／空白检查通过。
  35 个代码文件最多 376 有效行；权威 ratchet 扫描 6390 文件通过，受管源码 >700 为 0。
  上述仅是源码静态证据，不是 Rust 类型检查、测试运行或真实协议验收结果。
