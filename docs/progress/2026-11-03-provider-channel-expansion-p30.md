# 服务商分渠道扩展：第 30 批，迟到配置命令的事务栅栏

> 2026-11-03；继续 R05/W03/P20 的 P29 安全前置。此处完成的是正常配置 owner 的精确事务撤销/恢复接线与定向验证，不是 P29、Grok OAuth 或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；既有脏改保留，不提交、推送、打包或替换运行实例。

## 1. 实际复现与实现

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p30`。
生产修改前的 `red-late-command-v1.log` 已终态：编译 11m42s，0 pass / 2 fail / 0 ignore。由 fixture 持有并明确 join 的独立命令 actor 先报告 accepted，再等待 gate；调用方 timeout/drop 不会取消该 actor。恢复后释放命令，实际观察到迟到 activation 仍成功、迟到 Prepared 仍被创建。不是把同一个 never-ready future 丢弃后声称远端取消。

本批实现：

- `TransactionRecord::prepared_identity` 保留原 tx、旧/新 revision、归档、YAML digest、backup reservation 和 createdAt；不把后续 phase/updatedAt 当作原 Prepared 身份。
- `revocation_tombstone` 从原身份及 createdAt 稳定生成 Aborted，不使用重试时钟，不改变存储版本或记录 schema。
- Redis store 先读取、严格解析/校验 gate，再用 Lua 对这些观测 raw bytes 作精确 CAS。gate 并发改变或权威无关时保留 `console_recovery_required`，不猜测成功。
- 精确 tombstone 优先于 active 同 revision；tombstone 永久保留，无 TTL、无 DELETE，阻挡迟到 SET NX 和 activation。已证明本事务 RedisActivated 时才可能归为 Activated。
- startup recovery 和 indeterminate activation 都先 settle gate，再重读权威 active，之后才决定完成或回滚。本事务 Revoked、后继事务激活同 revision 时，按权威修复 YAML/内存，但仍将本事务记 Aborted。
- 新 ACTIVATE 在 AlreadyActive 前验证本事务 gate 与 immutable revision payload，不再把另一事务的同 revision 成功记给旧事务。
- Local backend 在同一 writer guard 下同步读取本地 authority；没有创建一套虚假的远端 gate。未知/custom backend 的默认 settle 明确拒绝，不默认授权回滚。

主入口：

- [Redis 原子 settle](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/redis_store/transaction_fence.rs)
- [正常 owner 收敛](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/transaction_fence.rs)
- [activation 结果处理](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/activation.rs)
- [实际取消/迟到 actor 回归](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/transaction_fence_tests.rs)
- [真实 Redis Lua 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/console_transaction_fence_redis_contract.rs)

## 2. 复审发现与 v2 收敛

v1 的 10 项栅栏测试通过，但只读复审发现正常 `commit_document` 的明确失败分支仍直接 rollback。反例是 A gate 已撤销、B 激活同 R、A 收到 mismatch 后把权威镜像回滚旧值；只测 recovery helper 不足以覆盖该入口。

v2 把 RevisionConflict、PreparedTransactionMismatch、RevisionPayloadMismatch、Unavailable 全部接到 `resolve_failed_activation`。先 fence/reload；只有证明本事务已 Activated 且当前权威内容匹配才成功；Revoked 时按权威收敛并返回原错误，保留对应 rollback code。新增从实际正常 commit 进入 same-R successor 的回归。复审确认原遗漏已从源码关闭；复审本身不充当运行证据。

新增负向场景还包括：tombstone 回包丢失后不提前回滚、重复恢复稳定；settle 后权威推进到无关 revision 不修改本地镜像；不同身份/未知 phase/损坏 JSON 不被覆盖；unsupported backend 不先回滚再报错。

## 3. 最终 v2 验证

`source-frozen-v2-full.json` 冻结 21 项源码/测试输入。以下命令均已终态；不累加 v1 或 P29 的重复测试来制造完成数。

| 检查 | 实际结果 / 日志 |
| --- | --- |
| 新栅栏及身份合同 | 11 pass / 0 fail / 0 ignore；`green-fence-v2.log`，编译 10m17s、运行 1.51s |
| Console 全模块相邻 | 117 pass；`console-adjacent-v2.log`，包含上述 11 项 |
| 刷新调度全模块 | 52 pass；`refresh-adjacent-v2.log` |
| 正常 coordinator 事务/恢复 | 12 pass；`console-transaction-runtime-contract-v2.log` |
| Local 无 Redis/重启恢复 | 7 pass；`console-local-runtime-contract-v2.log` |
| Redis key/namespace 合同 | 4 pass；`console-transaction-contract-v2.log` |
| 正常管理保存 | 12 pass；`console-management-contract-v2.log` |
| 自动发现、配置提交失败相邻 | 7 pass；`background-discovery-contract-v2.log` |
| 实际 Redis Lua，显式 `--ignored` | 9 pass / 0 fail / 0 ignore；`real-redis-v2.log`，不是缺环境变量的跳过 |
| 合并计数 | 231 次执行、220 个不同测试名；重复仅为栅栏 11 项在 Console 套件中再执行；`test-result-audit-v2.json` |
| 代表性 feature 关闭编译 | `--no-default-features --features line-gemini-web-reverse --lib` 通过；`feature-disabled-v2.log`，不是完整 feature 矩阵 |
| 静态 | Rustfmt、checker 31/31、ratchet 4088 文件/14 immutable/governed source >700=0、tracked `git diff --check` 通过 |
| 冻结与文本审计 | 21 项源码 hash 匹配、24 份 UTF-8 无 BOM 文本、112 个本地链接、围栏及含未跟踪源码的尾随空白检查通过；`source-doc-audit-v2.json` |

后端使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，未运行默认 management-ui 的发布/清理 hook。测试进程沿用 16MiB `RUST_MIN_STACK`，未改生产线程设置；TEMP/TMP/TMPDIR 明确位于本证据目录的 `runtime-temp`。

真实 Lua 验证使用本地已存在的 `redis:7-alpine` image ID，`--pull=never`、随机 loopback 端口、128MiB 内存、`/data` tmpfs、无用户目录或卷挂载。每项使用随机 namespace；没有 FLUSHDB/FLUSHALL、历史凭据、现有 Redis URL 或默认 legacy mirror 写入。覆盖 bootstrap 与普通 old-active CAS、新/旧脚本的迟到 activation、同 R 后继、永久 tombstone、旧非 canonical JSON、坏 gate、坏 immutable payload 及已有其他 namespace 不变。

`disposable-redis.json` 和 `disposable-redis-completion.json` 记录实际容器身份/隔离与清理；仅在核对精确 ID、purpose label、无卷及 tmpfs 后停止本次实例，auto-remove 后查询剩余容器为 0。未启动 Gateway 或接管在线服务。

## 4. 规模、安全与兼容边界

生产有效行：runtime 383→402、activation 128→120、recovery 235→176、local backend 176→194、Redis store 461→465、types 163→168、records 325→343。新 Redis/runtime fence 分别 93/89；新身份测试 120、owner 测试 232、unsupported 测试 55；迟到 actor fixture 309，真实 Redis 主测试/支持分别 322/218，均为明确单一测试职责。既有 HTTP fixture 460→494；未跨 500，无 baseline/豁免或依赖变更。初始测量脚本遗漏 commit 和部分后加测试入口，保留该测量边界，不把后补统计冒充前置记录；最终 21 项冻结覆盖它们。

新路径不引入秘密出口、任意 Redis key、任意出站目标或生产后台任务。Lua 只访问 store 自有 namespace 的 active/transaction key；永久 gate 是既有 transaction key 的状态推进，不维护第二份配置真源。GET→Lua gate CAS 竞争只保守失败，不另加无限重试；刷新仍受 P29 既有 attempt 预算约束。同步磁盘不是可抢占硬实时 IO。

旧脚本遇到 tombstone 后不能再写 active；但 active 已是同 R 时，旧脚本仍可能只返回 AlreadyActive，旧程序自己的本地 journal/响应归属不由新代码修复。本轮测试明确保留这个区别，不宣称旧新二进制完整混跑已验收。

## 5. 未完成与下一步

本批组合证明正常 owner 的迟到命令收敛路径及真实 Redis Lua 顺序合同；不是启动完整 Server 进程、网络代理中的所有取消组合、强制崩溃或整个 scheduler shutdown/join 验收。

P29 继续 `in_progress`：只读 Worker 的 refresh-only 持久能力、发送 grant 前的跨实例防重发、结果不明/强制退出的 token 处置，以及 authority 已提交后崩溃的派生 cache 修复仍缺。不得开放 Worker 的通用 automation 写权限或用 timeout/普通 lease 宣称一次性 token 可安全重发。

后续继续上述生命周期，再接 Grok 正常管理登录/授权结果保存；Accio/Kiro/Vertex、Perplexity、新 API 分支、Qwen/Bedrock/媒体剩余能力、P50 集成包、P60 集中录制和 P70 真实验收全部保留。前端仍为本计划的 9 个新增模板；本轮未新增模板、读取历史凭据、调用实际上游、提交/推送或出包，不标整卡 offline_ready 或总目标完成。
