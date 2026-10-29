# 服务商 ID 在线迁移：等待策略、发送授权与首次发送边界

承接 `2026-10-28-provider-id-wait-candidate.md`。本轮只开发和隔离验证；4200 原实例、
真实配置与数据库不动，迁移入口仍未开放。

## 改动与边界

- 将上一轮同账号归属验证提取为共享 binding。等待策略也必须通过原快照/当前快照
  的唯一身份及显式账号匹配，不能仅凭新 provider 名称获得等待权限。
- 有映射时读取当前有效 provider 的策略。发生改名时按更严格的旧/新模式计算：
  任一端 disabled 则关闭，任一端 bounded 则有界，只有两端都允许才长等待；
  初始化时采用较短的显式 timeout。循环内不重新初始化或延长原请求预算。
  没有映射时保持原来的精确 ID 查找，包括找不到 provider 时关闭等待。
- 请求保存经过验证的身份快照。发送阶段的 provider allowlist、显式 fallback
  授权和“是否尝试过其他 provider”均按同主体比较；不会授权别名表中的其他主体。
  空/未限制 route policy 仍不构成显式跨 provider fallback 权限。
- 已尝试 provider 数组保持实际观察到的 ID，不重写审计历史、不清零次数；原始
  计费快照、投影与配额预留不修改。Key 撤销/组过滤仍在候选刷新后重新检查。
- 补充路由完成后、首次发送前已发生改名的情况：在请求预算包装内，仅刷新需要
  改名的既有候选，不重新路由、不扩大候选、不重做预留，避免带旧候选进入首次 IO。
- 队列中已有 waiter/turn 的归属与 permit 没有迁移，未合并队列、压力或熔断计数。
  SQLite 映射与 route snapshot 仍非原子发布，不能据此启动实际在线迁移。

## 测试过程

新增四项等待策略用例，覆盖旧名找新策略、双名/隐式/投影账号拒绝、较严格模式与
timeout、旧/新 disabled、未发布与无映射情况。新增三项发送授权用例，覆盖双向
同主体授权、无关主体隔离、空权限语义与原始尝试历史保留。

新增隔离 route→映射/配置发布→send 测试：仅在临时 SQLite 注入测试别名，验证
首次发送前已刷新为新 ID，账号 ID 不变，allowlist 旧 ID 仍匹配同主体，实际发送
数为 0，最终收到 `gateway_network_unstable`。这不是生产迁移操作示范，也不是
成功生成或长流式中途迁移验收。

首轮 pipeline 为 128 passed / 1 failed / 1 ignored。失败的暂停时钟用例没有得到
预期 deadline；新读取涉及真实 SQLite worker IO，不能依赖虚拟时钟自动前进来
等待该 IO。改用真实 1 秒请求预算和 3 秒测试保护上限，保留 deadline/零发送断言。

第二轮 129 passed / 1 failed / 1 ignored。所有业务断言通过后，新测试的清理因
目录名称不符合 `gateway-local-` 安全前缀而拒绝。修正测试目录名，没有放松安全
检查。失败测试遗留目录如下；显式清理命令被工具策略阻止，未换工具绕过：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-identity-before-send-1320398d-2d87-4cb4-a14a-3346cdb41934`

该目录仅包含本轮合成测试数据；不读取或发布其 seal 文件。

最终验证（locked/offline、jobs=2、test-threads=2，TEMP/TMP 为 linshi）：

- `--lib pipeline::`：130 passed / 0 failed / 1 ignored；ignored 项需要独立 Redis。
- `--lib local_runtime::`：38/38。
- `--lib provider_identity::tests`：5/5。
- `--test access_key_groups_contract recovery_wait`：5/5，最新源码的本地回环 JSON/SSE、
  截止时间、禁用/账号组撤销和 HTTP 断连回归通过。
- checker 31/31、ratchet、scoped rustfmt、`git diff --check` 通过。

策略合并的模式/timeout 收紧用于首次预算配置。已经等待中的请求不会因改名而
重新初始化或延长预算，当前策略改为 disabled 会退出等待；已有长等待动态改成
更短 timeout 的即时收紧仍需预算 owner 接齐，不能把本轮测试当成该能力的证明。

## 规模与后续

有效行：provider_identity 72→75，pipeline/mod 402→404，fallback_authorization
70→152，stage_send 485→475，recovery_wait 238→260，identity 115→148。
新增策略模块 47 行、策略测试 80 行、首次发送集成用例 82 行；egress 测试 410→412。
保留其他线程改动，没有更新 baseline 或自动修复无关 warnings。

后续仍需队列/压力/熔断身份协调及映射快照的等待内存记账、初始路由持久化策略
引用、计费/审计/refill、统一发布与恢复协调器、旧版本并发和降级保护。没有新 EXE
交付，没有真实上游调用，不登记整体在线迁移完成。
