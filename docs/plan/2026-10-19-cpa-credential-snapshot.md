# CPA 对照增量：凭据导入的延迟快照提交保护

## 固定参考与范围

2026-10-19 再次联网核实 CPA 最新正式版本仍为
[v8.0.33](https://github.com/router-for-me/CLIProxyAPI/releases/tag/v8.0.33)。
参考其 [c4c1d43](https://github.com/router-for-me/CLIProxyAPI/commit/c4c1d43)
防止延迟文件快照覆盖新状态的行为目标，不复制 Go 实现。
上批 Responses 完整性工作见 [特性对照](2026-10-19-cpa-feature-adoption.md)。

Gateway 已有 routes revision/CAS、OAuth refresh generation 和 folder-sync run admission；
不重复实现这些保护。本批只处理服务器 PostgreSQL 的 folder-sync **既有凭据更新**：
本轮 metadata 快照读取之后，若该行被其他事务更新或删除，迟到导入不得覆盖它，
且必须在对象存储 PUT/DELETE 前拒绝。

不改本地 SQLite、存储版本、数据库 schema、S06/Gemini、私密配置、依赖或重试授权。
不提交、推送、部署、替换已有发行包，不使用真实供应商或业务存储。

## 实现合同

1. metadata 仅增加本轮运行内使用的 `xmin::text` 不透明 equality token，不读取旧 payload。
   它不是永久业务 revision，不持久化、不比较大小。保持既有 metadata/path/row 上限。
2. 条件更新在单一 PostgreSQL transaction 内先按 ID + token `SELECT FOR UPDATE`，
   等锁后重新匹配条件；不匹配返回固定脱敏 `provider_credential_snapshot_conflict`。
3. 匹配并取得 row lock 后才执行既有 payload persistence，随后 UPDATE 和 commit。
   普通更新共用这一 owner，但没有 expected token 时保持现有字段及 not-found 语义。
4. importer 只把专门的 snapshot conflict 计为 skipped；不吞掉数据库、输入或对象存储错误，
   不拿同一旧 payload 自动重试。创建新凭据和已有 hash 相同的 skip 行为不变。
5. 无新增任务、无界缓存或重试。transaction 的失败/取消沿 SQLx 释放/回滚机制处理。
   对象存储网络操作沿既有 30 秒 deadline；锁内 I/O 会延长该行及连接的占用。

PostgreSQL 的 [system columns](https://www.postgresql.org/docs/18/ddl-system-columns.html)
与 [READ COMMITTED 锁等待后的条件重查](https://www.postgresql.org/docs/18/transaction-iso.html)
是本设计的数据库合同；用真实、隔离的合成 PostgreSQL 测试验证，不用 mock SQL 代替。

## 明确未覆盖的边界

- 本批防止**本轮已读快照**的迟到提交，不建立文件和数据库的永久权威排序。
  Import-only 下一轮重新读取当前 metadata 后，同一磁盘文件仍可能作为一次新导入被接受；
  不新增持久冲突档案、不自动丢弃用户文件。Both 的后续 export 仍遵循已有语义。
- 不解决 export 的旧 DB 快照、Redis GET/SET refresh、keepalive 请求前身份验证等独立窗口。
- PG rollback 不能回滚已成功的对象存储 PUT/DELETE；未加锁读者与既有 delete owner
  的对象副作用也未在此批全面事务化。不能声称所有对象存储并发或崩溃一致性已闭环。
- `xmin` 因 health/status 更新也变化，保守跳过不等于一定发生 token 轮换。
  长期 XID 回绕、整轮长事务或改变隔离级别需要重新审查，不能把此 token 存到客户端。

## Claude 冷却核查

已只读对照 [db1bab0](https://github.com/router-for-me/CLIProxyAPI/commit/db1bab011d98a587522ab199fb1910c52155436f)。
CPA 修复的是特定 per-minute 错误在无 quota-status 时被无分类 unified reset 拉长。
Gateway 当前只解析 Retry-After 系列，不读 Anthropic unified reset，所以不能把同一放大
问题登记为本地已复现缺陷。跨请求冷却仍以 route policy breaker 为主；若增加供应商
配额窗口，需要另行确定 credential/account/model scope 及 typed error 传递，不全局压低长期冷却。

## 验证状态

本批源码、独立只读实现复核和本地聚焦回归完成；不是所有凭据竞争窗口、全部 CPA 能力
或 Gateway 发布验收完成。新增条件 API 在 red 阶段先委托既有无条件更新路径：
7 项合同中 5 项实际失败、2 项通过，复现 payload/label 覆盖、等锁后的旧值提交，
以及旧快照对新对象的覆盖和删除。随后才实现条件提交，未删除失败断言。

| 最终检查 | 结果 | 证据 |
| --- | --- | --- |
| 新增快照合同 + 既有实际 folder-sync 数据库合同 | 7 + 14 passed | green-final.log |
| 文件删除/路径安全合同 | 9 passed | deletion-files.log |
| folder-sync 单元默认回归 | 141 passed，14 ignored | folder-sync-unit.log |
| 上述 14 项 guarded 单元的显式执行 | 14 passed，0 ignored | ignored-unit.log |
| provider credential 单元 | 5 passed | credential-unit.log |
| folder-sync runtime 启停/取消/watcher 合同 | 23 passed | runtime.log |
| 有效行 checker 测试 | 31 passed | checker-tests-final.log |
| ratchet / strict | passed，governed source >700 = 0 | ratchet-final.log / strict-final.log |
| cargo fmt / git diff --check | passed | format-final.log / diff-check-final.log |
| line manifest validator | 44 checked，0 errors | line-manifests.log |

相关 Rust 测试合计 **213 项通过**；默认忽略的 14 项已单独运行，不把 filtered/ignored
算成通过。全部 Rust 命令使用 `--locked --offline` 和 `--test-threads=1`，构建串行。
这不是整个仓库所有测试、完整 feature 矩阵、正式包验收或真实供应商测试。

| 权威 effective lines | 修改前 | 修改后 |
| --- | ---: | ---: |
| mutations.rs | 264 | 148 |
| snapshot_update.rs | 新增 | 154 |
| import.rs | 151 | 161 |
| provider_credential_snapshot_contract.rs | 新增 | 231 |

条件更新 owner 与测试均在 250 行推荐范围内，没有新增软例外或改 adoption baseline。
模块逐项审查了输入/secret、参数化 SQL、连接与 transaction 生命周期、对象副作用顺序、
内存上限和取消；未新增任务、依赖、重试授权或数据库迁移。锁内外部 I/O 及对象存储
非事务性边界保持上文所述，不以本轮测试夸大为全面崩溃一致性证明。

使用仅绑定 loopback 的独立 guarded PostgreSQL/Redis 测试容器及 linshi 合成对象目录，
没有连接已有业务数据库；测试 schema 清理后计数为 0，测试容器由本轮 owner 核对 ID/
run label 后停止清理。开始时记录的 28 个已有 dirty/untracked 文件最终 hash 不变，
包括上批 Responses、私密配置和原有 checker dirty 报告；所有本批文本为 UTF-8 无 BOM。

没有提交、推送、真实供应商调用、正式发行打包、运行服务替换、完整发布门禁或
Linux/Docker 产品验收。调试编译与测试可执行文件已生成，但本批修改没有自动进入
先前交付的 Windows 发行包。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-snapshot-20261019/`。
`final-verification.json` 记录本批文件 hash、原有文件保护及检查结果；
`fixture-provenance.json` 与 `fixture-cleanup.log` 记录隔离实例和清理。
