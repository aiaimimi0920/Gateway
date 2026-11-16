# 服务商分渠道扩展：P33 实际刷新准入与成功结果恢复接线

> 2026-11-03；本批接线及下述定向验证已通过，完整生命周期仍执行中。本批推进 R05/W03/P20 的共同刷新 owner，不是 P32、Grok OAuth 或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有脏改，不提交、推送、出包或替换运行实例。

## 1. 本批实际范围

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p33`。

[P32](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p32.md) 原先只有存储能力；本批将其接入 `RouteConfigRedisBackend`、Pooled/Local backend、私有 runtime 能力以及实际 `token_refresh` scheduler。未知 backend 默认拒绝，正常生产启动始终提供 runtime，测试或手工构造的 `None` 不再获得无 authority 直接发送的兼容旁路。Worker 没有获得普通管理或 automation mutation 权限。

实际发送顺序为：已知本地输入检查与准备 → 绑定 pinned authority/attempt → reserve → 仅新 `Reserved` 发送 → 成功结果先持久化 `Result` → apply → 正常有限配置投影 → `Published` → credential cache。reserve 超时、ACK 不明、Existing Sending/Published、Blocked/Stale/Contended 都不授予发送资格。没有因 owner 死亡或过期而删除 Sending 的路径。

generic transport 在 reserve 前完成 client 加载、endpoint/secret 绑定、URL 安全检查和 request `.build()`，之后发送同一个 request。Grok 前移 context、产品、client、refresh token 的已知校验并保留固定受众；它仍在专用 transport 内构造最终 HTTP request，不声称与 generic 一样已完成 HTTP `.build()`。

## 2. 成功结果、撤销与兼容

- pending 保存原 request、消费 record、响应和原 in-process attempt/guard；保存 Result 的义务不再受 `is_current` 控制。HTTP 期间删除/重新授权后，仍先保存已收到的 Result，再拒绝旧授权 apply、配置发布或 cache 写回。
- 保存 ACK 不明时仅重试同一结果；确认保存前不发布 runtime override。Result、配置发布、Published 共用 20 秒 authority 预算，cache 沿既有独立 10 秒预算；reserve 同样有界。
- Existing Result 只在 record 的 provider、credential、revision、original material 与 request 精确符合时恢复，不再发 grant；不能反序列化旧 Arc/generation/guard 冒充旧 ticket。其他 revision 的 Result 保守拒绝，未扩展为任意历史材料恢复。
- 恢复保留原绝对 expiry 和原 lifetime；runtime deadline 根据绝对时刻重新计算剩余时间，不给旧 token 再授完整 lifetime。cache 原本已经按绝对 expiry 设置 TTL。
- provider-level access 继承、显式空 generic access、credential/session expiry 选择、无效 expiry 的 unknown/due、缺省空 client 都按真实旧编译合同保留。refresh endpoint/token/client/context 不从 provider 或其他产品猜测继承。
- actual attempt 的标准化 endpoint、client、access、refresh、context 必须与 pinned request 全等，expiry 必须表示同一绝对时刻；不是要求 RFC3339 小数位拼写相同。runtime override 与 document 未收敛时不准新发送。
- 配置投影现在要求 Result 已确认且 apply 完成，收到响应本身不再构成 authority admission ticket。

主要 owner：

- [实际 scheduler](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh.rs)
- [准备、绑定及 replay](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/grant.rs)
- [Result/authority/Published/cache 顺序](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/persistence.rs)
- [runtime 窄能力](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/refresh_grants.rs)
- [Local backend](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/local_backend.rs)

## 3. 实测失败与责任层修正

`red-scheduler-v1.log` 是新增测试误用临时 client 引用的 E0716，不作为生产行为红测。修复该测试后，未改生产源码的 `red-scheduler-v2.log` 实际运行 5 项、全部失败：

| 原行为 | 实际旧结果 | 要求 |
| --- | --- | --- |
| runtime 缺失 | HTTP 1 次 | 0 次 |
| Local writer lock 已占用 | HTTP 1 次 | 0 次 |
| durable Result 已存在 | HTTP 1 次 | 0 次，离线恢复 |
| 两个独立 Local runtime | HTTP 2 次 | 1 次 |
| 发送结果未知后重启 | HTTP 累计 2 次 | 仍为 1 次 |

接线首版的 `scheduler-v1.log` 是测试把 `Option<String>` adapter 赋成 String 的 E0277，不冒称生产失败。修正夹具类型后，`scheduler-v2.log` 实际 81 项：78 pass / 3 fail。随后同一个二进制的 `failure-repeats-v2.log` 显示 Result ACK/restart 用例连续三次失败，另两项分别重复三次通过；没有将重复通过掩盖为全部已绿。

复审和真实源码定位得到两个生产问题：

1. `future_rfc3339_after_secs` 输出固定毫秒，`.120Z` 经 RFC3339 format 会变 `.12Z`。字符串比较使真实 Published/后继随机失败。材料的 expiry 比较改为同一绝对时刻；其余 token/context/endpoint/client 仍全等，只有已 Published 的匹配结果允许后继，Sending/Result 不因此获得后继资格。
2. 相同内容的 bootstrap revision ID 不含 timestamp；重启新建 snapshot 可能保留新的 timestamp，全局同 ID 恢复会直接跳过。grant 原先比较完整 revision JSON，因而误判 Stale。修正在 grant owner 内要求完整 ID、完整 document/yaml digest 和完整 document 相等，不把非授权 metadata 的 bootstrap 差异当新文档。**Redis 仍将真实读取的不可变 revision 原始 bytes 原样传给 Lua，原子检查 active ID 与 immutable raw；不重写 revision，不放宽全局同 sequence 安装规则。**

修正上述两处后，`scheduler-v3.log` 为 82 项：81 pass / 1 fail；剩余失败暴露同一 expiry 拼写差异也存在于 actual attempt 与材料绑定。继续抽出共享 `GrantMaterial::matches_expiry`，并把确定性 fractional-expiry 测试加强为 force-due 后再次走真实 scheduler 的合法后继。最终 `scheduler-v4.log` 为 84 项：82 pass / 0 fail / 2 ignored；这两项 Redis opt-in 随后已在本次授权的新容器中实际执行通过，不把 ignored 计为通过。

旧测试中“先持 Local writer lock 仍发 HTTP”已改为先见到真实请求再持锁，测试成功结果保存失败；另加锁占用时零发送合同。所谓独立第二账号现在使用独立 refresh token，并另测复制同 RT 的两个 ID 不代表两个 grant 预算。旧 `runtime=None` cache/policy 夹具改走正常 authority，不为旧夹具保留生产旁路。

## 4. 定向验证终态

所有后端命令显式使用 `--locked --jobs 4 --no-default-features`，测试启用 `runtime-defaults`，避免触发 `management-ui` 的工作区发布/清理 hook。测试的 `TEMP/TMP/TMPDIR` 指向证据目录下 `runtime-temp`，仅测试进程设置 `RUST_MIN_STACK=16777216`。

| 实际命令的选择范围 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test ... --lib token_refresh::tests:: -- --test-threads=1` | 82 passed，2 ignored | `scheduler-v4.log` |
| `cargo test ... --test console_refresh_grant_contract -- --test-threads=1` | Local 7 passed，Redis 6 ignored | `local-storage-v1.log` |
| 同一 storage 合同加 `--ignored --test-threads=2`，真实 Redis | 6 passed，0 ignored | `real-redis-storage-v1.log` |
| `cargo test ... --lib token_refresh::tests::durable_real_redis:: -- --ignored --test-threads=1` | 2 passed，0 ignored | `real-redis-scheduler-v1.log` |
| `cargo test ... --lib routing::config:: -- --test-threads=1` | 119 passed | `routing-config-v1.log` |
| `cargo test ... --lib console:: -- --test-threads=1` | 117 passed | `console-v1.log` |
| `cargo test ... --lib protocol::grok_oauth:: -- --test-threads=1` | 52 passed | `grok-oauth-v1.log` |
| `cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib` | exit 0；代表性 feature-disabled，不是全矩阵 | `feature-disabled-v1.log` |

本轮上述 Rust 测试按完整测试名核对，共 **385 个不同测试通过**；默认 ignored 的 8 项仅在后续实际运行成功后计入，不累加早期失败、重跑或旧批结果。

静态与来源检查：

- `cargo fmt --all -- --check` exit 0，见 `rustfmt-v4.log`。
- `npm run test:effective-lines --prefix scripts` 为 31/31 passed；本批未改 checker，复用 `checker-tests-v1.log`，未为计数无故重跑。
- `npm run check:effective-lines --prefix scripts` exit 0，见 `ratchet-v4.log`；未改 baseline/exception，不宣称 strict 债务审计通过。
- 最后文档收尾后的 `git diff --check` exit 0，见 `diff-check-final.log`。冻结的 32 个源码 SHA-256 全部匹配 `source-frozen-v4.json`，源码和本批三份文档共 35 个文本为严格 UTF-8、无 BOM、无尾随空白，117 处本地文档链接已逐项检查；最终回执 `audit-final.json`。

有效行数使用官方 checker 的冻结值，最大为 456，无 501–700 例外或大于 700 的本批文件。已在开工前测量的主要 owner：`token_refresh.rs` 191→198，`persistence.rs` 221→283，`transport.rs` 121→142，`grok.rs` 69→88，`console/runtime.rs` 407→456，`runtime/local_backend.rs` 194→233；新增 `token_refresh/grant.rs` 119、`runtime/refresh_grants.rs` 45。全部 32 项终态与已知前值见 `audit-final.json`；后续纳入范围的既有文件没有初始测量时保留 null，不伪报从零新增。

### 4.1 真实 Redis 与授权清理

用户对 P33 的新单容器许可明确回复“同意”后，才创建 `gateway-p33-redis-ee43fe7cfd04`，精确 ID 为 `aa4d5c054e604c7bb44b917edae1bc9bc33ca909129691d70ee19c91cc285d7b`。固定已有 image `sha256:487efc0616382465781b8fdc3d6d1db449e6fd80ae23bf48432a2da6b6929908`，不 pull；唯一端口绑定 `127.0.0.1:54823`，无现有卷，`/data` 为 tmpfs、只读 rootfs、128 MiB/0.5 CPU，随机临时密码不写入报告。

真实 `CONFIG GET save appendonly` 返回 `save=""`、`appendonly="no"`。6 项 storage/Lua 包括 bootstrap metadata 兼容但保留真实 immutable raw 的 CAS；2 项实际 Pooled scheduler 验证独立 runtime 并发只有一个 HTTP sender，以及 unknown result 后新 runtime 不再次 exchange。HTTP 是合成 loopback，未调用真实服务商。

测试后 finally 复核精确 ID、purpose 和隔离配置，仅停止这只新容器并依 `AutoRemove` 删除；`disposable-redis-completion.json` 的两套 exit 均为 0，`remaining=0`，随后按相同完整 ID 查询亦无残留。没有 `FLUSHDB`、现有容器/卷删除或既有服务停止。本次单容器许可已使用并完成清理，不能复用到下一只容器。该证据不是两个 OS 进程、断电或 Redis failover durability 证明。

### 4.2 未通过的广检查仍保留

一次广 `cargo check --lib --tests` 在 `provider_credential_folder_sync_database` 的 `harness::run` 遇到 `queries overflow the depth limit!`，见 `check-initial-v1.log`；未改该测试或增加 recursion limit 掩盖。它不是本批全部 integration 已验证的证明，也没有在没有基线证据时把该失败断言为本批造成或必然无关。

## 5. 未完成与安全边界

1. authority 已轮换后，重启仅凭新材料找回旧 family 的 Result/Published 并修复 cache 的 locator 仍缺。Published 不是 cache 完成证明。
2. Result 保存后若发生其他 revision，再重启，目前保守拒绝跨 revision replay；要实现完整恢复仍需可信 provenance，而不是放宽同材料判断。
3. 永久 Sending 保证未知消费不被当作未发送，但没有提供所有未知结果的可用性恢复或人工重授权闭环；只改 access/expiry 不会解锁同 RT family。
4. 其他刷新入口、旧新二进制混跑、独立 namespace/Local root、多进程强退及实际 Redis durability/故障切换仍需覆盖。本批不宣称 universal exactly-once。
5. P32 和 P33 的新容器各自独立授权并已清理；需要后续新容器时仍须重新询问，不复用已消费许可。
6. 新记录保存秘密副本，仍归既有受控配置 owner，不导出到 HTTP/UI/报告。没有宣称新加密/ACL，没有改 storage version 或迁移数据库。

本批没有读取历史凭据、真实模型调用、费用测试、在线实例变更、包交付或新模板。正常 Grok 管理登录/UI/SecretPatch、Accio/Kiro/Vertex、Perplexity Web、其余 API/媒体功能及 P50/P60/P70 全部保留；总目标继续执行中。
