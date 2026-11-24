# Luma 媒体取消与任务费用事实保留

日期：2026-11-20。范围为 R14 Agents API 的请求取消收尾，不关闭整个 Luma 渠道。

## 改动与行为

原先任务 UUID、模型、操作和实际 credits 只随最终成功响应或失败 receipt 上交。客户端在轮询、下载或晚到费用补查中取消时，已有公共 pipeline／管理 probe 审计会写入取消状态，但拿不到已经校验过的供应商事实。

新增 `src/protocol/luma_agents/generation_audit.rs`，每个实际 Luma attempt／精确账号 probe 拥有独立内存快照，通过 Tokio task-local scope 交给原生成 future 更新。快照由已有审计 owner 持有；原 future 被丢弃时，原来的有界存储 finalizer 读取快照并完成审计。没有新增后台生成、补查、重试或远端取消任务。

- 完整响应通过 UUID、请求模型、操作类型、状态和非负有限 credits 校验后才记录。提交响应未完整、身份或状态错误时不凭空产生任务记录。
- 只保留任务 ID、模型、操作、供应商状态、`usage.credits_charged` 和 `mediaVerified`。不保留 prompt、Key、Cookie、签名 URL、任意供应商错误文本或媒体内容。
- 任务级状态 `completed` 可以与本地审计 `cancelled`／`failed` 同时存在：供应商完成不代表 Gateway 完成资产交付。`mediaVerified` 只在所有媒体验证及生成时间解析完成后置为 true。
- `null` 仍为未知，明确 `0` 及小数金额原样保留；后续缺失 usage 不擦除已经观察到的金额。晚到补查仅更新已经核对同一 UUID／模型／操作／终态的费用。
- 公共直接取消、公共失败／超时、管理直接取消及管理失败／超时复用同一份事实。正常成功及已上交失败 receipt 保持现有格式和优先级。公共错误正文仍不返回任务 receipt。
- 普通 Key 保持取消／失败退款，不把供应商 credits 换算成 tokens 或本地成功计费；取消不处罚模型健康。代价为每个在途 Luma 请求一个小型共享快照，无持久层 schema 变化。

## 验证

完整回执、原始日志、16 个本批源码文件的 SHA-256 与相对本批开始的 diff 已保存于[长期证据目录](../evidence/luma/gateway-luma-cancellation-20261120/verification.json)。以下检查均为本批实际运行：

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_agents --no-fail-fast` | 110 passed / 0 failed / 1 ignored；六项新增测试，原公共／probe 的取消与超时用例增加断言 |
| 同一 lib test 的 `model_probe_audit pipeline::lifecycle pipeline::attempt_audit pipeline::stage_finalize` | 31 passed / 0 failed，包含原 create／finish／drop／refund 所有权保护 |
| `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` | 通过，生产非 test 分支 |
| 代表性关闭组合 `--features family-openai-compatible-official-api` 的同类 lib check | 通过 |
| `npm run test:effective-lines --prefix scripts`／`npm run check:effective-lines --prefix scripts` | checker 31 passed；ratchet 通过，governed source >700 为 0 |
| `cargo fmt --all -- --check`、`git diff --check`、修改文本 UTF-8 无 BOM 与测试源码哈希核对 | 通过 |

ignored 为既有 `probe_browser_host::luma_agents_probe_normal_main_browser_host`，依赖专项浏览器会话；本批没有运行 UI 或浏览器验收。最初 formatter 检查报告新增代码格式差异，执行 rustfmt 后复检通过；该初次失败仅保留在工具结果，不补造日志。编译沿用仓库已有 warnings，没有借机修改其他服务商源码。Luma 测试构建约 13 分 41 秒，测试运行约 13.26 秒；不能由此推断冷／热构建提速。

新增测试复用现有 loopback HTTP、正常公共与管理 router、生产 Local bootstrap 和 SQLite 审计／余额设施。覆盖提交／轮询／下载取消与超时、成功／失败费用补查取消、零费用及小数费用、后续缺失 usage、错误身份／状态／usage、并发 scope 隔离、晚到费用更新及错配拒绝。验证只提交一次、关闭原 HTTP body、普通 Key 退款、没有秘密字段及健康误罚。

长期测试在 `src/upstream/luma_agents/cancellation_audit_tests.rs` 和 `observation_tests.rs`；现有 `pipeline_failures.rs`／`probe_failure_tests.rs` 增加审计事实断言。使用仓库已有 `tools/luma/run-check.py`，临时运行目录为 `linshi/gateway-luma-cancellation-20261120`，不是开发依赖。

有效行：新快照模块 68，新测试 225／128；`generation.rs` 210→232，`settlement.rs` 76→80，`PipelineContext` 所在文件 368→370，send scope 323→328，probe 审计 owner 206→230。所有新增和修改文件均不需要 501–700 例外，未更改基线或治理规则。

## 剩余边界

只保留当前进程在取消前已经收到并验证的事实。不证明远端生成被取消、不保证取消后的费用已知，也不保证强制进程终止后恢复。补查截止后追账、跨进程恢复、适用 cash 结算、真实账号生成／账单、Server 存储和新原生包仍需单独完成。未读取或修改真实凭证、数据库与运行实例，未调用真实收费服务，未提交或推送 Git。
