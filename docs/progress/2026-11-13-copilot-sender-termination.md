# Copilot sender 失败与调用方取消

本块补充真实 sender 入口的失败／取消组合测试，不解除公共路由限制，不调用真实账号。
新增 `stage_send/copilot_tests/termination.rs` 及测试模块注册，并修复运行中发现的失败审计问题。

## 发现与修复

首轮 `termination.log` 编译通过，但在 SDK denial 的审计断言失败：本地
`GatewayError::service_unavailable` 的 503 被 `provider_response_status` 当作上游状态。
这是生产失败路径缺陷，不是放宽测试。唯一 child 确认退出，保留
`failed-r1-process-exit.json`。

现在 SDK sender 在 polling 执行前设置 `observed_upstream_status=Some(None)`，失败审计
优先使用该三态。未观察状态的旧 HTTP 失败仍使用原有 error 状态；零 attempt 仍无上游
状态。错误本身没有改写，客户端收到的 Gateway 503 保持不变。新增三态兼容测试与实际
错误 IntoResponse 的 503 断言。归档失败输入复用同一状态值；没有数据库格式变更。

## 验收路径

测试通过可信内部测试 gate，调用真实 `recovery_wait::send_cancellable`：

1. 合成 SDK 返回含敏感标记的拒绝。即使 caller retry policy 为 3，也只能发送一次；
   返回 Stop／Abort，错误不携带 token、不允许 recovery wait，失败审计保留未知用量和状态。
2. 下一请求能抵达 generation，证明上一次已确认 stop 的失败释放文本槽。观察到实际
   `session.sendAndWait` 后触发 caller watch，要求 30 秒内返回，早于 SDK 生成超时。
   请求预算记录 caller_cancelled，真实失败 finalizer 写 cancelled 审计，不重放。
3. 取消后的新请求仍被拒绝，合成发送总数保持 2，保留一个未确认关闭的工作目录。
   这是保守槽隔离，不是自动恢复或 graceful shutdown。

## 验证边界

- 这是 sender 与失败 finalizer 的组合验证，不是公共 HTTP 成功路由或完整外层 budget
  wrapper 的验收；实际 compiled credential 继续禁用。
- 使用默认合成 Local Key，直接进入 sender，未执行 quota admission；余额字段保持只
  证明该测试路径未额外修改余额，不代表 token／cash 收费 Key 的预扣、退款或结算通过。
- background-task join 不证明 SDK child 退出；子进程退出必须另取 Windows PID 回执。
- 取消故意耗尽进程级文本槽，因此测试标为 ignored，必须使用独立测试进程和全新证据根。

## 回执

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-termination-20261113`。
最终 `termination-r2.log`：2 passed / 0 failed / 0 ignored。构建 15m50s，包含共享锁
等待与原生依赖，不能当作纯增量耗时。组合测试覆盖 denial、在途 caller cancellation
和隔离拒绝；另一个测试覆盖失败审计三态及旧 HTTP 兼容。

`termination-r2-process-exit.json` 核对两个 child 均退出，各一次会话创建与发送。
拒绝场景有一次 runtime.shutdown；取消场景没有 shutdown RPC，属于 Drop 后退出，
不是优雅关闭。Gateway 仍保留取消场景目录和隔离状态，不因外部测试回执自动解除隔离。

使用同一新 EXE、另一个独立进程与全新 probe 根执行既有 sender 回归：6 passed，
覆盖 Chat／Responses 成功、未知状态和用量、公共拒绝及真实 eligibility；两 child
均核对退出。finalizer 回归 16 passed。见 `sender-regression.log`、
`sender-regression-process-exit.json`、`finalizer-regression.log`。

checker 31 项、最终 ratchet、formatter 和 scoped diff 检查通过。有效行数：测试注册
文件 186 → 188，新增 termination 151，buffered 432 → 433，audit_persistence
200 → 201；未改 baseline。源码 UTF-8 无 BOM 与 SHA256 留于 `source-final.json`，
测试 EXE／合成 EXE 哈希留于 `binaries-final.json`。

没有出包、提交、推送、部署或真实上游请求。不增加整卡完成数；收费 Key、公开成功
请求、真实 CLI／账号、原生 EXE 与 Server 验收仍开放。
