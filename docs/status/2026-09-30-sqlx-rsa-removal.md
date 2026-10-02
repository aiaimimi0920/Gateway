# SQLx/RSA 依赖移除，2026-09-30

状态：**根 Gateway 的 SQLx owning-parent 依赖移除已完成本地验证；RSA 已从根锁图
删除。整体依赖安全仍未完成，不发布新的“安全通过”候选。**本记录接续
[HTTP 迁移基线](2026-09-30-http-client-migration-baseline.md)，不接管 S06 Rust/Gemini。

## 实际采用的最小方案

新增内部 `crates/gateway-sqlx`，根 dependency 保留 `sqlx` alias。该 crate 的生产
代码只有 **30 个有效行，全部是官方同版本类型和宏的重导出**：

- `sqlx-core`、`sqlx-postgres`、`sqlx-sqlite`、`sqlx-macros` 均固定 `=0.8.6`；
- runtime 保持 Tokio、rustls ring + webpki roots、JSON/time、bundled SQLite；
- 宏仅启用 `derive`，仍使用官方 `FromRow`，没有自制 derive、ORM 或 vendored fork；
- 普通 query/pool/Row/Transaction/QueryBuilder 和错误类型保持官方原类型身份；
- 66 个原 `FromRow` 模型、28 个业务文件均未改写，SQL/schema/真实数据库未修改；
- 原 local runtime owner 的 WAL / Full synchronous 保持不变。

此前“仅关闭 SQLx default features”的失败结果仍有效，但不能据此推断所有官方
宏组合都必须包含 RSA。本批新隔离证据证明：**直接组合官方 runtime，并仅启用
官方宏的 derive feature，可以保留当前行映射而排除 MySQL/RSA**。原 umbrella
参考锁图仍包含它们，候选不包含；没有照先前候选方向制作本地行映射宏。

这不是完整 umbrella API 的等价替代。当前模型不使用 MySQL/Any、编译期 query
宏、迁移/test 宏或 Type derive。已额外验证 rename/default/raw identifier；
`FromRow` 的 try_from 分支需要未提供的 `__spec_error!` helper，其他未测属性不作
支持承诺。新增需求和同族版本升级必须由此 owner 评审，详见
[内部 crate 说明](../../crates/gateway-sqlx/README.md)。

## 锁图、构建与回滚边界

Cargo 使用当前已验证锁文件作版本 seed，通过官方 offline metadata resolution
更新根锁图，未手工编辑 package/checksum。相对本批操作前：

- 仅新增内部 `gateway-sqlx 0.1.0`；
- 删除 7 个不再需要的 package：`sqlx`、`sqlx-mysql`、`rsa`、`pkcs1`、
  `num-bigint-dig`、`num-iter`、`libm`；
- 保留的注册表 package 版本/source/checksum 不变，必要的 dependency adjacency
  由 Cargo 随 feature 图重新计算；没有顺手重新解析或升级其他注册表包。

根 `Cargo.lock` 是新内部 crate 的交付依赖 owner，不创建第六个独立 lock。
Dependabot 和覆盖合同纳入其 manifest；正式 Docker 的现有 `COPY crates ./crates`
已经覆盖，开发 watcher 增加该 crate 路径。桌面/local-data 仍是独立构建图，
它们的 manifest/lock 保持原 hash，没有为了根迁移顺手改写。

不需要数据迁移。回滚可恢复本批操作前的根 manifest/lock，不修改数据库或旧包；
操作前字节/hash、锁图 delta、原有缓存和源码保护核对保存在本批证据中。

## 官方对照与实际验证

独立参考程序使用原官方 SQLx umbrella；独立候选使用直接 runtime + 官方 derive。
两边运行同一份 fixture，各通过 5 项测试。6 组结果/错误快照完全一致：缺失的
Option 列、类型不匹配、非 Option String 的 SQL NULL、无效 JSON、无效 time、
多个字段错误时的首个错误。正向映射覆盖当前字段类型、重排/额外列和 nullable 字段。

官方 SQLite String decoder 将 SQL NULL 解码为空字符串；初版参考测试错误地预期
异常，实际运行纠正了这个假设。只改测试，未改官方驱动或放宽候选行为。缺失的
Option 列仍返回 ColumnNotFound，不吞成 None。首轮失败回执保留。

`tests/fixtures/sqlx_error_snapshot.json` 保存独立官方参考结果。根正常回归直接
比较 kind、column/index、source 和 display，不从候选本次输出动态刷新期望。

| 检查 | 本批实际结果 |
| --- | --- |
| 独立官方参考 / 独立候选 | 各 5/5；6 组快照一致 |
| 根 `sqlx_runtime_contract`，含固定官方快照 | 5/5 |
| 根 Windows `cargo check --offline --locked --all-targets` | 通过；最终源码复验通过 |
| `local_storage_pipeline_contract` | 3/3 |
| `access_balance_contract` | SQLite 2/2；PostgreSQL 实库用例 1 项 ignored |
| SQLx dependency/owner/features/锁图 Python 合同 | 4/4；更新前锁图防回归明确失败 |
| 相邻 Docker 开发/依赖入口 Python 合同 | 23/23 |
| 安全/发布合同 | 15/15 |
| 行数 checker 测试 | 31/31 |
| root formatter / adapter rustfmt | 通过 |
| ratchet / strict | 通过；2609 文件、14 个单列 immutable runtime assets，受治理源码 `>700=0` |
| `git diff --check` | 通过 |

新 Rust 测试、Python 合同有效行分别为 **216**、**49**；生产适配层 **30**。
watcher 从 173 增至 174；安全合同保持 81。没有新增软例外或修改 checker 策略。
临时 runner 首次将绝对路径传给 checker 的 `--json`，被其路径边界拒绝；改为不写
统计文件的原门禁调用后通过。没有放宽 checker，也没有覆盖原有 dirty 统计缓存。

PostgreSQL 只验证当前全部消费者编译与 PgRow 类型/trait 契约，**未运行真实数据库
业务验收**。Windows all-targets 不是跨平台编译；没有运行 Linux/Docker/原生 UI
或新 package 验收。仍设置 `GATEWAY_PREBUILT_WEB_UI=1`，不冒充新前端 release 构建。

## 安全审计仍未通过

本批对三份独立 Cargo lock 执行 `cargo audit --no-fetch --deny warnings --json`，
使用保存且无 dirty 的 RustSec 数据库：

`9b3a3b73a7f42606494c943e95f8196e9994df46`

没有新鲜联网数据库，没有 ignore，没有关闭 yanked 检查。`--deny warnings` 明确
将 unsound/yanked 等告警也作为失败，不能以默认 warning-only 退出行为伪造通过。

| 锁图 | 退出码 | vulnerability | unsound | yanked |
| --- | ---: | ---: | ---: | ---: |
| 根 Cargo.lock | 1 | 0 | 2 | 1 |
| 桌面 Cargo.lock | 1 | 0 | 1 | 0 |
| local-data Cargo.lock | 0 | 0 | 0 | 0 |

- 根 `rsa / RUSTSEC-2023-0071` 已消失；仍有 `lru 0.13.0 <- rquest 5.1.0` 的
  `RUSTSEC-2026-0002`、`RUSTSEC-2026-0253`，以及 rquest 的 yanked 告警。
- 本批补充的严格独立桌面扫描发现既有 `glib 0.18.5 / RUSTSEC-2024-0429`。
  桌面锁文件未变，该问题不是根 SQLx 迁移引入；没有据此修改桌面或声称跨平台不可达。
- local-data 只有其当前保存数据库下的锁图扫描通过，不是整个 Gateway 已安全。

HTTP 候选源码/工具链仍无法经此前请求正常取得；本批最初的网络复查仍为 TLS
发送失败，之后没有重复同请求，没有关闭证书验证、换未知镜像或改全局代理。
下一步是独立处理 HTTP owning-parent 与桌面 GLib finding，保持正式发布门禁。
真实 GitHub OSV/Gitleaks/CodeQL、S06/S18、跨平台与新不可变 release 验收仍未完成。

本批没有 commit、push、Docker 重启、凭证/.ng/真实数据库或已有 release 变更。
其他独立仓库的既有修改保留。可复核命令、完整 stdout/stderr、失败与最终回执、
官方对照、锁图/feature 证明、源码与缓存保护核对及 completion：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-rsa-probe-20260930-111159`
