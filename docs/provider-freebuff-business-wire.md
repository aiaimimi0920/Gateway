# FreeBuff：目录绑定的 signed business wire

2026-11-08最新验证：[原run六阶段独立进程强制退出](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-run-process.md)已对真实child kill+wait、原OS fence、claim/run/seal恢复与heartbeat后禁止重放取得证据；新增1项含6场景，FreeBuff相邻463 passed/4 ignored及静态/来源审计通过，只增测试不改生产。恢复在父进程新runtime，不代第三进程/OS/原生包或远端安全续跑；固定公开START无发送前幂等身份、FINISH无金额receipt，内部GetAgentRunFromId类型不等于已证HTTP查询。未知结果不补发/end/rebuy，完整边界见回执。

2026-11-08最新开发：[原run/费用观测/FINISH事实持久化](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-run-journal.md)已强制接Local公共非流式Chat的原owner，START/model/FINISH intent先落库，真实run/client ID加密保存，原claim/run preimage原子CAS。当前未知不清除此前已确认累计，不造0；正常管理当前/历史与严格页面只读事实，不返回原始身份/body/secret。SQLite等待后重验active，未知store commit禁止后续wire。12新增Rust全过，最终合并502 passed/3旧ignored、前端209/类型通过，完整终态见回执。持久记录不等于续跑许可：未知START/FINISH的显式对账、远端lease/执行恢复、跨进程/包和真实平台仍缺，无自动FINISH/end/rebuy。下方各“尚无持久run”是历史停点。

2026-11-08最新开发：[公共付费阶段协作取消/原run费用收尾](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-cancel.md)已将caller/closed/shutdown/期限信号接入原paid owner；confirmed START后原dispatch最多20秒收尾，不延长生成或新attempt。reported费用显式FINISH，unknown不造0，在途FINISH等原ack、不end；函数工具单attempt可完整执行。9新增/5新增budget及正常HTTP shutdown已有证据，最终验证见回执。真正HTTP任务Drop/进程退出仍无detached FINISH，不承诺durable run/lease或START/FINISH未知恢复；下方“公共cooperative FINISH未接”是历史边界。

2026-11-08最新验证：[正常网页→活公共caller→逐次批准/取消→原claim显式end→下一次购买](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-ui.md)已用正常main/Edge、真实Local TCP/SQLite与合成上游联合验证，最终两次购买/模型/FINISH/原attempt DELETE、0 active/2 history、Local次数10→9→8。442去重Rust passed/2旧ignored、前端97/类型/main与静态/来源通过；中间编译、并行slot争用及stale-ref失败单列，不报全仓绿。仅退役第15节限定非流式Chat的正常UI联合缺口；不增加public end、付费阶段cooperative FINISH、完整run/lease或真实平台保证。

2026-11-08最新增量：[目录CLI / Local公共非流式Chat](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-chat.md)已接真实caller-native请求→原逐次确认/持久claim→原签名run/strict decoder/reported credits/显式FINISH；不是默认CLI agent，其他公共协议/模态仍拒绝。唯一付费POST前重验与attempt计数、START后资格复核、旧claim拒绝和未知费用不造0已验；FINISH不end。515去重Rust passed/2 ignored、两生产lib与静态/来源见回执。正常UI→真实管理TCP→活公共请求联合、公共cooperative FINISH和durable run/lease等仍缺；第15节是当前入口边界，下面各“后续子批/公共guard不变”是当时事实，不表示限定typed入口现在仍关闭。

2026-11-08最新增量：[前台自动摘要/canonical turn history](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-summary.md)已将匹配work receipt的80%阈值、同model/system/tools的结构化summary、真实EOF后保留当前用户的原子替换接入原run；summary是独立计费模型步骤，但不另START/购买，不污染work calibration。坏摘要/取消保留原history，未知费用不造0；14新增/相邻424 passed及其余终态验证见回执。是本进程前台子链，不是background/idle/manual、持久history/工具/child或公共业务，公共guard不变。

2026-11-08最新增量：[root上下文预测/usage校准](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-context.md)已接原run的实际native估算、完整成功usage anchor与runtime预算gate，17新增/相邻410项、两生产lib与静态门禁通过，最终来源/范围审计见回执。不是BPE/精确tokenizer、模型真实上限或自动摘要；不开放公共guard，也不改变reported credits/显式FINISH职责。历史章节的“计量缺口”由该批限定承接，完整summary/history/工具/child与公共业务仍未完成。

本页描述 `src/protocol/freebuff/catalog` 内的固定协议传输层及后续接线。当前正常管理 signed inspect、原claim显式end见第8节；请求拥有的正常逐次批准及限定公共消费者见第9、15节，仍无闲置管理购买端点。账号/目录/设备管理遵循 [FreeBuff 管理合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-login.md)。legacy `freebuff_business_admission_required` guard 保持原bytes；第15节的typed Local非流式Chat在旧执行器前接管，不再把它描述为全部目录公共请求都关闭，也不允许公共请求自动注册设备或跳过消费确认。

## 1. 公开参考与可信边界

本次联网核实 `CodebuffAI/freebuff` 固定 commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的完整公开客户端；不是当前线上 API 或发行 binary 的有效性证明。

- [session API](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-api.ts)：admission、status、release 的最终 method/path/headers/空 body。
- [CLI claim identity](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-identity.ts) 与 [purchase constants](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/constants/freebuff-desktop-sessions.ts)：`cli:<uuid>` instance 与同 UUID attempt。
- [completion hook](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-device-key.ts)、[最终 fetch wrapper](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/model-provider.ts)、[原样 body 测试](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/__tests__/request-headers-hook.test.ts)：签名覆盖最终实际发送的字节。
- [消费确认](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/hooks/use-freebuff-chat-admission.ts)：价格/余额变化须重新提问；服务器不执行 wallet 金额 ceiling。仅选模型或保存配置不等于消费授权。

35 个参考文件的 URL、blob SHA、实际 SHA-256 及 Git blob 对账在本批临时证据根的 `source-references.json`，不将第三方实现执行或复制进 Gateway。登录的 `https://freebuff.com` 不变；业务目标仍由已保存账号显式选择两个已证 origin 与 CLI/Desktop。

## 2. 不可变 scope 与最终 bytes

`CatalogSession::business_requests(device, stable_key)` 检查当前设备 capability 的 token/origin/client binding，并仅从本 session 的有效目录选择开放行。返回值借用不可变 account、device 与 catalog，不能在存活期间刷新目录，因而 handle 和 optional fetchId 不会来自不同 snapshot。locked、未开放、未知 key 或把 handle 当 key 都被拒绝。

`prepare_completion(final_body, limit)` 只接受 `model` 等于所选 handle 的最终 JSON object。它不替换模型、不组装 run/session metadata，也不把目录 key 当 agentId。按调用者指定的 `1..=50 MiB` 上限边序列化边限制，冻结一份 owned bytes；之后原 JSON 的变化不会影响发送。

`PreparedCompletion::send(self, budget)` 固定 `POST /api/v1/chat/completions`，对冻结 bytes 计算 SHA-256/Ed25519 后把同一份 bytes 交给 `.body(bytes)`；不再使用 `.json(...)` 二次序列化。completion 仅追加非空 fetchId 与设备签名，不照搬 session 的 catalog-protocol header，不伪造 CLI 版本、用户注意力或 gzip-recovery 能力。

HTTP response 原样交给未来执行 owner。该 owner仍须处理 HTTP/body、完整 decoder、stream/lease、计费与取消；本层不把 transport 2xx 当推理成功。completion 使用请求给定的总 timeout，最多300秒；wreq 的运行中总计时器继续随 response body，而不只在 headers 返回后终止。

## 3. 明确的 session 操作

| 方法 | 固定 wire | 副作用与边界 |
| --- | --- | --- |
| `inspect` | `GET /api/v1/freebuff/session` | 完整 pre-join snapshot；不 heartbeat、不购买、不排后台循环 |
| `prepare_cli_admission` | 仅本地生成 `cli:<uuid>` 与 attempt | 必须显式传 `MaySpendFreebucks`；这个 marker **不是费用确认流程或服务端上限** |
| `AdmissionRequest::send(self)` | `POST /api/v1/freebuff/session/admission` | 消费该请求对象；发所选 handle、wallet=`session` 与精确 claim，不 takeover、不自动重投 |
| `heartbeat` | `GET /api/v1/freebuff/session` | 同一 claim，multi-session/continuity、heartbeat 与 full rate-limits；可能更新远端 liveness，不声称纯只读 |
| `end` | `DELETE /api/v1/freebuff/session/attempt` | 精确 instance/attempt；响应未确认不能报释放或退款成功，不执行账号级泛化 DELETE |

所有 session 请求无 body，签名也覆盖空 bytes；统一发送 catalog-protocol、非空 fetchId、绑定设备签名及相应 client。claim 仅由 host 生成，不能从公共请求字符串/headers/serde 构造。heartbeat/end 在网络前核对账号 scope；claim 不以模型为绑定，因为它针对原 instance/attempt，而非重新准入当前选择的模型。

当前只实现已证 CLI purchase identity。Desktop 可执行显式 signed inspection；不会借 CLI prefix 猜测 Desktop 的购买/释放生命周期。

`BusinessReply` 是最多1 MiB的原始协议 JSON，保留 HTTP status 与可选字段；Debug不显示 body。**它不是公开 DTO，也不是 active/admission/refund capability。**未来 typed owner 必须同时校验状态码与业务状态、active 的全部必填字段、精确 instance/model/期限，以及费用/退款事实后才放行执行。没有 admission `queued` 状态；`waiting_room_queued` 是 completion gate 的429错误。

## 4. 未知结果、资源与尚未接通的 owner

会话单步 send+decoded body 共用一个最多20秒预算，零预算本地拒绝；明确禁用 HTTP retry/redirect。取消/drop 销毁本 future/response，不创建 detached task，不偷偷补 POST、DELETE 或 unsigned fallback。错误只返回固定安全代码；session reply body 仍是内部原始数据，不能直接发给 UI 或写日志。

admission 一旦发送，timeout、transport、redirect、超大或坏响应都可能意味着结果未知。调用者必须在 send **之前**保留并持久化 claim/pending，之后精确对账，不因 transport 错误准备一个新的购买。当前裸 `SessionClaim` 仍非可反序列化的外部能力；后续[本地持久 claim owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-claim-recovery.md)已通过 crate-only 密封 receipt 实现先持久化再一次发送、原 claim signed heartbeat 和 unknown 保留。它不接正常购买/公共调用，既有记录不自动清除或重买，费用确认与终态 closeout 仍未接。消费 marker 与一次性 Rust 对象不能替代管理身份、Secret Grant、revision、当前价格/余额确认、幂等合同或跨进程恢复。

dedicated `/session/reuse` 仅核到公开路径与“精确 live instance，never buy/take over”的常量说明，没有找到 method/body/完整 headers/handler，**本层不实现猜测的 reuse，也不回退旧自动 POST**。单独的 renewal/takeover 和旧 single-session release 同样不顺带开放。

公开 CLI 另证 catalog root agent 为 `base3-free-catalog`，不是 stable key、handle 或旧默认 agent。后续[run/context 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-run-context.md)已新增内部持久 active owner 的独占借用：固定 START 返回的真实 run ID、当前 handle/fetchId、原 claim 和 host metadata 贯穿冻结 completion 与响应 body；不是完整 root harness、decoder/费用/FINISH 或正常公共执行。START/FINISH 与 signed session/completion 是不同 wire，不能伪加设备签名或沿用旧 FINISH 的固定0。

后续[冻结报价子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-purchase-quote.md)新增内部 one-use quote→fresh signed inspect→原持久 claim 的组合：冻结完整 meter，60 秒单调 TTL，绑定账号/registration/稳定模型/revision，在 commit 前后和 POST 前重验当前时间投影及调用者 fence；已有 row 只恢复原 claim，不要求新购买许可。无管理购买路由或 UI/public receipt，真实 Grant/revision 与 public caller/candidate 的可信接线仍待完成，不把内部 callback 当已验证正常管理授权。commit 后撤权/取消可能保留未发 POST 的本地 row；不删除、不自动换身份重买。没有服务端金额 ceiling/CAS，不能宣称锁价。

逐次消费/排队确认的正常入口、完整 agent/run/lease、正常管理/公共入口的终态 closeout、公共 Chat/Responses JSON/SSE、probe/完整取消、Server/跨进程/原生包及真实权益继续集成。内部原 claim 收尾见第6节；不能拿本页或内部子链的 transport 测试替代整渠道验收。

## 5. 2026-11-07 补充调查：root 与费用，不解除公共 guard

同一固定 commit 的 [agents/base3.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/agents/base3.ts) 已匿名取得：`createBase3CliRoot` 明确区分基础 harness 与 CLI 产品工具，并要求 appendix 追加在 canonical opening 之后；[catalog→compiled model](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/util/freebuff-catalog-compiled-model.ts) 通过 legacy digest 找编译模型，不把 opaque handle 当 root 身份。它们是现有客户端的行为证据，不单凭注释断言整个 CLI 工具集都是普通 Gateway 聊天的服务端强制要求；也不以只补 opening 冒充完整业务支持。

费用换算函数并非未知：[sdk/src/impl/llm.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/llm.ts) 的 `calculateUsedCredits` 为 `Math.round(costDollars * (1 + PROFIT_MARGIN) * 100)`。本轮取得的 `common/src/old-constants.ts` 只是 re-export，不据此猜 margin 数值；更不能用公式替代实际响应费用来源、跨事件累计与 FINISH 确认。公共执行仍缺可信 server-side pending→实际调用者/候选绑定、正常 Grant/revision/逐次消费确认、完整执行/stream owner 和终态 closeout，不开放闲置购买或删除 `freebuff_business_admission_required`。

新增四份成功原文及失败路径的 URL/UTC/HTTP/SHA-256 见 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-completion-lifecycle-20261107/sources.json`；root 原文 SHA-256 为 `49f47878c9af7a3bd05a2ae91ffb1074447884c7c4667fab31aaa74e95772a14`，完整 LICENSE 为 Apache-2.0。GitHub recursive tree 匿名请求返回 403 rate limit，本轮不绕过、不使用凭据重试。没有复制或执行第三方实现，没有真实平台调用；本段仅补调查事实，不增加模板、测试通过数或整卡完成数。

## 6. 原 claim 的最终 end receipt 与本地收尾

后续[收尾子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-claim-closeout.md)重新匿名取得同一固定commit的session类型、API、测试与LICENSE。`ended`不等于已结算：optional `freebucksRefund`含0才是最终金额，pending需后续显式同instance DELETE；instance仍存在表示grace。end保留购买小时，通常refund=0。裸ended、GET合成状态与404不能清除未知购买，不承诺退款。

内部`SessionRecovery::end`持原OS guard，发送前核原claim sealed preimage，再一次签精确attempt空body DELETE；只有无pending/grace的明确最终回执才在同一SQLite事务加密保存原claim/receipt、CAS删除原active row并核fence。失败/未知保留原义务，不自动重投/重买。commit-await超时的调用者须核原claim与历史，不能断言事务未提交。schema-1 additive历史参与所有sealed owner启动检查；只剩历史而seal缺失也不生成新身份。历史reader绑定原authority/attempt/AAD，只返回事实，不恢复许可或映射成Gateway余额。

这里只接内部收尾；fence测试是模拟回调，不是已连接真实Grant/revision。正常管理/UI end、公共caller/receipt/candidate、完整root/decoder/费用FINISH/run/lease、Server/跨进程/原生包与真实平台/退款仍缺，公共guard不变。最终验证及四份原文SHA见收尾回执；证据根为`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-claim-closeout-20261107`。

## 7. native completion 聚合、reported credits 与显式单步 FINISH

后续[解码/单步FINISH子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-catalog-completion.md)补齐第5节当时未取得的`constants/limits.ts`：固定SDK的`PROFIT_MARGIN=0.055`。`usage.cost`和`usage.cost_details.upstream_inference_cost`各取最后非null数值，再相加换算一次reported SDK credits；不是逐chunk相加。缺失不造0，numeric0有效；SDK credits、上游估计、Freebucks小时价格与Gateway用户扣费不混用。

内部`CatalogCompletionResponse::decode`沿原run/claim独占借用读取HTTP200 native JSON或严格SSE，保留多choice/reasoning/tool/usage；共享64MiB上限与timeout，真实finish加实际EOF才返回成功，DONE可省略但不代替finish。错误/截断/坏UTF8/身份冲突/终态后数据/超限不假成功。`finish_single_step`只完成host拥有的一次模型步骤、无工具/子agent执行；按解码结果和reported费用发原run的Bearer FINISH（steps1/direct=total），不是完整agent harness或公共实时pipeline。

FINISH与signed completion是不同wire；不签设备头、不redirect或自动重投，HTTP2xx不是结算receipt。FINISH不释放原session/claim，Drop/取消不隐式FINISH。完整多步/子agent与取消累计、持久run恢复、远端lease/幂等、真实Grant/revision/逐次消费确认、正常管理/UI/public end和完整公共业务仍缺，公共guard不变。公开原文、归档198份参考、失败分类和终态验证见新回执；不以本页声称当前线上已验。

## 8. 正常保存账号的本地 journal、历史与精确 end

[正常管理原claim收尾子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-managed-closeout.md)复用第6节原claim/AEAD/OS guard/final receipt事务，接正常管理token/SecretGrant/expected revision。读取按同一已保存账号scope离线返回原claim和每页20项历史，不获取执行guard、不生成key、不请求上游；原UUID仅为exact target，不允许foreign inspection认领。end在fresh signed catalog后恢复原guard，发送与归档边界核真实fence；已归档重复end零网络，旧attempt不作用于新active。缺金额/pending/grace/错误或撤权保留本地义务，取消/timeout不自动重放或宣称回滚。

正常账号UI显式检查→确认原attempt→end→重读历史已接；当前模型列表为空不阻塞本地历史。strict DTO/no-store/16KiB限制与Server无owner拒绝保留。后端真实router/SQLite/合成上游与前端单测分别已验；后续[本地联合子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-claim-local-ui.md)已补正常main浏览器→真实管理TCP：取消零上游，明确确认后唯一原attempt DELETE，final refund0/历史、同目录同端口runtime重建/完整document与revision保留、旧Grant拒绝及正常页面重读有实况断言。仅为合成原claim收尾，不冒称EXE/跨进程/真实平台或购买/public已通。第6、7节当时尚缺的管理end由本节承接；完整逐次购买确认/public caller绑定、harness/多步与取消费用/持久run/lease、实时公共协议及Server仍待接，新设备/目录应用的正常UI联合也未由本批覆盖，公共guard保持原bytes。

## 9. 请求拥有的内部批准与正常管理决定

[批准子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-request-consent.md)新增原quote TTL/请求预算/最多16项的pending，正常管理list/decision只定位原活请求，不生成quote、购买或启动generation。动态fence每次重新认证当前management token、SecretGrant、revision和精确已保存账号；原请求独占quote/Lease并在消费及原admission边界重验。批准后未消费仍占slot，Drop/过期/drain取消许可，无后台购买/自动重放。Binding来自服务端PipelineContext及当前精确payload lookup，不推进RR，不信任body user或客户端确认字段。

本批验证为生产router的in-process管理请求、typed内部caller、真实合成回环session wire与sealed SQLite组合，不是公共producer、正常UI或完整executor。绑定的冻结harness请求意图不等于尚未生成的run/session metadata或最终signed wire；后续host需保持同一意图，只由原CatalogRun追加运行身份，并由原签名owner冻结实际发送bytes。公共guard保留，完整公共消费者/逐次UI/harness/取消费用/持久run/lease/Server和真实平台继续，不把 delivered、HTTP2xx或一条claim当模型业务完成。

## 10. 原 run 顺序模型步骤、累计费用与协作取消 FINISH

后续[顺序模型子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-multistep.md)承接第7至9节当时未接的内部多步/部分取消费用。`DecodedCatalogRun::continue_run(self)`仅在完整语义成功和真实EOF后移交原唯一run与completion；错误/取消返还原decoded owner供显式收尾，不能继续。下一步仍检查原scope/模型/live observation，沿原runId/clientId/claim instance冻结并签实际bytes，不另START，不借旧固定0费用的Drop lease。

O(1)累计owner按固定SDK的模型回调语义，每步先round为reported credits再相加，单步last-value observation不跨请求继承。缺费用与累计overflow永久unknown，后来已知或0不能修复；JSON safe-integer上限是Gateway本地数值安全政策，不声称上游server上限。当前仅顺序模型lane，没有实际工具/child费用owner，direct=total与totalSteps仅覆盖已decode模型尝试；测试中的tool历史不证明Gateway执行工具或完整agent loop。

`finish_run`用原run发送一次显式completed/failed/cancelled FINISH，保留前步和本步已观察费用；unknown不伪造数字或发0。旧`finish_single_step`对累计非1步明确拒绝，不静默截断。`steps: []`沿固定SDK无pending记录的wire。`decode_until`允许host协作取消并取回原owner/部分费用，随后必须另行await FINISH；`CatalogRun::finish_cancelled`覆盖步骤之间及已确认START但未发模型步骤。后者0只表示未发模型步骤的reported模型credits，不表示购买免费或真实供应商扣额为0。

FINISH允许在短TTL observation过期后报告原run，但不放宽下一步completion准入；HTTP2xx不是结算receipt，不释放原session或持久claim。取消只覆盖decode和步骤之间，prepare/send错误仍用原错误接口、不返还run；直接Drop、START结果不明、FINISH失败/丢弃和进程退出尚无durable run恢复。没有后台重放或远端exactly-once保证。完整root/harness、实际工具/子agent与ancestry费用、公共消费者/逐次UI/所有阶段取消、持久run/远端lease、Server与真实平台继续，公共`freebuff_business_admission_required` guard保持不变。

本批没有联网，逐项复核同一固定commit已下载的SDK database/llm、公共数据库合同、limits、run-agent-step和LICENSE六份原文及原download/archive receipt SHA-256；不声称当前线上已验，也没有复制或执行第三方实现。最终验证与保留的失败回执见子批记录。

## 11. 统一模型步骤 owner：准备、发送、解码与协作取消

后续[统一步骤子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-step-owner.md)承接第10节的prepare/send丢失原run缺口。生产内部`CatalogRun::execute_step(self, body, limit, budget, cancelled)`将原run留在可取消send future外部，成功、拒绝、错误、超时和协作取消均返还`DecodedCatalogRun`。旧分段run prepare/send只保留给test-only wire夹具；底层`BusinessRequests`仍是裸协议原语，不冒充完整准入执行器。

一个模型步骤在prepare前计算最多300秒预算，headers/body/真实EOF只取剩余时间；同一pinned cancellation future贯穿各阶段。已取消请求不先序列化/签名，确定本地拒绝不新增模型步骤或污染前步费用。network future被轮询后的错误/取消按可能已发送处理，记录一次unknown尝试；decode保留实际观察费用。unknown不发伪造0 FINISH、不继续下一步、不重放；原owner须另行显式await FINISH，原claim不释放。

这是已确认START后的单步预算，不是整个agent、公共请求或FINISH总预算；host仍须传当前剩余预算。有界同步序列化/签名不可抢占，不承诺硬实时。直接Drop不发送FINISH，START/FINISH结果不明、进程退出和durable run/远端lease恢复仍缺，阻止未知费用FINISH不等于远端义务已完成。完整root/harness、实际工具/child及费用、公共producer/正常UI逐次确认与三公共协议仍未接，公共guard原bytes保留。

root调查复核同一固定commit既有归档中的base3、catalog agent、compiled model和LICENSE原文：canonical opening不是完整harness，公开归档没有注释引用的服务端root合同。下一处真实输入缺口是`ModelRow/SelectedModel`尚无同一row的`compaction/legacyDigests`，不能由opaque handle或显示名猜策略。本批未重新联网，不声称当前线上合同已验；来源SHA、失败分类和终态验证见子批回执。

## 12. 同目录快照的 root 模型输入

后续[模型输入子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-model.md)承接第11节发现的字段缺口。网络catalog解析optional compaction/legacyDigests，select和BusinessRequests借用同一row的handle/displayName/policy/legacy身份及同snapshot fetchId；无落盘/管理DTO或raw材料debug，不新增网络/购买/推理。字段缺失兼容，存在但无效的policy明确拒绝、清除永久坏snapshot，不转换成默认；临时refresh错误仍由原owner保留原快照。

固定公开参考的21项compiled模型，按row digests顺序取第一个已识别ID；unknown/未来格式仍可表示，不用handle/label猜身份，不把多个已识别项的flags合并。turn-ending与CLI batching提示符合固定base3选择，lean=true禁batching；这些只交给未来builder，不是prompt/compaction已消费。静态表不是当前模型/权益列表，compiled ID不替换wire handle或用于路由fallback/认证/额度。额外metadata及safe-integer边界是本地政策，非上游额外保证。

完整root/prompt、上下文预算/摘要/compaction与工具/child费用执行仍缺，不因模型输入齐全就开放公共guard。缺policy/maxContextLength保持缺失，不能猜模型token预算；现有单步owner和显式FINISH不能替代完整请求的持久run/远端lease、正常逐次消费确认及公共三协议。固定来源、独立UTF-16 digest向量、真实红测、13项定向和后续终态检查见回执；本批未重新联网或调用真实平台。

## 13. 默认 CLI root 文本/native 请求进入原 run

后续[root builder 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-builder.md)承接第12节的 prompt 消费缺口：同一 BusinessRequests 模型输入进入完整默认 CLI system core＋appendix、逐 user 的固定 instructions 与只在当前请求末尾的 wrapped step reminder。host 日期/知识/git/system context 单遍插入，插入内容不再解释成模板或命令；原生图片/reasoning/tool_calls/tool result 与 schemas 保留。opaque handle 仍是 wire model，provider data_collection=deny，不由 compiled 身份误加 Anthropic/Bedrock pin。

`execute_root_step` 在与原 `execute_step` 共用的预算/取消 owner 内组装，再复用原 run/claim/fetchId、最终 bytes serializer/signature、decoder/逐步 reported credits 与显式 FINISH。缺工具、预取消、body 超限本地返还原 run、无模型发送或新增尝试；不是新生命周期或公共准入。

这是默认 CLI 请求 builder，不是完整 agent。可信 host 必须真实提供宣告的工具与上下文计量/摘要、准备 bare native history；没有执行15项工具、lean自动摘要、child/工具费用、native tool history/prefill处理或完整loop。15项工具检查是本实现的默认CLI契约，不是服务端强制所有Gateway调用具备该集合的证明；不复制Loom实现或给予调用者本机权限。公共guard原bytes保留，公共producer/正常UI逐次确认/三公共协议、durable run/lease/Server/原生包与真实平台继续。

实质移植的固定提示文本已保留Apache-2.0原LICENSE/NOTICE与修改说明；P50出包仍需验证随包交付。13新增首次定向通过，相邻FreeBuff373 passed/0 failed/2 ignored（包含13项、不相加）、两生产lib/格式/checker/ratchet及源审计通过；无重新联网、第三方TS执行或真实凭据/供应商调用，不把这些结果当当前平台或整卡完成。详细来源/失败分类/边界见新回执。

## 14. root native history 准备

后续[history子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-history.md)承接第13节的当前snapshot清理缺口：`build_root_step(step, limit)`在生成任何提醒前，按真实user边界删除无后续结果的native calls，连续assistant不构成边界；迟到结果不能跨user补答，freestanding/duplicate结果本身保留。不信任native JSON中的providerExecuted能力标记，不猜未公开web sanitizer。

相邻assistant的同形content/reasoning连接、details/calls追加，其他metadata按固定converter shallow later-wins；原生非文本扩展保留，不猜模型/签名来源，非空string/part-array混合本地拒绝而非flatten。raw history按原`1..=50MiB`limit预检且不留额外bytes buffer；最终body仍由原run限制、冻结并签名。预检/坏字段/merge拒绝返还原owner、零模型尝试，预算/取消/费用/FINISH复用第11节owner。

这里只消费当前bare snapshot，不写回持久canonical history，不提供tokenizer/摘要或实际工具/child。固定runtime确有400000默认context-pruner预算，但不是已证模型上限且尚未实现消费；prefill判断用agentTemplate.model，不能按compiled Claude身份给opaque handle强加continuation。来源/许可、native适配差异、真实红测/夹具失败及终态验证见新回执。公共guard原bytes保留；公共producer/正常UI逐次确认、三公共协议完整执行、持久run/lease/Server/包/真实平台仍缺，模板和整卡完成数不变。

## 15. 目录 CLI / Local 公共非流式 Chat

[公共Chat子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-chat.md)将真实caller的native文本/函数历史、支持参数和完整响应接原pending/显式管理批准、持久claim与原signed run。
这是caller-native relay，不是默认CLI RootTurn/harness：不注入CLI prompt/工具，不修复或摘要caller history，不执行本机/child工具。
closed native字段校验在报价前拒绝隐藏音频/未知参数，opaque reasoning_details/call extra_content/arguments字符串保留。

报价/等待不占provider并发；批准后fresh inspect、原OS guard/SQLite事务与真实Grant/revision继续工作。
公共模式只允许新exact claim，预检及购买owner内都拒绝遗留/竞态claim，不能转legacy Restore或heartbeat。
唯一paid POST前异步重验mutable eligibility，再核费用/Grant/revision，最后计一次attempt；START后、模型前再次重验。
重验不重复消费provider rate-limit；commit后被拒绝保留unsent claim，不删除重买。报价仍不是服务端锁价。

原run添加host metadata后由原serializer/signature发送实际bytes，strict decoder/reported SDK credits/显式FINISH与原Local quota/audit收尾复用。
原生JSON不经canonical repack降维。未知费用不造0，不retry/fallback/rebuy；FINISH不释放claim，成功后的原attempt仍由正常管理UI显式end。
管理HTTP驱动的真实公共/管理回环TCP、SQLite、实际completion bytes的Ed25519验证和并发/费用/封禁race已验；后续正常main/Edge→管理TCP→活公共请求的批准/取消、精确原claim end/refund0/历史→不同attempt再购买联合已验，来源与失败边界见顶部最新回执。合成peer独立验证completion及空body DELETE的Ed25519和原instance/attempt，不是供应商线上验收。

后续公共付费owner已接caller/closed/shutdown/原deadline的协作取消：确认START后仅原dispatch最多20秒收尾，仍禁止到期生成/新attempt；已报告费用显式原run FINISH，未知费用不填0，在途FINISH等待同次ack，不重发、不end。函数工具单attempt的后续阶段不再被错误计为新发送。真实HTTP shutdown与直接pipeline watch/期限、SQLite audit/余额/permit已有本批证据。
真正HTTP任务Drop/进程退出仍只丢弃future并保留原claim，不排detached FINISH；START/FINISH未知后的持久run/费用/lease恢复仍未完成。
其他公共协议/实时SSE/模态、完整CLI工具/child与费用、持久run/lease、精确probe、Server/PG/Hosted、跨进程/原生包及真实平台仍缺。
17新增Rust通过，同lib完整名去重515 passed/0 failed/2 ignored、两生产lib和静态/来源证据见回执；模板仍17、无新增整卡offline_ready，P50/P60/P70与总目标继续。
