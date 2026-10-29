# 服务商 ID 在线迁移：等待队列身份协调

承接 `2026-10-28-provider-id-wait-memory.md`。本轮仅源码及隔离验证，4200 原实例、
真实配置、账号与 SQLite 不动，不开放实际迁移入口。

## 改了哪些内容

- 等待队列增加有界、进程级的身份登记。读取已验证映射后，在队列互斥锁内校验
  完整并集并发布；旧快照只能补充，不能删除已保留名称。改向、合并、链及超过
  256 对的累计映射均拒绝。
- 旧／新名统一到原始名称 owner。若只有一侧已有队列，保留该侧的原 FIFO 锁与
  计数，仅调整索引；既有 lease 不重新入队，新请求共享同一 provider 限额和锁。
- 如果两侧已经存在独立队列，返回 `recovery_wait_identity_conflict`，不改任何
  映射、计数或锁。不能安全拼接两个已排队的 Tokio mutex；未来发布协调器必须
  预先防止这个状态，而不是在这里丢弃请求或猜测全局顺序。
- 修复锁持有句柄比 lease 活得更久时可能过早移除 owner 的边界。lease 与 turn
  共持一个 RAII 记账对象，最后一个引用释放才减计数；turn 先解锁，再释放记账。
  Drop 在同一个队列锁内按当前映射找到 owner，改名前创建的句柄不会漏减计数。
- 恢复等待在策略绑定验证后、入队前协调身份。读取旧快照的并发请求也由登记表
  解析名称，不能在映射登记后再创建另一套队列。没有修改凭证 ID 或授权集合。
- 只读交叉核验发现睡眠前与醒来后刷新读取的映射可能不同。已在候选刷新完成
  绑定及 Key 重新授权之后，使用同一份新快照再次协调；成功后才更新请求授权
  状态及候选。双活冲突不能绕过检查进入恢复发送，首次发送前刷新也复用此门禁。

登记表是进程内有界状态，不是持久化迁移日志，也不承担整套路由配置发布事务。

## 验证

新增六项回归覆盖：旧／新任一侧已有队列时复用锁及共享32项限额；双活冲突原子
拒绝；旧快照／重复发布／改向；改名时 FIFO 和取消；turn 跨越 lease 存活；累计
映射上限。测试只使用合成 ID、进程内锁和队列，不调用上游。

另扩展临时 SQLite 的 route→映射发布→send 回归：先建立两套独立等待 owner，
再发布映射，新候选刷新必须返回 identity conflict、原候选不替换、attempt count
为0。它覆盖共用刷新门禁，不是定时睡眠竞态或真实 HTTP 零连接验收。

首轮编译因原有取消测试的 `unwrap_err()` 要求新 WaitTurn 实现 Debug 而失败。
已改用 `matches!` 保留对 JoinError 取消状态的断言，未增加无必要的生产 Debug。
只读复核确认后置快照检查修正覆盖所指出的窗口，未重复执行测试。

最终定向验证使用 `--locked --offline --jobs 2 -- --test-threads=2`，TEMP/TMP 指向 linshi：

- `--lib request_wait::`：15 passed，包含新增6项队列身份／生命周期测试。
- `--lib pipeline::`：133 passed，1 ignored（需要独立 Redis），包含新增候选刷新冲突回归。
- `--lib provider_identity::tests`：6 passed。
- `--lib local_runtime::`：38 passed。
- 共192项 Rust 测试通过；checker 31/31、scoped rustfmt、ratchet、`git diff --check`
  通过，未改 baseline。没有运行全量测试、真实 HTTP 流式迁移验收或 EXE 构建。

收尾只读核验：127.0.0.1:4200 仍为 PID 42912，LAN 转发仍为 PID 4280，
healthz／readyz 均 HTTP 200。未启动第二个共享真实数据库的实例。

有效行：provider_identity 92、queue 161、queue/identity 37、identity_tests 143、
recovery_wait 282、route identity_wait 112。新增模块均低于250行。

## 后续边界

此处仅统一恢复等待队列，不代表发送并发、压力、模型熔断、计费审计等引用全部
完成迁移。统一发布／恢复协调器、旧版本并发保护、等待预算动态缩短及不停服务
实例接替仍未完成。没有实际迁移或 EXE 交付，不能登记整体在线迁移完成。
