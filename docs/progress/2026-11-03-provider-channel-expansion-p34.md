# 服务商分渠道扩展：P34 刷新发布后的重启 cache 修复

> 2026-11-03；本批实现与下述定向验证已通过，不表示 R05、P32 或总计划完成。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留既有脏改，不提交、推送、出包或替换运行实例。

## 1. 实际关闭的窗口

[P33](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p33.md) 已接持久消费证据，但进程内 pending 丢失后，新的 authority 材料不能反向找到旧 Result/Published，fresh token 也不会触发刷新。本批实现：Result 已持久化、正常 refresh 事务已激活 authority 后，即使旧 pending 丢失，新 store/runtime 仍能凭明确 publication 来源修复旧 cache，不再交换 grant。

实际 scheduler 在 fresh token 无需刷新时也先执行恢复；恢复故障会继续保留义务并暂缓新 exchange，不创建额外 authority revision。分别测试 Published 后丢失 pending，以及 activation 后、Published 前丢失 pending。只读 Worker 复用私有 refresh owner，没有获得普通管理或 automation mutation 权限。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p34`。

## 2. 来源证明、并发与兼容

- 复用 RefreshIntent、完整 Prepared identity、parent/new archive、有限 refresh projection 和 P30 settlement；publication 候选在 journal Prepared 前、同一 writer guard 内保存，不等到 Published 才建立 locator。
- revision ID 不含 tx UUID；按当前 revision 和 tx ID 分别保存不可变候选。恢复只查当前 revision 目录，核验本地真实事务、原 intent、父/子 archive、精确 grant identity 和远端实际 activation。孤儿和 Aborted 不被计为发布者，同 revision 的合法新 tx 重试不与旧候选互相覆盖。
- Redis 最终写入原子比较五项：active revision ID、真实 immutable revision raw、原 grant raw、真实 Activated transaction raw、原 cache raw。cache 竞争可有界重读，authority/grant/transaction 变化则拒绝写入。
- cache 必须具有真实 CredentialEntry schema、正确 credential ID、base URL、adapter、原 endpoint/client/RT/context 和原绝对 expiry；显式 call flow 也须相符。Grok Build 另要求专用 preset，不能因 xAI API 同为 OpenAI-compatible 就认定同一产品。未知归属、独立重授权或同 token 的独立续期不修写；缺失/过期 cache 不重建。
- endpoint/client/payload expiry 的非字符串值被拒绝；合法 null 保持原 Option 缺省语义。expiry 比较沿用同一绝对时刻，不要求 RFC3339 小数位拼写相同。
- quota、未知 cache 字段、成功结果保存时的绝对 expiry 和由该时刻计算的 TTL 被保留，不给恢复的 token 再授完整 lifetime。
- 新增来源记录是可选附加数据，locator 不复制秘密材料。旧 journal、RefreshIntent、GrantRecord、revision 和全局 storage version 不改格式，不改写、迁移或删除旧数据。旧发布没有新 locator 时不自动追认；本批不证明旧新二进制混跑。

主要 owner：

- [publication 存储](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/persistence/refresh_publication.rs)
- [当前 authority 的恢复](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/refresh_cache.rs)
- [五重 Redis CAS](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/redis_store/refresh_cache.rs)
- [cache 目标归属](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/cache_target.rs)

## 3. 红测、失败归因与修补

1. 生产修改前 `red-cache-recovery-v1.log` 的两个实际 scheduler/new-runtime 测试均失败：两种窗口的 cache 写入实际为 0，要求为 1。这是本批功能红测，不是编译失败。
2. 首轮 `refresh-v1.log` 为 89 passed / 1 failed / 4 ignored。旧 writer-lock 测试在请求前持锁，新恢复预检会正确阻止发送，因而无法覆盖其原定的“响应后保存失败”窗口。锁改为实际 HTTP 到达后持有，并另加预检锁占用时零发送合同，没有恢复生产旁路。
3. 第二轮 `refresh-v2.log` 为 91 passed / 2 failed / 4 ignored。Grok cache 产品负测实际暴露 adapter 相同不足以证明产品相同，已补专用 preset 绑定。另一次失败为测试 RESP 服务把客户端断连 `ConnectionReset` 当成 panic；旧二进制同名用例另重复 5 次均通过，见 `v2-ack-failure-repeats.log`。夹具现只接受该已知 socket 断连，其他 I/O、任务 panic、RESP 和业务断言仍会失败。
4. 修补前 `red-malformed-target-v1.log` 实际运行 4 项，2 pass / 2 fail：再次复现产品 preset 负测，并新复现空 client 合同下数值类型被当成缺省。最终补齐 endpoint/client/expiry 的类型边界、可选 null 正例和缺产品 preset 负例。
5. 最终 `refresh-v3.log` 全绿；修补后的 ACK 用例另重复 5 次均通过，见 `v3-ack-fixture-repeats.log`。重复执行不另加到不同测试数量。

## 4. 定向验证终态

后端命令均为 `--locked --jobs 4 --no-default-features`，测试启用 `runtime-defaults`，不触发 `management-ui` 工作区发布/清理 hook。TEMP/TMP/TMPDIR 指向本批 `runtime-temp`，16 MiB 最小线程栈只设置给测试进程。

| 实际选择范围 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test ... --lib token_refresh:: -- --test-threads=1` | 94 passed，4 ignored | `refresh-v3.log` |
| `cargo test ... --lib console:: -- --test-threads=1` | 117 passed | `console-v3.log` |
| `cargo test ... --lib routing::config:: -- --test-threads=1` | 119 passed | `routing-config-v3.log` |
| `cargo test ... --lib token_refresh::tests::cache_recovery_real_redis:: -- --ignored --test-threads=1` | 真实 Redis 2 passed | `real-redis-cache-v1.log` |
| `cargo test ... --lib token_refresh::tests::durable_real_redis:: -- --ignored --test-threads=1` | 真实 Redis 2 passed | `real-redis-scheduler-v1.log` |
| `cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib` | exit 0；代表性 feature-disabled，不是全矩阵 | `feature-disabled-v3.log` |

按完整测试名去重，本轮 **334 个不同 Rust 测试通过**；四项默认 ignored 仅在本批实际运行成功后计入。Redis scheduler 两项复验的是独立 runtime 单发送者和 unknown result 后重建 runtime 不再次 exchange，不把它们说成全新测试。

静态和来源检查：

- `cargo fmt --all -- --check` exit 0，见 `rustfmt-v3.log`。
- checker tests 31/31 passed，见 `checker-tests-v2.log`；checker 未再改变，终态不无故重跑。
- `npm run check:effective-lines --prefix scripts` exit 0，见 `ratchet-v3.log`；未改 baseline/exception，不声称 strict 债务审计通过。
- 收尾后另做 `git diff --check`、37 项源码 SHA-256 与 `source-frozen-v3.json` 核对、源码及三份文档的严格 UTF-8/无 BOM/尾随空白和本地链接检查，终态见 `audit-final.json`。

有效行使用官方 checker。主要已测前值：`runtime.rs` 456→487，`runtime/local_backend.rs` 233→246，`persistence.rs` 322→324，`redis_store.rs` 466→469，`token_refresh.rs` 198→212，`server_cache.rs` 180→212；新增 runtime cache 118、publication 105、Redis cache 96、GrantLocator 31、cache target 89。冻结的 37 个源码最大 487；后纳入而缺初始值的文件在审计保留 null，不伪报从零新增。没有本批 501–700 例外或大于 700 的文件。

### 4.1 本次新 Redis 的授权与清理

用户对 P34 的单容器创建及精确清理明确回复“同意”后，创建 `gateway-p34-redis-30ef98a5e3d2`，完整 ID `edf84c7990f9b81e8102b83fc69eb54832057cbcceb092c3e97a43246657fb02`。使用固定已有 image `sha256:487efc0616382465781b8fdc3d6d1db449e6fd80ae23bf48432a2da6b6929908`，不 pull；只发布 `127.0.0.1:55534`，不挂现有卷，`/data` 为 64 MiB tmpfs，rootfs 只读、128 MiB/0.5 CPU。临时密码不写入回执或报告。

实读 `CONFIG GET save appendonly` 为 `save=""`、`appendonly="no"`。新增 Redis 合同一项从明确 seeded 的旧 cache 状态重建 runtime 并离线修复，另一项逐个改变五重 CAS 的 key，证明 authority/grant/transaction/cache 变化时不误写，再验证真实 repair API 成功。stale-cache 状态是显式播种，不冒充实际杀进程；所有 HTTP 都是合成 loopback，没有真实平台请求。

finally 重新核验精确 ID、purpose 与隔离配置，只停止这只新容器并由 AutoRemove 删除。`disposable-redis-completion.json` 两套 exit 均为 0，`remaining=0`。没有 FLUSHDB、现有容器/卷删除或既有服务停止。本次单容器授权已使用并清理，不复用到下一只容器。

## 5. 仍未完成与接力入口

1. 只恢复当前 active revision 明确来源的 publication；其他 policy 等 revision 之后的跨 revision lineage/Result 恢复、删除/重加的身份连续性不由本批推定。无来源旧数据不扫描 token 猜归属。
2. 永久 Sending 的可用性恢复/人工重新授权、其他 refresh 入口、多 Local root/namespace、真正独立 OS 进程强退、断电与 Redis durability/failover 仍需单独闭合；不宣称 universal exactly-once。
3. P33 广 `cargo check --lib --tests` 的 depth-limit 失败继续保留，本批没有重跑或宣称全仓绿；本批仅使用上表定向范围。
4. 正常 Grok 管理 HTTP/UI、provider/revision 实查、device/listener、SecretPatch/CAS 保存、完整业务保真和客户端使用边界仍缺。Accio/Kiro/Vertex、Perplexity Web、剩余 API/媒体，以及 P50/P60/P70 全部保留。

本批没有新增模板，前端仍为此前的九个新增模板。未读取历史凭据、真实模型/费用调用、录制、出包、替换实例、提交或推送。下一步不要重复实现已闭合的当前 revision locator；继续跨 revision 可信恢复、Sending 可用性与其他刷新入口，再接正常 Grok 登录管理闭环，并按总计划推进其余独立渠道。
