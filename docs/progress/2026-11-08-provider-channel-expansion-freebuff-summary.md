# FreeBuff：原 run 内自动结构化摘要与 canonical turn history

日期：2026-11-08。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承 dirty edits 保留，不提交或推送。本批承接 root context，不是完整 harness、公共渠道或总计划完成。

## 1. 实际入口与历史所有权

内部 `CatalogRun::execute_root_turn(RootTurn)` 接入 `RootHistory` 的独占可变借用，继续使用原 immutable BusinessRequests、catalog handle/fetchId、claim、runId、clientId、签名 serializer、decoder、累计 credits 和显式 FINISH。没有第二次购买/START、平行 store、任务队列或自动重试。旧 `execute_root_step` 保持 snapshot-only/预算拒绝合同，不偷偷替调用者改历史。

`RootHistory::new` 接受已完成的 bare native 前缀；`begin_turn` 划定当前真实请求，`steer` 保留本 turn 内的新用户指令，`append_tools` 只接收可信 host 提供的结果，不执行工具。模型完整语义成功与真实 EOF 后的唯一 assistant 才追加，失败不编造回答；所有追加先在原字节/4096条限制内检查，失败回滚追加。历史是本进程中的 canonical owner，不是持久化 schema、磁盘恢复或跨进程会话。

摘要覆盖当前独占借用下的整个历史，候选为 summary memory + 当前 turn 全部真实 user 消息，当前请求/steering 的 native parts 和 opaque host 字段逐项保留。无需弱 fingerprint 搜索：摘要期间没有第二个历史 writer。生成的 system/逐 user/step 提醒不写回 canonical history；旧 assistant/tool 内容由摘要取代，不宣称无损保留每个旧工具结果。

## 2. 触发、模型调用与原子替换

自动触发只使用**原 work 请求完整成功后、与当前 system/tools/history prefix 匹配的 provider-reported input+output**，不是 forecast、缓存 expiry、模型名或累加 usage。严格大于 runtime budget 的80%才考虑；固定 prompt/tools/提醒 + 当前真实用户 + 摘要余量不能舒适落在 threshold 的85%内时，推迟到 reported hard budget。不能减少消息数时不为 summary 再发一请求。无有效 receipt 时保留现有 cheap forecast 的本地硬预算拒绝，不拿估算冒充上游事实。

summary 使用同一 opaque model/system/tools/native 请求前缀，末尾追加固定结构化摘要指令、`stream=true`、`max_tokens=16384` 和专用 compaction metadata/UUID；仍由原 runtime metadata/冻结实际 bytes/签名发送。目标长度为 `min(2000, floor(budget/10))`。这是同一 host 默认 root 的前台适配，不实现来源的40k/30k background 预计算、idle/manual/缓存摘要复用或其他 root variants；也不假装消费未支持的 row `maxOutputTokens`。生成提醒的 native prefix 复用不是 SDK internal tag/TTL 的精确等价实现。

沿固定来源移植英文八节指令、paired `<think>` 去除、`## Objective`/至少六个二级标题的翻译版提取、至少200 UTF-16 units 和 conversation-summary/historical-memory wrapper。只接受唯一正常 stop 的文本摘要；length、filtered/tool-producing、短/不识别或超1MiB文本明确失败，不调用返回的工具。摘要内容仍是不可信模型数据，不执行其中的文件、命令、UI 或网络指令。

只有摘要完整 EOF、文本校验、候选字节限制、重新 root build/cheap context 上限、取消/剩余预算均通过，才一次替换 canonical history。失败保留原 history；摘要成功后 work 再失败，则保留已经有效替换的 memory/live user，不追加假 assistant。下一 work receipt 从新历史重新校准；summary 自己的 usage 不成为 work anchor，失败也不靠擦除旧 baseline 绕过预算。

## 3. 费用、取消与资源边界

前台 summary 与下一 work 使用同一个最多300秒的本次 turn-step 剩余预算及同一 pinned cancellation future；复用原 send/decode owner，不叠加独立重试。同步 clone/build/估算/序列化/签名不可抢占，边界处检查已消耗时间，不承诺硬实时或严格全进程 heap 上限。

summary 是实际模型尝试，照原 ledger 每请求 round 后累计 reported SDK credits。坏摘要、超限或取消也保留已经观察到的费用；在 headers/费用未知时中断仍为 unknown，禁止伪造0的 FINISH。不添加 Drop FINISH、自动释放 session、重投购买或模型请求。`totalSteps/direct=total` 仍只覆盖这条 sequential model lane，不包含尚未实现的 host tool/child 费用；reported credits 不是供应商最终扣额/余额/退款证据。

输入 history/body 原限制继续独立执行。候选准备临时持有旧历史、native 请求和新候选，内存占用按有界输入线性增长；不保留所有历史版本、第二份永久 JSON bytes 或后台 summary。没有新文件/凭据/store/迁移、工具进程、后台任务或真实平台副作用。

## 4. 固定来源与验证

本批未重新联网或执行第三方 TypeScript；复核既有 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` archive，SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`。13份原文逐项 hash、summary prompt/提取/trigger 的固定向量及原 Apache-2.0 LICENSE/NOTICE 见证据根。source `conversation-summary.ts`、`compaction.ts`、`run-agent-step.ts`、`compaction-summary.ts`、`compaction-policy.ts` 是主要消费者，不用旧 mechanical pruner 或 cache-expiry 注释替代。

原 LICENSE/NOTICE 和修改说明继续随源码保留；本批不交付 binary，P50 必须验证 notices 随包分发。所有 Rust 由正常有效行政策测量，无 generated/immutable 豁免；不改 baseline/exceptions。

真实红测 `red-v1`：0 passed / 1 failed，5222项选定源码输入稳定；旧 snapshot-only owner 在第二个 work 被 reported budget 拒绝，自动摘要恢复断言失败。首轮实现定向 `focused-v1` 在编译阶段因 estimator visibility 的 `E0603` 失败，日志保留，不计为行为测试通过。

首轮可执行定向 `focused-v2` 为13 passed / 1 failed；六项SQLite/TCP全部通过，唯一失败是英文提取夹具误期望尾空格，而固定来源与生产实现均trim。只修测试的期望为`expected.trim()`，生产源码未改；`fixture-correction.json`和两个source快照核对只变这一份测试。初始失败不隐藏，不与终态通过数叠加。

| 实际终态检查 | 结果 |
| --- | --- |
| `cargo test --locked --offline --lib freebuff_root_summary_ -j 4 -- --test-threads=1` | **14 passed / 0 failed / 0 ignored**，含6项真实SQLite/TCP测试；部分测试内覆盖多个取消/错误case |
| 相邻 `--lib freebuff` | **424 passed / 0 failed / 2 ignored**，包含14项，不相加 |
| 默认生产lib、仅移除FreeBuff的代表性关线生产lib | 两项locked/offline/jobs4通过；关线不启management-ui，未跑关线单测或完整feature matrix |
| 20份Rust `rustfmt --check` | 通过 |
| effective-lines checker tests | **31 passed / 0 failed** |
| effective-lines ratchet | **4914文件通过**，governed source >700为0；不是strict或全仓功能回归 |
| 27份范围文本UTF-8无BOM、直接preimage范围diff与Git空白检查 | 通过 |

真实SQLite/TCP已断言：原run只有heartbeat/START/三次模型/显式FINISH，三次模型实际bytes签名与fetchId一致；首步/摘要/下一work的reported credits分别13/26/13，最终52、steps3，原sealed claim row不变。summary的合成99999 token usage没有成为下一work anchor。坏/length/超大摘要不替换原history且保留累计39 credits；费用已观察但EOF未到的取消/超时同样保留history与费用；header阶段取消仍unknown、无伪造0的FINISH。有效swap后work失败则保留新memory/真实用户，不追加假assistant或重投。合成回环不证明当前供应商、实时公共三协议或工具真实执行。

7个终态阶段各5228项选定输入before/after与最终源码逐项一致。与前批410项FreeBuff完整测试名对账，无旧身份丢失，新增恰好14项，两个既有browser-host ignored未变；未重复无变化的独立LocalRuntime/pipeline_lifecycle全选择器，不挪用旧通过数或执行无关cargo fix。

官方lexer按直接preimage测量：catalog211→215、business274→278、claim index222→228、root builder104→104、history138→150、context134→149、request44→49、estimate143→145、execution167→168、step109→109、原context tests196→210；9份新增Rust最大161有效行，其他新增history100、root turn152、summary94、prompt39等均低于250。最大278的既有business仍是同一业务wire/error定义职责，位于251–500单职责允许区间；无baseline/exceptions修改。

13份固定原文与archive逐项SHA、原LICENSE/规范化NOTICE、完整summary指令的静态重建逐字向量、公共guard/baseline/exceptions原bytes和根checker report直接preimage恢复已审计。源码审查覆盖实际输入/secret/权限、同步分配/复杂度、费用与取消/所有权和资源生命周期；没有新后台任务/工具进程。终态以`audit.json`、`test-identities.json`、`final-lines.json`、各阶段receipt/log/source及`scoped-diff.patch`为准。

证据根：[gateway-freebuff-summary-20261108](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-summary-20261108)。真实 SQLite + 回环 TCP 只用合成身份，peer 显式 await close；临时 SQLite/脚本/日志位于本证据根或其 TEMP。没有调用真实平台、读取历史凭据或变更Redis/Docker对象（只读核对过容器清单），不操作现有实例/数据库；未跑前端/浏览器、整包、部署或 Git 提交/推送。

## 5. 未完成与下一接力

模板仍累计新增17个，无新增整卡 `offline_ready`，P50/P60/P70 与总目标未完成；公共 `freebuff_business_admission_required` guard 必须保持原 bytes。本批不开放公共购买或把 schema/prompt 当实际工具权限。

尚缺 background/idle/manual 摘要、canonical history 的持久保存/崩溃恢复、durable run/远端 lease、真实 host tools/child loop 与费用回调、公共 producer/正常 UI 逐次价格余额确认、公共三协议及 public closeout/probe/全部阶段取消、Server/跨进程/原生包和真实平台验收。不复用已消费且已清理的 P34 Redis 一次性容器授权。
