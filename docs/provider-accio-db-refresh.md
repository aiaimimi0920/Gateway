# Accio 数据库刷新权限与持久结果合同

本页描述 PostgreSQL 凭证的刷新存储 owner，不是 route-config authority，不是整个 Accio 渠道已完成的声明。独立 DB steward／专用单次合成 HTTP／持久结果及新worker恢复的合同已验证，见 [worker回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-09-provider-channel-expansion-accio-db-steward.md)；后续真实server／管理TCP／共享Gateway Key public子闭环已验证，见 [联合回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-09-provider-channel-expansion-accio-db-server.md)。unified key/bundle/prepaid、独立进程、全部后台join、包与真实供应商仍未验，现有ChatGPT steward不因此支持Accio。正常DB reader不读取hosted `gw:cred`，不能误写同名Redis entry冒充缓存接线。

## 数据库权限来源

`src/db/provider_credentials/refresh` 从实际数据库行取得 account 与 credential 的权限，再读取 inline/object payload。Account 使用 `FOR SHARE`，credential 使用 `FOR UPDATE`，顺序为 account → credential → grant family head → grant record。凭证必须仍属于原 account；二者必须 active、凭证未归档，adapter 和最终业务受众必须是固定 Accio。

每个 account/credential 具有数据库生成的 UUID incarnation 与单调递增的 authorization generation。authority digest 同时绑定两者稳定 ID、incarnation/generation 和两层完整 payload，不能用 `xmin`、时间戳、token 字符串或临时 route YAML 冒充长期权限。

- 正常 payload UPDATE 即使保留相同对象键也推进代际，防止原位 object writer 的成功 SQL 更新不可见。
- 禁用、归档、转移账号、改变 adapter/稳定 ID 等控制变化推进代际；删除后重建得到新 incarnation，不允许同 ID、同 created_at、同材料恢复旧发送权限。
- `active/cooling` 属于同一 enabled 权限桶。Cooling 期间不接纳或发布；正常冷却恢复后可以发布原已确认 Result。禁用后恢复仍是新的授权代际。
- 单纯诊断、时间戳、failure_count、相同 active 状态的健康更新不是新的 grant 权限。

原材料仅取自该 credential。账号层不能提供 access/refresh/SG/client/endpoint 或其他产品的 grant context。复用协议 owner 的材料、别名和绝对期限解析；部分 clear 与非空 alias 同时存在时拒绝，不恢复归档秘密。没有 client 身份、TTL、设备身份或额度推断。

## 永久消费记录与发布

复用 `console::refresh_grants` 的 `Sending → Result → Published` 状态机，而不是复制弱化版本。

1. 在真实 DB 权限仍匹配时，事务内锁全局 token-family head 并插入 Sending；**只有明确确认 commit 才能允许后续 HTTP**。
2. Sending 无 TTL。丢失发送者、超时、进程退出或失去 commit acknowledgement 不证明 grant 没有消费；重复/重启只读原记录，不重新取得 Reserved。重复 credential 也不能用同一 family 再发一次。
3. Result 在原 attempt 上先单独持久化，即使账号/凭证已禁用或删除也保留。不因撤权丢弃已返回的轮换材料；不同 result 不覆盖同一 attempt。
4. 发布重新锁真实权限与 family head；只有原权限、原 attempt 和原 Result 一致时更新 live credential。
5. Credential payload/pointer、实际 successor authority receipt 与 Published 在同一 PG transaction 中提交。重复发布仅在实际 authority 仍等于该 receipt 时确认，不把后续更新或重建的同名凭证当作原发布成功。

绝对 `expiresAt` 保留原精度，不用重启后的 `now + lifetime` 延长期限。Refresh 缺新 SG 时沿用原同一 grant 的 SG；这不同于新登录缺 SG 时 clear 旧 SG 的合同。发布只修改已识别的授权字段，保留设备/同 grant 上下文、policy、label、status、source hash/path、诊断和其他元数据；root `token` 未被 parser 认定为 access alias，因此保留，只清理 extra-body 层已认定的认证副本。

## 对象与普通管理并发

Object-backed 发布使用由 family/material/attempt identity digest 派生的**新键**，不原位 put 或删除 predecessor。PG 发布失败时原 pointer 与对象 bytes 保持不变；已知 staged 对象和 Result 保留供原操作恢复。回滚不能撤销 object put，不宣称跨 PostgreSQL/object store 的分布式事务，也不自动清理保留的旧对象或失败 staged 对象。

普通管理更新现在从同一次 `FOR SHARE` 读取取得 hydrated view 和短期 `xmin` snapshot，再走已有 snapshot updater。迟到的改名/metadata-only 请求不能把刷新前的整份 payload 写回；在对象 side effects 前返回 `provider_credential_snapshot_conflict`，调用方需重新读取，而不是盲重试旧输入。这个短期 CAS 不充当永久 refresh authority。

既有 account object writer 和 deletion 在其 SQL 行锁前操作对象的边界没有在本批改写。新 owner 的局部 predecessor 保护不能宣称这些旧 writer/deletion 的全流程对象原子性已完成。后续后台接线仍须审核实际取消、删除、重启和缓存并发。

## Schema 扩展和部署

增量脚本为 `deploy/postgres/initdb/005-gateway-refresh-authority.sql`。它增加两张权限行的 additive 字段、generation triggers，以及不带 credential/account cascade 的 grant/head 表。记录含真实凭据秘密，应用角色、备份、恢复和访问审计必须采用凭据存储的保护等级；不能在普通管理响应或日志显示 record。

**Platform 仍拥有正常 PostgreSQL schema。** 不手改 `001-gateway-schema.sql`，不由 Rust 自动 DDL，不迁移现有实例。005 是本功能的待部署扩展；与 Platform 共库时必须经 schema owner 纳入正常 migration/ledger，独立既有服务器也须审核、备份后在事务内应用。添加带 UUID default 的列及 triggers 有表锁/重写成本，不把 additive 误称为无影响在线迁移。新 initdb 可按顺序执行脚本，不代表旧数据目录会自动升级。

重复应用不会重建已存在的 incarnation 或删除任何记录。回滚到旧 binary 时保留扩展，旧 reader/writer 忽略新列；不要 Drop 表、清空 Sending 或回写旧 token 来“回滚”。任何收缩/归档与生产迁移需另行批准。本批只在明确批准的一次性隔离库中验证。

## 边界和未接内容

一次 snapshot 每层 payload 上限 4 MiB，grant record 复用共享 512 KiB 上限；处理串行、无新增后台队列或自动重试。Opaque IDs 与固定 digest 作为 SQL 参数/对象键组成部分，错误为固定本地诊断，不附 token、callback 或上游正文。

先前存储 owner 的聚焦测试不证明 scheduler、网络或整卡完成。后续 DB worker 已接单独 spawn/drain-stop/join、keyset、Result-before-due、发送前原权限锁及专用 HTTP；真实隔离PG＋合成HTTP的最终默认栈19项worker合同通过，连同去重相邻合同164项通过，具体来源与边界由本轮回执记录。I/O／HTTP／process future 固定到堆上，不新增task、retry或Drop owner。DB 正常请求每次 fresh hydrated read，hosted Redis raw CAS/recovery 仍属另一身份，不在此路径创建平行 cache。

发送前权限锁保持到单次HTTP结束；SQLx Drop rollback释放，已确认material先留入唯一内存slot，再进入独立Result transaction。存储等待不延长HTTP、未知Sending不重发、正常shutdown仅有限原操作收尾与persistence-only flush。共享Key的正常server/管理/public联合已有上述运行证据；完整身份/结算、各运行模式、独立进程与包、真实client/wire/模型额度和整卡 `offline_ready` 仍须各自验证，不从源码推定完成。
