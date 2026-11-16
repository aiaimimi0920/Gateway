# Copilot Responses 额度与在途取消退款

本块只修改测试，不修改生产逻辑或公共路由状态。复用上一块的真实 Local Key 保存、
stage_filter 预扣、SDK sender 与 finalizer，补充以下缺口。

## 覆盖

- 成功／SDK 拒绝矩阵扩展为 Chat／Responses × message_prepaid／token_prepaid ×
  成功／拒绝，共 8 个执行场景。Responses 通过真实 normalizer 构造，不仅修改 endpoint
  标签；保持未知 usage／上游状态，成功保留既有扣款政策，拒绝退款。
- 新增两项独立进程测试：message + Chat、token + Responses 在实际 sendAndWait 后
  触发 caller cancellation。要求及时返回、取消审计、单次发送、余额恢复、无现金账单。
- 退款后使用真实 Key 保存接口将额度类型改为 unlimited。Local 在有 pending hold
  时禁止模式切换，因此切换成功同时证明 reservation hold 已释放，不只检查余额数字。
- 未确认停止的工作目录保留，文本槽保持隔离；测试进程必须独立，仅串行线程不足以
  隔离进程级 semaphore。子进程是否退出另外取得 Windows PID 回执。

## 边界

取消仅覆盖上述两个组合，不是两个协议与两种额度的完整四组合。未知 token 保留估算，
不是实际用量。仍是可信内部 gate 的 Local 测试，不是公共 HTTP 成功路由、现金支持、
启动失败、优雅关闭或重启恢复验收。真实账号／CLI、原生 EXE 与 Server 继续单列。

## 回执

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-prepaid-cancel-20261113`。
`matrix.log`：3 passed / 0 failed / 0 ignored，包含扩展的 8 个执行场景，以及原有
零额度／现金拒绝回归。构建 10m10s（包含原生依赖编译），执行 2.88s。
两个独立测试 EXE 进程分别执行取消测试，各 1 passed，见 `message_chat.log`、
`token_responses.log`；没有将两项取消测试放进同一进程。

共 10 个 child 经 Windows PID 核对退出。矩阵 8 个 child 各一次发送、一次 shutdown；
两个取消 child 各一次发送、零 shutdown RPC，不冒称优雅关闭。对应回执为
`matrix-process-exit.json`、`message-chat-process-exit.json`、`token-responses-process-exit.json`。

checker 31 项、ratchet、本轮源码 rustfmt 与 scoped diff 检查通过；源码 UTF-8 无 BOM。
全仓 `cargo fmt --all -- --check` 未通过：另一工作线的 3 个 Luma MCP contract 测试文件
存在格式差异，见 `formatter.log`。没有改动这些文件；本轮定向结果见 `formatter-scoped.log`。
prepaid 文件有效行数 173 → 194，新增 interruptions 111；未改 baseline。源码／测试
二进制哈希保存在 `source-final.json`、`binaries-final.json`。
没有提交、推送、出包、部署或真实上游请求，不增加整卡完成数。
