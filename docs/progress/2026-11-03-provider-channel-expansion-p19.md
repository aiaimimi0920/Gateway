# 服务商分渠道扩展：第 19 批，模型探测取消与审计所有权

> 2026-11-03（Asia/Shanghai）；P01 / P10、R09 / W05 的共享 probe 生命周期切片。本地审计取消实现与定向验证已收尾；PG 管理审计身份仍缺，不是 W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保护既有脏改，没有提交、推送、打包或重启实例。

## 1. 范围与产品规则

API Key、OAuth、Web 仍分别核实受众、产品和额度；Web 不是兜底。相同品牌、模型或 Bearer header 不证明共享额度。未知关系继续隔离并标 `unknown`。R01–R16 全部保留，本批不改变渠道名称、模板、认证受众或额度池，也没有新增外部产品事实、依赖或第三方代码。

本批修复共享 [model_probe_recording.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/model_probe_recording.rs) 的本地审计取消窗口。真实管理入口的定向 fixture 为普通 Qwen workspace 产品；共享 owner 的单元测试使用合成 Bedrock labels，不冒称逐一动态验过所有 probe 产品。

## 2. 所有权与安全边界

- 新 [model_probe_audit.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/model_probe_audit.rs) 仅拥有审计终态存储。调用方在 spawn 之前等待独立 **32 槽** semaphore；槽位贯穿创建、生成期间 guard、完成或取消写库。它是 Gateway 本地准入策略，不是厂商并发或额度，也不借用上游 AIMD/浏览器槽。
- 创建由短 owned task 完成，SQL 返回 ID 后在 task 内立即构造完整 guard，返回 guard 而非裸 ID。调用方取消、或已完成 output 未领取时，Tokio 的 output 析构仍触发取消所有权；不使用 abort-on-drop JoinHandle。
- 正常 `finish` 的 input 与 guard 一起移交给短 owned task，即终态提交点。等待者随后取消，既有 completed/failed 继续落库；SQL 前取走 pending，存储错误不偷偷转为 cancelled。
- 未提交终态时 Drop 写固定 `cancelled`、无 usage，保留精确 provider/model/credential trace；未知取消原因不伪造 `client_disconnected`。日志只记录固定文案及错误 code，不回显任意上游正文、凭据或 SQL 错误。
- LocalRuntime drain token 在 spawn 前取得，连续移交 create→guard→finish/cancel；没有“先释放再重新 track”的空隙。[local_runtime.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime.rs) 只增加 crate-private `FinalizerGuard` reexport，不改变存储 schema、版本、迁移或现有关闭流程。
- 不 detach 模型生成、HTTP body、健康更新或后续测试计划。健康 snapshot 仍只读；未提交结果的取消不记失败、不恢复健康、不编造用量。正常 audit finish 等待后仍执行既有健康 CAS，未宣称两种存储跨 owner 原子提交。
- Drop 使用 `Handle::try_current()`；没有活动 runtime 上下文时不 panic，但不能执行异步取消 SQL。runtime 强制退出时，即使有 handle，新任务也不保证执行；drain token 释放不是 SQL 成功 ack。当前 finalizer 没有 stop-admission，本批也不声称已关闭整个进程 shutdown 入场竞态。

### 2.1 PostgreSQL 审计缺口明确保留

现有 model probe 的 `CreateRequestAuditInput` 中 `api_key_id/user_credential_id/access_key_id` 都为 None；[PostgreSQL persistence](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/db/request_audits/persistence.rs) 在网络前拒绝，原文：

```text
request audit requires api_key_id, user_credential_id, or access_key_id
```

本批保留该校验，不伪造 principal、不借他人 Key ID、不放宽公共审计安全合同。**本地审计取消闭环不等于服务端 probe 审计已实现。**PG 的健康 snapshot/apply 是另一条路径，不能说 PG 所有探测能力都不可用。后续需要有明确管理身份与审计归属设计。

## 3. 测试与证据

证据目录：[P19](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p19)。TEMP/TMP/TMPDIR 指向其中 runtime-temp，独立 SQLite 与合成凭据不触及真实 `.ng`、数据库或运行实例。loopback origin 同时为非转发代理，未改写的 HTTPS CONNECT 也不能触达上游；没有历史凭据试用、真实生成或费用。

### 3.1 已执行红测

`run-red-v1.ps1` / `red-probe-cancellation-v1.log` 实际 exit 1：编译成功，**0 pass / 1 fail**。真实管理 router 收到单提示词探测，确认上游一次请求和一条 running 审计后 drop 外层 future，3 秒后仍无 cancelled 终态：

```text
Dropped probe left its audit running: Elapsed(())
```

循环在首个单请求分支失败，红测没有执行临时 test plan 分支，不把未执行路径也算已复现。

### 3.2 当前定向验证

`run-green-v1.ps1` 已实际 exit 0：provider_runtime 33、local_runtime 39、公共 Qwen pipeline 15、生产 integration 13（Bedrock probe 7 + Qwen workspace runtime 6），合计 **100 pass**。其中真实管理入口的单请求/临时 plan 取消均已执行。

随后只读审查指出三处覆盖应补强，主代理在不改变生产逻辑的情况下完成：

1. 原 SQL 失败测试关闭 pool，不能检出偷偷提交的 cancel 也一起失败。现改为健康 SQLite 上仅拒绝 completed 的 trigger；等待生命周期 permit 归还后检查唯一 row 仍 running/无 completed_at，取消更新不受 trigger 限制。
2. 新增无 entered runtime Handle 的同步 Drop：runtime 仍存活但当前上下文没有 Handle，不 panic、permit 归还；重入后 row 仍 running（不伪造终态）、带超时 close 能检查 drain 释放。它不证明 runtime 已 shutdown 时仍能落库。
3. 临时 plan 增加第二个 enabled case，取消首个在途请求后只允许一次实际发送，不再用单 case 冒充约束后续 case。

`run-green-v2.ps1` 在 `source-frozen-v1.json` 来源上实际 exit 0；最终定向结果如下，不把 v1 重跑或每个测试内部循环重复累计：

| 日志 | 实际 test 数 | 结果与边界 |
| --- | ---: | --- |
| `green-probe-v2.log` | 34 | pass；包含 7 项新 audit owner 用例、真实管理 Qwen 单请求/双 case plan 取消及已有精确 probe 回归 |
| `local-runtime-v2.log` | 39 | pass；本地 audit CAS/取消/重启、finalizer drain 及既有存储回归 |
| `qwen-pipeline-v2.log` | 15 | pass；公共 Chat/Responses buffered/stream、usage/audit、失败/截断/取消回归 |
| `probe-integration-v2.log` | 13 | pass；不含 test seam 的生产 library：Bedrock probe 7 + Qwen workspace runtime 6 |

最终合计 **101 pass / 0 fail / 0 ignore**，所有本任务 Cargo sessions 均已结束。真实 router 取消要求精确账号 `other`、映射后的 model、唯一 cancelled audit、无 usage/健康记录、上游 body 释放、无换号/重试或继续 plan case。完成/失败结果提交后的等待者取消另以 SQLite 写锁证明原终态继续落库，并验证 LocalRuntime close 等待 drain。

这不是全仓回归或完整 feature 矩阵。P18 通用 Responses 两项脱敏失败本批没有重跑/修复；不能把这 101 项定向通过说成该 suite 或整个仓库全绿。

## 4. 来源与有效行

`source-before.json` 在本批写入前记录八个 owner；`source-additional-before.json` 在修改 LocalRuntime 之前追加其 hash/有效行。`source-frozen-v1.json` 已冻结最终九个源码供 v2 验证。最终源码行数如下：

| 源码 | 前 → 后有效行 |
| --- | ---: |
| `src/provider_runtime.rs` | 80 → 81 |
| `src/provider_runtime/model_probe_recording.rs` | 161 → 154 |
| 新 `model_probe_audit.rs` | 206 |
| 新 `model_probe_audit_tests.rs` | 256 |
| 新 `model_probe_audit_test_support.rs` | 136 |
| `qwen_model_probe_tests.rs` | 115 → 117 |
| `qwen_model_probe_test_support.rs` | 153 → 177 |
| 新 `qwen_model_probe_cancellation_tests.rs` | 67 |
| `src/local_runtime.rs` | 132 → 133 |

新测试 256 行仅覆盖 audit 生命周期这一职责，仍低于 500，不需要例外；未修改 baseline 或添加有效行豁免。`run-static-v1.ps1` 已实际 exit 0：Rustfmt、checker **31/31**、ratchet **4018 文件 / 14 immutable / governed source >700=0**、全工作树 `git diff --check` 通过。ratchet 使用无报告入口，未覆盖原有脏报告；Git CRLF→LF 提示不是空白检查失败。

`run-source-final.ps1` 已实际 exit 0：最终九源码 frozen hash、四文档严格 UTF-8 无 BOM/链接/围栏与六份未跟踪源码、四份文档的独立 diff 检查通过，留痕于 `source-verify-final.log` / `untracked-diff-final.log`。P16 的模块注册和 Qwen fixture 是有意变化；其余 P16/P18 frozen owner 保持匹配。没有锁文件、依赖、存储版本或 schema 变化，没有接管 S06 Rust/Gemini 或其他桌面连接/设置/UI 改动。

该 final verifier 首次误将 `git diff --no-index --check` 的 clean-difference exit 1 当错误而提前退出。已用 linshi 内正常/尾随空白夹具实际核实：clean 为 1、whitespace 为 3；修正为仅接受 0/1 后复验全部通过，没有忽略 whitespace diagnostics，也没有改动源码来掩盖验证器误判。

本任务后端 Cargo 串行使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，不启用 management-ui、不运行工作区 Web 发布/清理；保留 P13 已发生授权遗漏的历史记录。观察到独立 Otter 编译，没有停止或修改它，也不把本次耗时当性能结论。

## 5. 仍开放的范围

普通 buffered 外层取消审计、PG 管理探测审计 principal、reasoning/refusal/多 choice 与全部工具参数保真仍需独立闭环。P18 通用 Responses 的两项脱敏失败、此前 feature/前端/expiry 未通过项与 P13 授权遗漏不因本批消失。

下一最小实现优先检查公共 buffered pipeline 的外层取消 audit owner，并先用真实 router 的持有响应体复现；不能拿本批 probe guard 或 P18 Stream Drop 代替它的证据。PG probe 的 principal 需要单独设计管理身份与持久审计关联，不能用假 ID 旁路公共检查。

Bedrock 普通参数/reasoning/signature、其余 API/OAuth/Web/媒体、P50 devtest 包和隔离启动、集中录制及真实逐渠道登录/刷新/生成/额度验收仍完整保留。W05/W06 不标整体 offline_ready，总 goal 继续 active。
