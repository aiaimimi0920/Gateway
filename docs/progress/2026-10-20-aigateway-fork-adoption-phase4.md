# AIGateway fork 能力采纳：第四批

日期：2026-10-20（Asia/Shanghai）。用户授权继续完成有收益的采纳项。本批落实 E02 的精确定向模型健康恢复，使用 Gateway 现有路由、生成探测和 SQLite / PostgreSQL 健康 owner 重新实现，不移植 CPA 的全池 ResetQuota。

## 已实施

- 修正生成探测与普通路由读取的健康身份不一致：统一 provider、真实 credential ref、有效 protocol profile 与映射后的 upstream model；包括无凭据池默认账号和 discovery 覆盖。
- 生成前保存内部 expected-state，结束时成功和失败都经过原子健康行 CAS。迟到成功不能清掉期间新增的失败，迟到普通失败也不能降级期间新增的硬封禁。
- Local 复用原有 `BEGIN IMMEDIATE` writer，比较完整 payload，所有 owner 写入使时间戳严格递增；兼容旧字段和原 `user_version=1`，不新增状态库或迁移。时钟回拨更保守，极端不可推进时间戳失败关闭、不 panic。
- PostgreSQL 复用同一事务内的状态转换 writer，锁行后重验短期 `xmin:ctid`；初始缺行时不覆盖冲突期间出现的记录。没有把物理 token 变成永久业务版本。
- 写前复核 revision 和完整内存 payload binding，同 revision 的 OAuth runtime refresh 也会使旧目标失效。写后再验，发现配置变化返回独立不确定性回执，不做旧快照补偿回写。
- 生成探测不再追加 credential/account-wide 无条件写入。不解除硬封禁、disabled、全局 breaker、账号配额或错误预算，也不恢复其他 model/profile/credential。
- API 新增可选 `healthRecovery`；测试计划按 case 保存，不伪造整轮 reset。结果页独立显示调用结果与健康回执，Passed 也会显示 stale / protected / 存储未确认。旧结果缺失字段继续兼容。

完整范围、wire 与真实边界见 [模型生成探测与定向健康恢复](../model-health-recovery.md)。这是既有生成测试路径的增强，不新增公开 reset 写接口或后台真实供应商调用。

## 工程与保全

开始快照保存 118 项 dirty 内容和 1766 个 tracked Rust 文件的 hash / 有效行数。本批相对开始快照有 35 个源码 / 测试文件变化，最大 424 有效行；没有新增超限文件、修改 effective-line baseline 或接管 S06。最终源码及功能文档 hash 与冻结快照一致，118 项原有 dirty 内容未变。

原始 16 项现场另行保全；参考 fork 的 219 个源码文件、HEAD、branch、index 均与审计快照一致。未 fetch 或整理参考仓库，没有读取 live credentials、改 `.ng` owner / storage version、端口 8317、计划任务、部署实例或其他 Neuro 项目。

已检查鉴权 / secret 边界、精确身份、SQL 参数绑定、并发覆盖、存储失败、事务生命周期及取消清理。健康快照不接受客户端输入，不序列化回 API；秘密 payload 仅内存比较，不记录公开摘要。数据库事务不跨上游生成调用持有。

## 验证回执

最初 fixture 路径错误使请求没有进入合成端点，已修正并加入有界等待；保留该日志。修正后的红测试为三项 `healthRecovery` 缺失断言。实现后第一轮 8 项后端聚焦测试通过，随后补入测试计划 case 回执及额外并发 / 时间 / binding 边界，纳入以下最终全量回归。

| 检查 | 本轮实际结果 |
| --- | --- |
| Rust 全量 `cargo test --locked --offline -- --test-threads=1` | 98 suites；4072 passed / 0 failed / 104 ignored |
| `cargo check --locked --offline --all-targets` | exit 0；仍有 unused / dead-code 等 warnings |
| 受影响后端合同（5 suites） | 31 passed / 0 failed / 1 ignored；最终全量再次覆盖 |
| 新生成恢复合同 | 9 passed；完整覆盖精确映射、迟到故障、初始缺行、硬封禁、存储失败及 per-case 回执 |
| 新 Local / 配置边界 | 并行恢复 CAS、ABA-shaped 转换、时钟回拨、极端时间戳、discovery profile、同 revision OAuth refresh 均通过 |
| Desktop TypeScript | exit 0 |
| 本批相关前端测试 | 4 files / 19 passed |
| Desktop 全量 `src`，`--maxWorkers=1` | 695 passed / 12 failed；不是全绿 |
| 与上一批已验证基线比较 | 12 个失败名称完全相同，无新增失败名称；本批未重跑隔离基线副本 |
| Node scripts | 2122 passed / 0 failed / 2 skipped |
| Python | 358 passed / 0 failed / 4 skipped（总 362） |
| line manifests | 44 checked / 0 errors |
| checker tests / ratchet | 31 passed；ratchet exit 0，未运行 strict |
| formatter / final scoped diff check | exit 0 |
| 生产 Web entry | 使用现有 Rsbuild 配置编译通过；独立产物仅写入 `linshi` |

Rust 使用默认线程栈，清除 `RUST_MIN_STACK`，Cargo 检查串行执行。初期红测试命令触发了 `build.rs` 的 Web 构建；之后使用既有 ready marker 和 `GATEWAY_PREBUILT_WEB_UI=1`，独立生产 Web 编译关闭 staged publish 插件、禁用输出清空，未作为 EXE 打包结果。

Node 首次运行因临时目录的正反斜杠字符串断言失败；只规范化测试进程 TEMP / TMP 后全量重跑通过，没有修改该测试或产品代码规避断言。首次格式检查 exit 0、stdout 为空，临时 PowerShell 日志包装器误将空输出当 null 数组；已修复包装器并串行重跑 all-targets / formatter，保留首次记录。前端旧失败未过滤，没有放宽 timeout 或修改既有 12 项断言。

## 未验证与未扩张的边界

- PostgreSQL 新增受控 opt-in 合同，涵盖共享 CAS 反例及真实锁等待后的版本重验，但本轮没有启动独立 PG fixture。默认 ignored 不计通过，不借用业务数据库。
- 配置发布与 SQL 属于不同 owner；写后复核不是跨 store / 跨实例原子事务。absence ABA 不保存历史 tombstone，普通业务请求和旧非生成 connectivity probe 的完成顺序合同没有被本批整体改写。
- 未运行真实供应商、Server Redis live、44 条独立 feature-line 矩阵、真实浏览器交互、native EXE、发行或部署验收。因此不能声称整个仓库门禁全绿或可直接发布。

E01 人工维护闭环和本批 E02 已有各自源码与本地验证回执。完整 D01 reasoning 历史往返、A04 显式 OAuth-first、B01/B02 共享预算 / 单次恢复许可、B03/B04/B05 外部健康 advisory 仍是需要独立合同的候选，不冒充完成。不采纳的无限恢复、指纹鉴真、请求外 ticket ping、DOM 补丁、周期重启等结论不变。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-fork-adoption-phase4-20261020/`。包含 before / frozen scope、红绿测试日志、Rust 全量与 all-targets、静态检查、前端失败名称比较、hash manifest、保全结果及 `verification-summary.json`。

本批未 commit、push、打包 EXE 或部署；Gateway HEAD 仍为 `08634912809fea48b45fd888936cd688356fdfac`。
