# 模型生成探测与定向健康恢复

本实现参考 AIGateway fork 的冷却恢复需求，使用 Gateway 现有模型健康 owner 重写；不提供全池清零，不复制 CPA 的 ResetQuota 执行器，也不新增持久化版本或第二套健康库。

## 生效范围

现有 Console 的 OpenAI-compatible chat / 官方 Codex 生成探测，以及调用它们的手动和自动测试计划，在生成前固定精确健康身份与预期状态。只有完整、非空回复才是调用成功；模型调用成功不等于健康恢复一定成功。

健康身份与普通 route-config 候选一致：provider ID、真实 credential ref（无凭据池的默认账号使用候选本身的空 ref，而非展示用默认账号 ID）、有效 protocol profile 和映射后的 upstream model。profile 解析与候选构造共用函数，包括 discovery 覆盖。Console 目标来自 route config，不会猜测一个同名的数据库 credential ID。

开始与结束均复核当前 target 的 payload。OAuth runtime override 在同一 revision 内变化也会使旧探测失效。payload 仅在内存做结构化比较，不回传到 API，不记录秘密或公开凭据摘要。

## 并发与保护

- Local：读取完整原始 payload；结束时进入既有 `BEGIN IMMEDIATE` 事务，比较预期 payload 后才调用原有状态转换。所有 Local owner 写入使 `updated_at` 严格递增，即使时钟回拨也不复用同一 payload token。极端不可推进时间戳返回错误，不 panic，不回退墙钟覆盖。
- Server PostgreSQL：使用仅在本次操作内存活的 `xmin:ctid` token。事务内锁定精确行，拿锁后重验，再调用同一事务的普通转换 writer。初始不存在时冲突分支禁止 overwrite，不能用成功插入覆盖期间出现的新失败。它不是永久、可排序或对外的业务版本。
- 成功和失败探测都经过 expected-state 检查。较早启动、较晚返回的失败也不能把期间新增的硬封禁降级成普通冷却。
- 已有 `blocked` 模型状态保留；被禁用的 credential 不获恢复许可。不会恢复其他 model/profile/credential，不重置全局 breaker、账号配额快照、错误预算或 credential enabled 状态。
- 生成探测完成后不再追加旧的 credential/account-wide 无条件 success/failure 写入。旧的非生成型 connectivity probe 仍保持原合同，并没有被宣称全部迁移到 CAS。

Local 继续读取相同字段的旧 JSON，数据库 `user_version` 不变。时钟回拨时，失败冷却使用单调逻辑时间，可能比当前墙钟更保守。当前不存在的行在期间插入又删除的 absence ABA 不会保留历史 tombstone；不宣称能检测所有历史修改。

## API 回执

既有单次结果新增可选 `healthRecovery`，测试计划在每个 case 上保存该字段；计划顶层不把最后一次更新冒充整轮恢复。旧结果缺失该字段仍可读取，旧客户端的 passed/failed/unsupported 字段不变。

| 值 | 含义 |
| --- | --- |
| `applied` | 精确健康行已确认写入 active；不表示账号级所有限制都解除 |
| `failure_recorded` | 本次失败已确认写入精确健康行 |
| `failure_not_penalized` | 本次失败不处罚模型路由 |
| `stale_health` | 预期行已变化，本次不覆盖 |
| `stale_configuration` | 写入前发现 revision 或 payload binding 变化，跳过本次更新 |
| `configuration_changed_after_write` | 写入后复核发现配置变化，需重新核对，不宣称当前配置已恢复 |
| `protected` | 保留硬封禁，不自动恢复 |
| `storage_failed` | 存储更新未确认，不能断言一定未提交；刷新核对，不盲重试 |
| `unavailable` | 当前没有持久化模型健康 owner |

沿用现有 management authentication、精确 Secret Grant、测试输入/响应大小限制和调用预算，无新增公开写接口。前端在调用成功时也展示独立恢复回执，不用绿色调用状态掩盖 stale / protected / 存储不确定性。

## 配置 owner 边界

路由发布与 SQLite/PostgreSQL 是不同 owner，本实现没有跨 store / 跨实例的原子事务。写前校验并不排除 SQL await 期间发生路由发布，因此写后再验，并在变化时返回独立不确定性状态；不会用旧健康快照补偿回写。返回之后发生的配置变化同样不受本次结果保证。

普通业务请求成功的既有完成顺序语义保持不变，本批只为生成探测增加 expected-state CAS，不宣称修复所有请求的完成顺序问题。历史 profile-less probe 行不会批量迁移或删除；它们不被拿来代替精确路由的健康证据。

## 验证

自动测试只调用 loopback 合成生成端点和隔离 Local 数据库。共用健康 CAS 合同覆盖 credential DB ID / ref-only identity、初始缺行、新失败、ABA-shaped 转换、并行恢复与硬封禁；PG fixture 额外包含锁等待后的物理版本重验，必须显式启用并通过其 loopback、独立数据库、非业务端口及 run ID guard。

本轮实际执行情况以对应进度记录和证据目录为准。未运行的 PostgreSQL opt-in 不算通过；不借用业务数据库，不以 SQL 编译或源码 review 代替真实 PG 并发证明。测试也不代替 EXE 打包、真实供应商、Redis live 或部署验收。
