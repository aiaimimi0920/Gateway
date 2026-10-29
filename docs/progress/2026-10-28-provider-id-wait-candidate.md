# 服务商 ID 在线迁移：等待后的同账号候选刷新

承接 `2026-10-28-provider-id-network-admission.md`。本轮接入的是已经进入等待的
请求醒来后的候选刷新，不是完整在线迁移或真实账号发送验收。4200 运行实例不动。

后续等待策略、发送授权和首次发送边界见 `2026-10-28-provider-id-wait-policy.md`。

## 实现与权限边界

- `ProviderAliases` 增加按原名/对应名选择当前快照中唯一存在身份的能力。
  不猜测品牌、preset、URL 或大小写；两名同时存在即拒绝，即使其中一个被禁用。
- 本地存储复用现有 `model_provider_aliases`，一次有界查询读取并验证不可变映射
  快照。保留 256 项上限、257 行哨兵和完整图校验，不增加第二套映射表。
  Key 的同事务权限扩展继续使用相同读取器，None/空限制语义不变。
- 将等待候选匹配提取到 `stage_send/recovery_wait/identity.rs`，同时检查请求原始
  路由快照与当前快照。登记了映射但路由仍未切换时继续匹配旧名；切换后只匹配
  同一个显式 credential ID，后续再次刷新仍能识别原始快照中的归属。
- 改名必须在两个快照中都声明同一个显式账号 ID；不推导或重建
  `provider::default`，不选择另一个账号，不复制候选。上游模型、adapter、
  protocol profile/family 必须保持匹配，禁用账号与当前账号组限制继续生效。
- 数据库/投影账号不归本地路由改名逻辑所有，涉及映射时拒绝，不擅自改写账号或
  配额投影。没有映射时保留原有外部候选行为。
- 刷新后仍重新执行本地 Key 授权，成功才替换上轮已拆分的 Key fallback 权限。
  不修改原现金计费快照、不重建请求预算、不增加预留、不执行真实发送。

映射快照不等于路由与 SQLite 原子发布。未来协调器仍必须证明两名是同一主体、
保留旧 ID、禁止独立复用，并处理并发版本与恢复日志；本轮没有新增迁移入口。

## 已验证

所有 Cargo 验证 locked/offline、jobs=2、test-threads=2，TEMP/TMP 指向 linshi。

- 新增候选刷新规则 4/4：改名成功与二次刷新、未发布时旧路由保持、无映射/双名
  并存/隐式账号拒绝、不同账号/模型/adapter/协议/账号组拒绝、投影账号不被改写。
  这是直接调用刷新函数的合成快照测试，不是真实睡眠中切换实例的端到端测试。
- `--lib pipeline::`：122 passed / 0 failed / 1 ignored，包含上述 4 项以及前两轮
  fallback 来源与网络故障回归。ignored 项仍需要独立 Redis。
- `--lib local_runtime::`：38/38。新增映射快照不随后续写入改变，畸形注册表拒绝
  新读取，同时不改变已捕获快照的隔离 SQLite 用例。
- `--lib provider_identity::tests`：5/5，新增原名/对应名唯一选择和大小写用例。
- `--test access_key_groups_contract recovery_wait`：5/5，本地回环验证未改名请求
  的 JSON/SSE 恢复、截止时间、禁用/账号组撤销和 HTTP 断连，非完整迁移验收。
- checker 31/31、ratchet、scoped rustfmt、`git diff --check` 通过。
  既有 unused 等编译 warnings 未自动修复。

## 代码规模与剩余工作

有效行：provider_identity 52→72，规则测试 61→78，本地读取器 24→31，
本地别名集成测试 187→214，recovery_wait 267→238；新增候选匹配模块 115 行、
测试 164 行。不修改 baseline，不覆盖其他线程工作。

尚未接齐：进入等待前的旧 ID 策略查找、队列/attempted-provider 身份归并、路由
policy 与 fallback 的完整身份一致性、计费/审计/压力/refill 引用，以及统一发布、
恢复协调器和部署/降级保护。当前旧 EXE 不会自动获得这些源码改动。

没有迁移真实数据，没有上游生成调用，没有交付或替换 EXE，不登记整体迁移完成。
