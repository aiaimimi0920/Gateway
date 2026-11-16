# FreeBuff：原 run、费用观测与 FINISH 事实持久化

日期：2026-11-08（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。
承接[公共付费阶段协作取消](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-cancel.md)。大量继承 dirty/untracked 实现保留，HEAD 不是本批来源凭据。

状态：本子批实现、定向验证和收尾审计已完成；FreeBuff 整卡和服务商扩展总目标仍在执行中，不标记 `offline_ready`。

## 1. 本批实现与实际入口

原公共 Local 非流式 Chat 的逐次批准 → admission → `ActiveAdmission::start_catalog_run` → 原 `CatalogRun` 现在必须使用 Local run recorder。生产没有无 journal 的 START 入口；原分段/无 store wire API 仅保留为 cfg(test) 夹具。

- 在原 SQLite schema-1 中 additive 创建 `local_freebuff_runs`，不升版本、不重写旧 claim、历史、凭据或 seal。原安装 seal 继续拥有新表；仅剩 run 行且 seal 缺失时也不生成替代密钥。
- 每个原 binding/attempt 仅一条密封 run 记录；原 claim sealed preimage、旧 run sealed preimage 与 active claim 存在性由一条 SQL 原子 CAS 同时检查。首次 INSERT 不 UPSERT，旧 run/未知 START 即使重新 heartbeat 也不能再 START。
- 保留真实 run ID、原 client ID、当前阶段、可确认累计值、最近一步费用观测、此前已确认累计前缀、固定安全错误代码及 FINISH 接受事实。运行身份加密保存，不返回 UI，不持久化 prompt、completion body、Authorization 或设备能力。
- START intent 必须先落库；真实 START ID 必须先保存，才允许模型调用。模型发送前保存结果/费用未知；reported cost 变化才写库，不逐 token/chunk 写库。当前未知不抹去此前已确认累计前缀，也不会把其当作完整总费用。
- FINISH intent 必须先落库；HTTP 接受且 acknowledgement 已返回、再成功持久化，才显示 `finish_accepted`。持久化写入被取消或失败后，原对象锁存 unknown，不再调用 recorder 或发送后续 FINISH。
- SQLite 等待不延长 active observation：START intent、模型 intent 保存后和 test-only split send 都重新核对原 observation；新 send 仍受原 deadline/cancellation 约束。FINISH 仍可为过期 observation 收尾，不变成另一份生成许可。
- 新 journal 存储失败按本地 503 返回，原 claim 不清除、无 retry/rebuy，公共分支对原模型 circuit 使用 neutral，不把本地 store 失败归成账号失败。

`RunRecord` 的累计与最近观测是 O(1) 恢复事实，不是逐步骤完整 trace、持久 conversation/history、完整工具/child 费用账本或远端 settlement receipt。

## 2. 正常管理与页面

复用原 saved-account 管理 read、SecretGrant/revision fence、同一 SQLite read snapshot、每页20条历史和 no-store。当前 claim 与其归档历史均能读取对应原 run 事实；不取执行 guard、不登记设备、不请求上游。

正常账号面板同步严格 Zod schema，兼容旧响应缺少 run。只增加短状态和真实数字：START/模型/FINISH 阶段、reported SDK credits，未知显示 `—`；当前总量未知但此前累计已知时单列此前值。`FINISH 已接受（未结算）` 不当作释放、退款或推理完全成功。没有新增续跑、重放、后台补偿或闲置管理购买按钮。

SDK credits 来自合成 supplier 的 reported dollar components，按原 SDK 每模型 callback round 后累计；不是 Freebucks 小时报价/退款、实际平台账单或 Gateway Key 次数。

## 3. 验证与真实失败

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-run-journal-20261108`。
所有 Cargo 使用 locked/offline、runtime-defaults、关闭 management-ui、jobs4；FreeBuff 测试 threads1，临时状态在本批 linshi/tmp。

| 检查 | 当前结果 |
| --- | --- |
| 首个真实公共 run/journal 行为红测 | 旧实现返回 run Null，预期 finish_accepted，准确失败 |
| 新公共/原 owner 第一轮定向 | 6 passed；是中间版本，不与最终数量相加 |
| 最终 FreeBuff 相邻 | 462 passed / 0 failed / 3 ignored；以 `freebuff-verified` 为准 |
| LocalRuntime 全部 lib 选择 | 52 passed / 0 failed；与上一行按完整名去重为 502 passed / 0 failed / 3 ignored，重叠12项 |
| 本批新增 Rust / 前端 | 12项 Rust 全通过，已包含在上面；前端9项新增已包含在209项中，不另相加 |
| 最新前端10文件与类型检查 | 209 passed；typecheck passed；`ui-final` / `typecheck-final` |
| 正常生产 / 代表性仅关闭 FreeBuff 的生产 lib | 两者 `cargo check --lib` 均通过；不冒充关线单测或完整 feature 矩阵 |
| formatter / checker tests | 最终24份 Rust 官方格式通过；checker 31 passed |
| 最终 ratchet | 4953文件，通过；14份明确分类的 immutable runtime assets 单列，governed source >700 为0 |
| 最终来源、编码、范围和 diff | 通过；见 `final-audit.json`、`scoped-files.json`、`scoped.diff`、`effective-lines.json` 与 `final-diff-check.json` |

新测试覆盖：正常公共调用与原 end 后历史、START/model headers/费用已观察/FINISH 在途的真实任务 abort 与同目录 runtime 重开、未知 START/缺费用/拒绝 FINISH 的区别、store preimage 改变后的下一次 wire 拒绝、heartbeat 后不再 START、逐步 round 累计、已知前缀在下一步未知/Drop 后保留、真实 SQLite 写锁等待导致 observation 到期、取消 store future 的 unknown 锁存、原 ID/seal 保留、AAD/篡改拒绝，以及旧 schema-1 additive 初始化。

失败保留：前端旧 strict schema 无法显示新事实的红测；首次临时 runner 不能直接 CreateProcess `npm`，报 `[WinError 2]`，改显式 `npm.cmd`，不是改测试或安装依赖。静态交叉核验发现新增 SQLite await 后缺 active 再检查，已补三个发送接点与真实写锁测试。中间名为 `freebuff-final` 的检查执行了461 passed/3 ignored，但其输入在执行期间因“未知下一步保留此前累计”修正而变化，明确作废为最终来源，不拿该结果代替最终源码的重跑。

继承的 `tests/console_provider_toggle_health_contract.rs:23` / E0560 未由本批修改，也未重跑全目标，不声称全仓绿。没有升级依赖、修改旧有效行 baseline/exceptions 或借未运行的浏览器检查凑通过数。

### 3.1 最终来源与资源审计

本批以直接保存的 preimage 比较，最终范围为24份 Rust、5份前端源码/测试和5份文档，共34份文本；10份源码文件为新增。所有范围文本均为 UTF-8 无 BOM，最终 `git diff --check` 通过。语言感知 checker 计数的最大范围文件为 `business.rs` 278→280 有效行，最大新增为 `claim_owner_tests/run_journal.rs` 195行；没有新增例外或改变基线。

最终 FreeBuff、LocalRuntime、正常/关线生产 lib、formatter、ratchet 六阶段均前后输入一致，且各4418项选定输入与最终快照逐项相同。前端最终测试和类型检查之后只有 Rust 的 `execution/journal.rs` 保留已知累计前缀修正；前端实际消费的全部已选文件/配置仍同 hash，不把两份较早的全量快照声称为最终 Rust 来源。`terminal-tests.json` 记录同一 lib harness 的完整名去重；`lib-harness.json` 的实际二进制 SHA-256/bytes 在收尾时仍匹配。

原 legacy 公共 guard、行数 baseline/exceptions 保持本批原字节；根 `artifacts/effective-code-lines.json` 已从本批直接 preimage 精确恢复，最终实际 ratchet 报告另存为 `final-actual-ratchet-report.json`，恢复见 `final-report-restore.json`。没有以重新生成 baseline 或覆盖原始失败记录取得通过。

以上是选定输入的一致性凭据，不是外部 path dependencies、node_modules、运行期资产和宿主环境的 hermetic 保证。最后只读资源检查为本批精确 lib harness 0个活进程；未终止历史 PID、清理失败现场、启动浏览器/helper/Redis/Docker 或改动已有运行实例。

## 4. 本批不承诺与下一棒

这里完成的是原 run/费用观测/FINISH 事实的持久化和正常只读恢复，不是断点续跑能力。真正任务 Drop/进程退出不自动 FINISH；未知 START/FINISH 不重投，不自动 end、重买、renew、takeover 或后台补偿。确认账务/重放安全需要已证的远端恢复协议及新的正常管理授权，不能凭一条记录或 boolean 自行补请求。

同目录 runtime 重开及任务 abort 不冒称实际进程 kill/restart、跨进程竞争、EXE/整包或真实平台验收。本批页面只做组件/严格 DTO/类型验证，没有重新构建 main 网页或执行新字段的真实浏览器联合，上一批 UI 联合仍是上一批证据。

后续仍需 START/FINISH 未知的显式对账与安全恢复、完整 run/远端 lease、实际工具/child/持久 history、精确 probe、新设备/目录应用 UI 联合、其余公共协议/实时 SSE/模态/参数、Server/PG/Hosted、独立进程与原生包，以及真实平台资格/模型/额度。
全局模板仍17，无新增整卡 `offline_ready`；P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道真实验收和总目标未完。

本批没有真实凭据/供应商、Redis/Docker、既有实例、出包、提交或推送操作；不复用已消费的 P34 Redis 授权，不重放或清理上一 UI 批失败现场。
