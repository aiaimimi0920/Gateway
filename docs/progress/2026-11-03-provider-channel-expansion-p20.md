# 服务商分渠道扩展：第 20 批，公共 buffered 请求取消所有权

> 2026-11-03（Asia/Shanghai）；P01 / P10、R09 / W05 的公共 pipeline 生命周期切片，不是总计划 P20 OAuth 工作包完成。
> 本批实现与 147 项定向离线验证已通过；不是 W05/W06 整体、OAuth 工作包或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；没有提交、推送、打包或重启实例，保护其他桌面连接、设置和 UI 脏改。

## 1. 范围和复现

API Key、OAuth、Web 继续按产品、受众和实际额度分别评估；Web 不是兜底，未知共享关系隔离并标 `unknown`。R01–R16 全范围保留。本批不改变模板身份、认证受众或凭据池，也不新增外部产品事实、第三方代码或依赖。

本批针对 [run_pipeline](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/mod.rs) 的外层 future 被 Drop 后，buffered 请求审计仍为 running 的缺口；不以 P18 Stream Drop 或 P19 probe guard 代替公共入口证据。

证据目录：[P20](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p20)。`run-red-v1.ps1` 实际编译成功，**0 pass / 1 fail**；真实公共 Chat router 确认一次上游发送和 running audit 后 drop 外层 future，3 秒内没有取消终态：

```text
Dropped public buffered request left its audit running: Elapsed(())
```

红测的循环在首个 Chat 分支失败，Responses 分支未执行；不把未执行分支算作已复现。TEMP/TMP/TMPDIR 指向本批 `runtime-temp`；SQLite、Key 和回环响应均为独立合成 fixture。loopback 同时为不转发的代理，未改写 HTTPS CONNECT 不能访问真实上游；没有读取历史凭据、触碰真实 `.ng`、数据库、在线实例或产生上游费用。

## 2. 实现及其责任边界

### 2.1 不能仅在 Drop 中重跑普通 failure

原有 Filter SQL/Redis 预扣、Route auto-route source key 预扣、Cash durable reserve 都有异步返回与 ctx 记录责任之间的取消窗口。quota refund 没有 request ID 幂等，不能在不明确终态 owner 的情况下无条件重跑退款；Cash 已有 request ID ledger 和 NotSent/Unknown 规则，继续复用。

新 [lifecycle.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/lifecycle.rs) 在鉴权后、创建 audit 和任何预扣前建立完整 `Owner`，连续拥有 boxed ctx、AppState 和 lifetime。Audit、Filter、Route、RateLimit、Cash 分别进入短 owned task；task 始终持有 armed Owner，并返回 `(Owner, result)`，不是先返回裸 ctx 再补 guard。调用方取消或完成 output 未领取时，owner 析构负责中性终结，不继续下一 stage。

生成 `cpa_scope::send` 仍在调用方同一 task 执行，保持实际网络/body 取消与原 CPA/final-URL task-local seam；不把模型生成 detach 到后台。

### 2.2 正常终态和 SSE 必须互斥

- buffered JSON、Binary 或错误得到后，在首个 await 前把 ctx、结果和 lifetime 移交正常 terminal task，消费取消责任。等待者随后 Drop 不把已提交 completed/failed 再改成 cancelled，也不重复退 quota。该 task 包含原有正常 finalize 的计费、审计、报告和适用健康更新，不声称这些跨 owner 原子提交。
- SSE 在首个 await 前同步取走 ctx 并返回，不执行新的生命周期终结；现有 TrackedStream 独占快照、quota/cash 和审计。没有“得到 SSE 后先 await 普通 finalize”造成双 owner 的空隙。
- 未提交结果的 Drop 进入新 [cancelled.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_finalize/cancelled.rs)：原有 cash NotSent/Unknown、quota refund、固定原因的中性 audit/archive、失败 usage report。无成功 usage、不调用 provider/model 健康更新；未知 Drop 原因不伪造 client disconnect。
- [failure.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_finalize/failure.rs) 仅将既有失败 usage report 抽为共享函数，正常 failure 的退款→审计→报告顺序保留。报告中的零 token 是 `success=false` 的失败报告，不是成功零用量发票。

### 2.3 新准入策略和退出限制

独立全局 semaphore 为 **128 个 admitted lifecycle 槽位**，涵盖 admission、buffered 生成与终态存储；SSE handoff 释放。它是新引入的 Gateway backpressure，不是纯 SQL 并发，不是厂商并发/配额，也不借上游 AIMD 或浏览器槽。**等候请求数量和已解析请求体总内存没有由此获得全局上限。**

入场等待同时监听 shutdown、显式 caller watch（包括 channel 关闭）及当前 RequestBudget deadline；此时没有 audit 或预扣。入场发生在 Route/Send 配置 ClientConnected 长等待策略之前，因此使用初始 budget（普通请求默认 300 秒）；不能说所有等待时序完全不变。每个 stage 前再次检查；已开始的 owned admission stage 不直接 abort，完成当前 stage 后才停止后续阶段，以保留预扣责任，代价是退出需要等待当前存储操作。

LocalRuntime finalizer guard 在 spawn 前取得，并连续移交 stage→Owner→正常终态/取消 task；ctx 的 CashGuard 析构可先登记其既有幂等 fallback，再释放 lifecycle drain。SSE 沿现有 terminal callback 登记 drain，这不是为所有活动流新增完整生命周期 drain，也没有关闭整个进程 stop-admission/close 入场竞态。

Drop 使用 `Handle::try_current()`，无活动 runtime 时只警告、不 panic，不能完成异步 SQL；runtime 强退、新 spawn 被丢弃、进程崩溃或 SQL/Redis 网络结果歧义不保证持久化成功或 exactly-once。permit/drain 释放不是 SQL 成功 ack。

## 3. 验证记录

### 3.1 实际保留的首次失败

- `green-lifecycle-v1.log`：编译失败，3 个 `E0659`，新测试同时 glob 导入生产和 fixture 的 `run`；改为显式导入。
- `green-lifecycle-v2.log`：编译失败，2 个 `E0609`（audit view 没有直接 `route_attempt_count` 字段）及 1 个 `E0308`（测试 stream error 类型不匹配）；改为断言真实 `route_trace.routeAttemptCount`、使用 typed `TrackedStream::new_with_error`。

以上都没有执行绿测，不算产品通过，也没有删除或覆盖失败日志。`run-green-v3.ps1` 在冻结源码上实际 exit 0；不把失败尝试、重复执行或测试内部循环当独立 test 累计。

| 日志 | 实际 test 数 | 结果 |
| --- | ---: | --- |
| `green-lifecycle-v3.log` | 6 | pass；admission/正常终态/queued/Route error/Cash/SSE owner |
| `qwen-pipeline-v3.log` | 16 | pass；含本批真实公共 Chat、Responses buffered 外层取消及已有流回归 |
| `finalize-v3.log` | 12 | pass |
| `cash-v3.log` | 12 | pass |
| `request-budget-v3.log` | 5 | pass |
| `local-runtime-v3.log` | 39 | pass |
| `provider-runtime-v3.log` | 34 | pass；P19 probe 生命周期及既有精确探测回归 |
| `production-integration-v3.log` | 23 | pass；不含 test seam 的生产 library：Bedrock native pipeline 6、Cohere relay wire 4、Bedrock probe 7、Qwen workspace runtime 6 |

最终合计 **147 pass / 0 fail / 0 ignore**（单元 124 + 生产 library integration 23）；`audit-results.mjs v3` 已实际解析全部八份日志/11 个 suite，拒绝空过滤器、失败或 ignore，并保存 `test-results-v3.json`。这不是全仓回归、完整 feature 矩阵或真实平台验收；本任务 Cargo handle 已结束。

### 3.2 新测试的证明范围

- Audit/Filter 的 stage 前、stage 后、completed output 未领取，使用真实 SQLite token 预扣和余额/inflight/permit 检查。
- 正常成功/失败 terminal 提交后 drop waiter，用 SQLite 写锁及 close 等待证明既有终态继续落库、成功只结算 10 token、失败只退一次。
- 排队时 deadline、caller cancellation、closed caller、shutdown，不创建 audit 或预扣。
- Filter 真实预扣后 Route 返回错误，claimed 和 unclaimed 错误 output 分别保留 failed/cancelled 与单次退款。**不是 PG auto-route source key reserve 的动态验证。**
- 真实 Cash admission 与 durable receipt，取消前/后及未领取 output；NotSent 为 released、amount=0、reservation 清零，模拟已发生 attempt 时 Unknown 为 unresolved、保留 reservation/pending，不把未知费用当零消费。attempt 观察为合成值，没有真实调用 transport。
- SSE handoff 使用真实 SQLite quota、手工 TrackedStream callback 和生产 stream finalizer，断言 handoff 不退款、Drop callback 恰一次后退款。它是 owner 单元测试，不是完整公共 streaming integration；公共流的已有回归另随 Qwen pipeline suite 执行。
- 真实公共 Chat/Responses buffered 外层取消要求唯一 cancelled audit、精确 provider/model/credential trace、无 usage/健康、body 关闭、无重试换号、AIMD active 归零且容量不降低。

## 4. 来源、门禁与剩余事项

`source-before.json` 在实现前保存初始 owner hash/有效行；`source-additional-before.json` 在修改 request_budget/failure 前追加记录。`source-frozen-v1.json` 已冻结 v3 启动时的 13 个源码；新增两个补强测试文件的前值为 0。

| 源码 | 前 → 后有效行 |
| --- | ---: |
| `src/pipeline/mod.rs` | 404 → 359 |
| 新 `src/pipeline/lifecycle.rs` | 216 |
| 新 `lifecycle_tests.rs` | 200 |
| 新 `lifecycle_test_support.rs` | 178 |
| 新 `lifecycle_handoff_tests.rs` | 131 |
| 新 `lifecycle_cash_tests.rs` | 131 |
| `src/pipeline/stage_finalize.rs` | 132 → 134 |
| 新 `stage_finalize/cancelled.rs` | 34 |
| `stage_finalize/failure.rs` | 106 → 113 |
| `src/pipeline/request_budget.rs` | 257 → 260 |
| `qwen_pipeline_tests.rs` | 200 → 202 |
| `qwen_pipeline_test_support.rs` | 208 → 211 |
| 新 `qwen_pipeline_cancellation_tests.rs` | 91 |

`run-static-v2.ps1` 已实际 exit 0：官方 Rustfmt、checker **31/31**、ratchet **4025 文件 / 14 immutable / governed source >700=0**、全工作树 `git diff --check`。使用无报告 ratchet，不覆盖既有脏报告、不改 baseline 或豁免；CRLF→LF 提示不是空白失败。

`run-source-final.ps1` 已实际 exit 0：全部 13 个 frozen hash、四文档严格 UTF-8 无 BOM/本地链接/围栏、九份未跟踪源码与四文档的独立 diff 检查均通过，记录于 `source-verify-final.log` / `untracked-diff-final.log`。`--no-index --check` 只接受 clean 的 0/1，不放过 whitespace 的 3。P18 两个 Qwen fixture/register 是本批有意变化，P18 其余及 P19 已冻结 owner 保持匹配。

本批未修改锁文件、依赖、schema、存储版本、迁移或 S06 Rust/Gemini owner。Cargo 统一串行使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`；没有运行默认 management-ui / Web 发布清理 hook，P13 已发生授权遗漏的记录保留。

PG probe 的审计 principal 缺口、P18 通用 Responses 的两项脱敏失败、reasoning/refusal/多 choice/legacy function_call 和完整工具保真、Bedrock 普通参数及 reasoning/signature、其他 API/OAuth/Web/媒体、P50 devtest 包及隔离启动、集中录制和真实逐渠道登录/刷新/生成/额度验收仍开放。此前 feature/前端/expiry 未通过项不因本批消失；W05/W06 不标整体 offline_ready，**总 goal 继续 active**。

下一最小任务优先从 P18 留存的 `accumulate_responses_rejects_explicit_failure_after_partial_output` / `translate_responses_failure_never_emits_successful_stop` 两项失败入手，先复验当前来源再修复错误脱敏；原结果为 51 pass / 2 fail，没有 clean-HEAD 对照，不能称已证历史 baseline。本批没有重跑或修复这两个用例。PG probe principal、PG auto-route source reserve 动态取消、Bedrock 专属完整取消链和 lifecycle 无 runtime Drop 的独立测试仍需各自证据，不能拿本批本地 owner 通过替代。
