# FreeBuff：原 run 内的 root 上下文预测与 usage 校准

日期：2026-11-08（证据目录沿用启动日期20261107）。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留继承 dirty edits，不提交/推送。此批承接 root history 准备，不是完整 harness、公共渠道或总计划完成。

## 1. 实际入口与职责

`build_root_step` 在原 native history 准备、system/提醒组装后返回 body 与 request-local context facts；`execute_root_step` 在原单步预算/协作取消 owner 内检查预算，仍由原 serializer/signature 冻结、签署和发送实际 bytes。`CatalogRun` 仅新增本 run 内的校准状态，复用原 immutable BusinessRequests/目录/handle/fetchId/claim，不另起生命周期、store、START、retry 或摘要请求。

廉价估算覆盖实际 system、生成提醒、native messages 和 wire tool schemas。文本按 JS UTF-16 units 与 UTF-8 扩展加权后除3向上取整；message overhead8，识别的 image/file/audio/video 内容 flat1600，不逐字符计 base64。JSON 遍历最多100000节点/100层，超出返回明确本地错误，不作0或假成功；原 history/final body 的字节上限仍独立执行。native tool arguments 已是字符串，按文本估算，不重新 parse/复制大型结果；reasoning/details 与未知 native 扩展采用相应文本/JSON fallback。这是 native 适配，不是来源 internal part/schema 的精确等价计数，更不是实际 tokenizer 或供应商报告。

预算采用同 row 的 optional `maxContextLength`；缺失时，对当前 opaque handle 消费固定 runtime 默认400000，不按 compiled identity 偷套明文模型的特殊预算。**这是运行时策略，不是已验证的模型最大 context。** 本批采用本地硬预算拒绝：预测值大于预算时返回 `freebuff_business_context_budget_exceeded`，原 owner 可显式 FINISH；尚未实现参考 runtime 的80%阈值自动结构化摘要，不能将本地拒绝冒充 compaction 完成。刚好等于预算允许；不猜 output reserve 或真实费用。

## 2. 供应商 usage 与历史连续性

只有 native JSON/SSE 完整语义成功和真实 EOF 后的最终 usage 才可成为新基准：`prompt_tokens + completion_tokens`，cached input 已在 prompt 内，不重复加；scale 仅取 prompt/estimated-input，限制在0.5–2.5。后续预测为 `round(old tokens + (new estimate - old estimate) * scale)`；output 估计只加入本次 assistant，不把尚未发生的工具结果加入 anchor，下一步工具结果按 delta 计入。每次回执替换基准，不累加多步 token usage；reported credits 的原累计 owner 不受影响。

缺失、input0、负数、非整数、超 safe-integer、失败/取消或不能唯一确定 assistant 的多choice不创建新基准，不把旧事实擦成0。token 校准的数值安全策略是本地约束；不能据此推断真实费用、余额或退款。使用者可从 decoded owner 读取仅含数值的 forecast（原始估计/预测/预算/是否校准），无 raw body/secret/hash 输出。

原 run 仅保存 system/tools hash 与 prepared bare-history 的 SHA-256 prefix witnesses；通过 streaming hash writer，不保留第二份序列化 bytes 或完整历史。构造中只暂存最多4096个消息下标；hash 前先完成 bounded estimate。只允许与旧 expected prefix 相同的追加历史复用 receipt；host 替换前缀/system/tools 或 normalization 合并/删改最后 assistant 时，保守退回估算并使旧基准失效，不猜测历史等价。生成提醒不进入 bare-history witnesses，但进入实际请求估计。generic `execute_step` 不携 root witnesses，会清除校准；不跨 run、持久恢复、任意 opaque handles 或不同 harness 共享 receipt。

请求准备中的同步估算/哈希耗时在原步骤发送前扣除；EOF 后的 usage 观察也是有界同步工作，不可抢占，不承诺硬实时或严格全进程 heap 上限。无新后台任务、工具进程、队列、凭据/store/migration 或远端副作用。确定本地拒绝仍零模型尝试，保留之前的 steps/credits 与原 sealed claim；0只表示未发送模型步骤的 reported 模型 credits，不表示 session 购买免费或供应商实际扣额为0。

## 3. 固定来源与旧假设更正

本批未重新联网、未执行第三方 TS。复核既有固定归档 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`，archive SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`；逐项原文与 hash 见证据根 `reference-provenance.json`。不声称当前线上合同已验证。

关键来源为 runtime `util/token-counter.ts`、`util/context-token-count.ts` 及测试、`run-agent-step.ts`、`conversation-summary.ts`、common `compaction-policy.ts`、model-config、base3、原 LICENSE/NOTICE。**当前 token-counter 明确要求 cheap fallback/estimate 并禁止 BPE**，以免长重复空格阻塞 embedded server；model-config 内旧 GPT-4o/BPE 注释不能推翻实际消费者。前批“真实 tokenizer 尚缺”的说法在这里澄清：需要 provider-reported anchor＋cheap delta，而不是新增 BPE 依赖。

base3 的当前自动摘要使用结构化 conversation summary，不是机械移植旧 context-pruner；`cacheExpiryMs/cacheExpiryMinTokens` 虽参与严格 schema，base3 runtime 实际只消费 maxContextLength/maxOutputTokens。不得把 catalog cache expiry 充当摘要 idle trigger。完整 background summary、canonical history/persistence、摘要请求及失败/取消 owner 本批未实现；来源中的跨-session opaque-handle 等价关系也不能扩张成 Gateway 全局能力或跨 run 校准。

沿原 Apache-2.0 LICENSE/NOTICE 保留来源与修改说明；全部 Rust 仍由正常有效行政策测量，非 immutable/generated 豁免。P50 必须另验 notices 随 binary 分发。

## 4. 验证状态

真实红测 `red-v1` 已终态：0 passed / 1 failed，选定5214项输入稳定。旧实现没有预算 gate，超预算请求得到模型成功响应，触发行为断言；不是编译负测。初始失败回执与日志保留，不与终态通过数叠加。

| 实际终态检查 | 结果 |
| --- | --- |
| `cargo test --locked --offline --lib freebuff_root_context_ -j 4 -- --test-threads=1` | **17 passed / 0 failed / 0 ignored**，含3项真实SQLite＋回环TCP测试（usage分别验JSON/SSE） |
| 相邻 `--lib freebuff` | **410 passed / 0 failed / 2 ignored**，包含上述17项，不相加 |
| 默认生产lib、仅移除FreeBuff的代表性关线生产lib | 两项locked/offline/jobs4通过；关线不启management-ui，未跑关线单测或完整feature matrix |
| 14份Rust `rustfmt --check` | 通过 |
| effective-lines checker tests | **31 passed / 0 failed** |
| effective-lines ratchet | **4905文件通过**，governed source >700为0；不冒称strict或全仓功能通过 |

17项新增首轮即通过。真实SQLite/TCP证明目录预算拒绝只有heartbeat/START/显式FINISH、零模型尝试；JSON/SSE真实usage可使便宜文本估算本来低于预算的第二步在网络前拒绝，原第一步13 reported credits/steps1保留，原run/fetchId/实际bytes签名与sealed claim不变。替换history与generic harness后不套用旧receipt，四步累计reported credits52；这些是合成原run子链，不外推为当前平台、完整公共SSE或工具执行。

7个终态阶段各5219项选定输入before/after及最终源码一致。与上一批393项FreeBuff完整测试名对账，旧身份无丢失，新增恰好17，两个旧browser-host ignored未变；未重复无改动的独立LocalRuntime/pipeline_lifecycle全选择器，不挪用旧通过数。既有warning保留，没有无关cargo fix。

官方lexer按直接preimage测量：catalog210→211、business272→274、execution165→167、step96→109、decoded122→127、root builder96→104、原root tests251→252、claim index220→222；新增root context134、estimate143、request44、context tests196、estimate tests70、wire tests187有效行。最大274的既有business仍是单一业务wire职责，252的root tests仍只验证root组装，均处于251–500单职责允许范围；新增模块最大196。未修改baseline/exceptions规避门禁。

14份Rust＋7份文档/说明共21份范围文本检查UTF-8无BOM、直接preimage范围diff和Git空白；29份固定原文及archive逐项SHA、原LICENSE/规范化NOTICE、公共guard/baseline/exceptions原bytes保护与根checker report直接preimage恢复纳入最终审计。审查覆盖实际输入/secret/权限、JSON遍历/内存/复杂度、取消/原run费用/资源生命周期；没有新后台任务或工具进程。终态以证据根`audit.json`、`test-identities.json`、`final-lines.json`、各阶段receipt/log/source及`scoped-diff.patch`为准。

来源审计两次失败另列：首版临时脚本按suffix查`package.json`，在monorepo归档内歧义；改为固定root＋精确path后发现本批最初提取把`agents/package.json`误标为根`package.json`。29份原文逐项对账仅此一份错位；保留原manifest/原bytes和`audit-v1.py`，重新从同一SHA固定archive提取真正根文件并更新其manifest，再完整执行审计。`reference-correction.json`保留前后SHA及失败分类，未改变token-counter/context等实际行为来源、生产Rust或已通过测试输入，不以改测试或源码掩盖失败。

证据根：[gateway-freebuff-context-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-context-20261107)。本地测试使用合成账号、真实隔离 SQLite 与回环 TCP，不调用真实平台。未运行前端/浏览器、全仓功能、完整feature矩阵、Redis/Docker、现有实例或数据库、出包、部署、提交/推送。

## 5. 仍未完成

模板仍累计新增17个，无新增整卡 `offline_ready`；P50/P60/P70 与总目标继续。公共 `freebuff_business_admission_required` guard 保持原 bytes，不开放闲置购买；公共 producer/正常 UI 逐次价格余额确认、完整公共三协议与 public closeout/probe/全部阶段取消、实际 host tools/child 及费用、自动摘要/持久 canonical history、durable run/远端 lease、Server/跨进程/原生包和真实平台仍未完成。不拿本批 token forecast 当费用/权益或完整 agent 验收，也不复用已消费的 P34 一次性容器授权。
