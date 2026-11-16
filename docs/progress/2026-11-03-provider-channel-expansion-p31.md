# 服务商分渠道扩展：第 31 批，Worker 的受限刷新持久化

> 2026-11-03；继续 R05/W03 的刷新生命周期前置。本批受限提交/恢复及定向验证已完成，不表示 Grok OAuth、P29 或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有脏改，无提交、推送、出包或在线替换。

## 1. 问题与权限边界

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p31`。

生产修改前，两个实际 Worker 保存测试在 `red-worker-v1.log` 中均失败：0 pass / 2 fail，编译 11m04s。交换已得到新 token，但 `writable=false` 的正常 runtime 没有 coordinator，权威配置仍是旧 token；scheduler 周期及 shutdown 两处还会把该 runtime 过滤掉。

不能直接让 Worker 的公开 coordinator 变成可写，也不能放开 `commit_automation_document`。后者同时被池维护、补货、发现等完整配置写入使用。全量 `recover_startup` 也会处理旧管理/普通 automation journal，甚至从本地 Committed bootstrap，不属于当前刷新结果保存的权限。

## 2. 本批实现

- runtime 私有 refresh coordinator 复用正常 persistence、journal、backend 和 writer lock；公开 `coordinator()` 对 Worker 仍为 None，普通管理、通用 automation 仍拒绝，公开 Worker startup recovery 仍不做写入。
- crate-private 保存入口只接收由刷新模块持有、不可反序列化构造的 `PendingRefresh`。从一次 pinned snapshot 得到身份、revision 和原材料，核对共享 authorization/generation；CAS 重试会重新验证，不接收任意完整 document。
- 有限投影只更新 access、可选轮换 refresh、lifetime、absolute expiry、Grok authorization context 及已有 session expiry。完整 document 重建全等保护其他账号、产品/地址、headers、模型、enabled 和 policy。Grok context 复用原 exchange 的 principal、client 和 scopes 只缩不扩规则。
- 正常事务在持久化 Prepared 前写不可变 `console/refresh-intents/{txId}.json` 准入证明，绑定完整 Prepared identity、provider/credential 与 parent/new document digests，并保留 parent archive。此证明不存第二份 token，不改变旧 journal/revision schema 或 storage version；缺失证明的旧事务不会被自动追认。
- Worker 只读扫描 journal；未知/残缺尾部不先修复。所有 pending 的证明、parent/new archive、canonical digest、actor/parent/sequence 和完整投影全部验证后，才在同一 writer guard 内复用 P30 settlement。未知管理/普通 automation 事务使整批恢复在任何 fence/YAML 动作前拒绝。
- grant 前预检要求已经存在 Redis authority；private commit 内也独立禁止 Worker bootstrap。仅采用当前权威到内存，不执行全量恢复的本地 Committed fallback。
- scheduler 周期和 shutdown flush 都保留 Worker runtime；继续 authority→cache 双阶段、同一 pending/guard 和既有 20 秒预算。失败不重新 grant，不把非法投影与“已保存/已取代”混为成功。

主要 owner：

- [受限提交](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/refresh_authority.rs)
- [受限恢复](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/runtime/refresh_recovery.rs)
- [不可变准入证明](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/persistence/refresh_intent.rs)
- [公共有限投影](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/projection.rs)
- [实际 scheduler/flush 测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/tests/worker_scheduler.rs)

## 3. 复审与验证过程

只读复审发现 v1 在 settlement 后才检查 active 缺失，以及 projection 的 None 同时包含非法材料与已取代语义。v2 已把存在性检查放到任何 settlement 前，并在末尾重读；projection 改为 `Result<Option<_>, ()>`，非法投影明确失败。新增对应的“合法 pending + authority 缺失不产生任何 settlement/journal/YAML 变化”和“非法 ticket 不成功/不写 cache”测试。复审确认源码修复，但复审不代替运行证据。

v2 新 Worker 13 项全部通过（编译 12m35s，运行 1.92s）；随后刷新全模块为 64 pass / 1 fail。失败发生在两笔 pending 的夹具准备阶段，只生成 1 而非 2 笔记录。复审定位到与该失败吻合的竞争：第一笔 stall 的 Notify permit 没有被消费，第二笔 waiter 可能误领旧通知，在第二个 commit 真正执行前将其 drop；没有独立录制该次具体调度，不将源码推断冒充调度追踪。v3 用显式 join 消费第一笔通知，并断言第二笔确实新增记录及实际到达 YamlReplaced；仅修改 `worker_recovery.rs` 测试，不改变生产语义或削弱两笔事务/零 settlement 的断言。保留 v2 原失败日志。

`source-initial.json` 在生产修改前测量 123 个相关已有源码；`source-frozen-v2.json` 冻结 20 个实际变更源码/测试，v3 冻结同一集合并核对 v2→v3 只有上述测试文件变化。最终 v3 刷新全模块 65 pass / 0 fail / 0 ignore，编译 14m12s、运行 7.22s；新混合 pending 用例又独立重复 5 次全部通过。

另一次 `cargo check --tests` 遇到相邻既有夹具 `tests/console_provider_toggle_health_contract.rs:23` 的 `RecordCredentialModelFailureInput.native_transport` 字段不匹配（E0560，`check-v1.log`）。本批未修改该无关测试，也不把这次广检查记为通过；后续定向检查分别记录实际范围。

### 3.1 最终冻结验证

| 检查 | 实际结果 / 日志 |
| --- | --- |
| 刷新全模块，含新增 Worker 13 项 | 65 pass / 0 fail / 0 ignore；`refresh-adjacent-v3.log` |
| Console 相邻模块 | 117 pass；`console-adjacent-v3.log` |
| routing/config 授权 publication | 119 pass；`routing-adjacent-v3.log` |
| Grok OAuth 协议/会话/授权 context | 52 pass；`grok-adjacent-v3.log` |
| 正常提交/恢复 integration | 42 pass：background discovery 7、Local runtime 7、management 12、Redis namespace/key 4、transaction runtime 12；`integration-adjacent-v3.log`，编译 8m13s |
| 已观察到夹具竞争的混合 pending 用例 | 独立重复 5 次，均 1 pass；`mixed-pending-repeat-1-v3.log` 至 `-5-v3.log` |
| 合并计数 | 400 次执行、395 个不同测试身份，重复仅上面 5 次稳定性检查；`test-result-audit-v3.json`，不累计 v2/旧批次数 |
| 代表性 feature 关闭编译 | `--no-default-features --features line-gemini-web-reverse --lib` 通过；`feature-disabled-v3.log`，不是完整矩阵 |
| 静态检查 | Rustfmt、checker 31/31、ratchet 4097 文件/14 immutable/governed source >700=0、tracked diff check 通过；对应 `*-v3.log` |
| 冻结/文本 | 20 项源码 hash 匹配；24 份 UTF-8 无 BOM 文本、92 个去重本地链接、围栏和含未跟踪源码的尾随空白检查通过；`source-doc-audit-v3.json` |

后端均使用 `--locked --jobs 4 --no-default-features`，主定向套件为 `--features runtime-defaults`，未触发默认 management-ui 发布/清理 hook。测试进程使用既有 16MiB `RUST_MIN_STACK`，不改生产线程设置。checker 使用官方脚本同一 ratchet、无报告写入入口，避免覆盖既有脏 artifact；没有重建 baseline、豁免或升级依赖。

有效行：runtime 402→407、commit 220→249、persistence 317→320、journal 305→321、journal storage 175→184、context 150→160、scheduler 187→191、pending persistence 197→221；新准入证明/受限提交/受限恢复/共享投影分别为 87/119/107/58。新测试为 106/51/290/87/97，有明确单一测试职责；旧测试注册及两类 fixture 为 169→174、187→207、196→223。全部本批变更源码低于 500，没有增长 >700 债务文件。

## 4. 安全、兼容与剩余范围

新增证明是正常 secret owner 目录下的配置事务来源记录，不是 grant 发送记录，也不是另一份凭据真源。路径只由安全 tx ID 派生，读取有既有 metadata 大小限制、拒绝 links/非普通文件、版本和未知字段检查；相同身份幂等，不覆盖冲突内容。孤立证明不会被当作可执行事务扫描。普通可写 Server 的管理 bootstrap 保持原行为；Worker 的受限入口不能扩大为全量恢复。

测试仅使用合成账号、随机 loopback issuer/cache 及本任务 TEMP；没有读取历史凭据、连接真实上游、触碰在线数据库或启动 Gateway 实例。重启用新 store/runtime 和旧 bootstrap 合成，不是强制杀死完整 Server/Worker 的进程验收；RESP cache witness 也不冒充 Redis 引擎。本批没有改 Lua，无新增真实 Redis 测试声明。

本批期间收到用户对单个新建、回环绑定、无既有卷挂载、关闭持久化的 Redis 测试容器及仅清理该新实例的“同意”；当前没有必要新增容器，因此未执行该操作。此回复不扩大为既有容器/卷或任意后续清理授权。

仍未完成：发送 grant 前的跨实例所有权及持久 send-intent、结果不明/强制退出处理、成功响应到首个 durable result 间的崩溃窗口、authority 已提交后崩溃的派生 cache 修复。现有进程内 mutex、cache CAS、TTL lease 或本批 `refresh-intents` 均不能证明这些已解决。

下一步 P32 应在共享 authority 上绑定真实授权材料代际，先 durable 标记可能发送，再执行 exchange；另一实例不得因 lease 到期重发同代一次性 token。结果不明必须持久保留并依据可恢复结果、上游明确重试合同或重新授权处理，不能声称跨上游 exactly-once，也不能通过禁用 Worker 刷新来替代实现。先复用现有 owner，再补合成多实例/断连/崩溃证据。

Grok 正常管理登录/UI/SecretPatch 保存、Accio/Kiro/Vertex、Perplexity、新 API 分支及其他 W 卡剩余能力、P50 集成包、P60 集中录制、P70 真实验收全部保留。前端仍是 9 个新增模板，无新渠道真实验收或包交付；总目标保持执行中。
