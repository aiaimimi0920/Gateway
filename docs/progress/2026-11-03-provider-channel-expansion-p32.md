# 服务商分渠道扩展：P32 的 grant 消费证据存储前置

> 2026-11-03；R05/W03/P20 刷新生命周期继续实施。本回执是 P32 的存储层前置，不是 P32、Grok OAuth 或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有脏改，不提交、推送、出包或替换运行实例。

## 1. 本批实际范围

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p32`。

在现有 Console persistence / Redis store 内增加 grant 消费证据存储能力，没有建立第二套网关或普通配置写入口。它与 P31 的 `refresh-intents` 不同：P31 只证明配置事务的有限投影来源；本批记录网络发送可能已经发生，以及随后已接受的轮换结果。

**当前尚未接入 `RouteConfigRedisBackend`、`RouteConfigRuntime`、scheduler 或其他刷新入口。除了初始化一个新的空存储子目录外，现有生产刷新行为没有因此改变，仍不能声称已解决跨实例重复发送。** 本批只证明下面的存储合同，为下一步实际接线提供已测 owner。

## 2. 存储合同

- `Sending → Result → Published`；Sending 没有 TTL，也没有“owner 失联即可接管”的分支。只有这次 reserve 明确返回 `Reserved` 才能作为未来发送入口的准入，`Existing` / `Blocked` / `Contended` / `Stale` 都不授予发送资格。
- family identity 由规范化 token endpoint、client 与 refresh token 导出；不同 Provider ID、access token 或 expiry 不能绕开相同 refresh family 的未收敛 head。只协调同一 authority namespace / Local state root，不宣称跨独立安装、不同 client/endpoint 的普遍防重。
- material identity 另绑定 access、授权 context 和绝对 expiry。旧 material record 永久保留；只有 head 已 Published 且新材料与其保存结果全等，才能推进同 family 的后继，不能靠编辑配置或 TTL 清除旧消费记录。
- 成功结果保存不接受另一份冲突结果；重复保存同一结果幂等，已 Published 不降级回 Result。Grok context 沿用既有 principal/client/scope 只缩不扩规则。
- Published 需要结果的材料、绝对 expiry 和 lifetime 已与实际权威配置全等。它表示历史 authority publication，不表示 cache 已完成，也不表示此刻 authority 永远不变。
- Redis reserve 在一次 Lua 中 CAS active revision、immutable revision 的原始 bytes、head、前一笔 record 和目标 record 不存在性；不写 legacy mirror，不新增 TTL。结果 transition 同样按观测 raw bytes CAS，不以 JSON 重序列化代替旧值比较。
- Local 复用正常 writer lock、active pointer、archive 和 atomic JSON publisher。先写 head 再写 record；中间退出留下 dangling head 后拒绝，而不是把不确定性当作未发送。没有跨 HTTP 持有磁盘锁或创建后台任务。
- 记录最大 512 KiB，变量文件/key 组件只使用固定长度 digest；读取拒绝 links、未知字段/version/phase、损坏或缺失的 head-record 关系。grant 类型 Debug 和新错误归一为固定脱敏文案。

主要 owner：

- [请求/身份合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/refresh_grants/mod.rs)
- [授权材料身份](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/refresh_grants/material.rs)
- [消费记录与结果状态机](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/refresh_grants/record.rs)
- [Local 存储](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/persistence/refresh_grants.rs)
- [Redis 原子存储](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/redis_store/refresh_grants.rs)

## 3. 验证和复审

新 API 的初始合同在 `red-api-v1.log` 因 API 尚不存在而编译失败；其中还含测试误用 private snapshot accessor 的 E0624，已改用实际初始 document。**这不是已重现生产重复 grant 的行为红测，不能将编译缺 API 冒充网络竞争实证。**

只读复审发现仅按 token/context 标识 material 会阻止“access/refresh 均不变、仅延长有效期”的合法续期。最终实现把绝对 expiry 纳入 material、仍不纳入 family，并补同 token 续期和未知状态下修改 access/expiry 的正负合同。复审还发现新增 Redis revision 读取会继承原解析错误；本路径已将其归一，避免返回原始 JSON 字符串片段。

`source-initial.json` 在设计/修改前记录 3 个既有接线文件；`source-frozen-v1.json` 冻结实际 10 个源码/测试输入。

| 检查 | 结果 / 证据 |
| --- | --- |
| Local grant 新合同 | 6 pass / 0 fail；`local-contract-v1.log`，编译 7m27s、运行 0.64s |
| 新 Lua 的真实 Redis 合同，显式 `--ignored` | 5 pass / 0 fail / 0 ignore；`real-redis-v1.log`，运行 1.04s；不是没有环境变量时的跳过 |
| 新 API 定向编译 | 通过；`check-api-v1.log`，当时仅初始 3 项测试，后增合同以实际 test 编译为准 |
| 正常 Local/事务/key 相邻合同 | 23 pass / 0 fail / 0 ignore（Local 7、key/namespace 4、正常事务/恢复 12）；`integration-adjacent-v1.log`，编译 38.71s |
| 本批后端合计 | 34 个不同测试通过（新 Local 6、新真实 Redis 5、相邻 23），不累加 P31 或 checker 的重复执行 |
| 代表性 feature 关闭编译 | `--no-default-features --features line-gemini-web-reverse --lib` 通过；`feature-disabled-v1.log`，1m46s，不是全 feature 矩阵 |
| 静态 | Rustfmt、checker 31/31、ratchet 4104 文件 / 14 immutable / governed source >700=0、tracked diff check 通过 |

最终源码冻结、UTF-8 无 BOM、含未跟踪源码的空白与本地文档链接审计见 `source-doc-audit-v1.json`；测试数量按原始日志核对见 `test-result-audit-v1.json`。本批没有运行此前被无关 native_transport 夹具阻挡的广 `cargo check --tests`，也没有声称全仓编译/全回归通过。

后端使用 `--locked --jobs 4 --no-default-features`，主合同为 `--features runtime-defaults`；TEMP/TMP/TMPDIR 均指定本任务 `runtime-temp`，测试进程保留既有 16 MiB stack，不改变生产线程设置。没有运行默认 management-ui 发布/清理 hook、依赖升级、baseline 重建或添加豁免。

新生产模块有效行为 97 / 117 / 182 / 163 / 244；Local/Redis 测试分别 284 / 286。三个既有接线文件为 Console mod 42→43、persistence 320→322、Redis store 465→466。各文件单一职责且低于 500；没有增长 >700 债务文件。

## 4. 隔离 Redis 授权及清理

使用用户此前明确“同意”的单个新建容器授权。本批新建的精确 ID 为 `290a66a53b3776ba0cd85fc846795e63ae4286458a09067f304176403880e572`，purpose label 为 `provider-expansion-p32`。

采用本地已有 image ID、`--pull=never`、仅宿主 `127.0.0.1` 的随机端口、128 MiB 内存、只读 rootfs、`/data` tmpfs 参数、关闭 save/AOF、随机临时密码。记录的 mounts 为零，没有挂载用户路径或既有卷；没有 FLUSHDB/FLUSHALL 或写默认 namespace。每项使用随机独立 namespace，仅以合成授权材料测试 Lua 顺序合同。

`disposable-redis.json` 记录身份/端口/隔离，`run-disposable-redis.ps1` 是实际执行脚本。清理前核对精确 ID、purpose、无非 tmpfs mount 和 AutoRemove，仅 stop 此实例；`disposable-redis-completion.json` 记录 testExit=0、剩余容器=0。该授权已经用于此单个实例，不扩大为后续任意容器/卷清理。

## 5. 安全与未完成边界

记录包含 original 和成功轮换的真实秘密副本，只能位于与凭据配置归档相同的受控 owner。没有暴露到 HTTP/UI、普通配置模板、日志或报告；Debug 不输出材料。不宣称现有文件 publisher 自动提供加密或新的 ACL 保证。旧 storage/journal/revision schema 和版本未改，旧二进制也不会遵守新增 grant gate。

Local 新 persistence handle 与 Redis 多 store/concurrent future 证明的是存储合同，不是完整 Server/Worker 进程强退或真正网络发送。文件 sync 仍复用已有平台原语，不把 Windows 目录 sync 或该测试的无持久化 Redis 当成断电 durability 证明；实际 Redis 持久化/故障切换与消费证据保留的部署合同仍需解决，不能只凭 SET ACK 声称任意故障后安全。

下一步仍属于 P32：

1. 将此能力挂到现有 backend trait，Pooled/Local 复用各自 owner；未知 backend 默认 fail-closed，Worker 不获得通用写权限。当前 request 提取仅接受 credential 显式 access material；接线前补正常继承语义，不能降低既有支持范围。
2. 实际 scheduler 必须在任何 grant 发送前绑定当前 authority 与 pinned attempt；传输前校验先完成。仅新 Reserved 可以发送，reserve ACK 不明不得重发；Sending 永久保留未知状态。
3. 成功先保存可恢复 result，保存 ACK 不明仅重试同结果；然后通过有限投影发布 authority/cache。重启从 durable record 恢复，不反序列化进程内 Arc/generation/OwnedMutexGuard 冒充旧 ticket。
4. 实现 authority 已提交后的 cache 修复定位，以及旧记录、人工重授权、撤销、未知结果、取消/shutdown/强退的实际入口合同。Published 不是 cache 完成证据，不可据它旁路新授权 fence。
5. 其他刷新入口（包括既有 keepalive 旁路）和旧新实例混跑仍需审查；不能把一个未接线的存储 owner、普通 lease 或缓存 CAS 当作所有 grant 已防重发。

正常 Grok 管理登录/UI/SecretPatch、其他原定平台、P50 集成包、P60 集中录制与 P70 真实验收全部保留。仍为 9 个新增前端模板；未读取历史凭据、调用真实上游或改动在线实例，总目标继续执行中。
