# Copilot buffered 执行接线

本块将冻结身份与 owned SDK 文本执行接入 pipeline 的专用 buffered 分支；compiled SDK
credential 仍无条件禁路由。此处不是公共 API、真实 CLI、订阅费用或整张 Copilot 卡片验收。

## 执行与错误边界

- buffered 入口先运行严格单轮文本准入；不把 SDK 请求交给通用 HTTP UpstreamClient。
- 在 ProviderAttemptGate 内执行；固定零重试，同时以 Option 消耗实际执行 future，防止
  同一 generation 被再次轮询。SDK 错误只输出固定安全错误，不携带原始 Debug/source 链。
  禁止 retry/fallback，native_transport=None，避免 HTTP connect-recovery 隐藏重放。
- 复用现有 caller-owned cancellable send，不创建 detached generation 或 cleanup task。
  启动前、启动后、发布前复核身份与 inflight eligibility；不把时点栅栏当跨 RPC 锁。
- 启动上限 15 秒；session/生成/stop 包络 160 秒，SDK wait 上限 120 秒；授权失败后的
  stop 上限 40 秒。现有外层 RequestBudget 可提前取消整个执行。
- canonical 保留未知 usage、上游 HTTP status、finish_reason 与 model declaration。
  最后消息 output_tokens 不拼成总 usage，不作为计费证据。公共 packer 的完成状态投影
  不等于 SDK 原生 finish reason。streaming 分支明确拒绝，不将最终文本伪装为实时 SSE。

## 有界进程与目录

文本执行使用进程级单名额、try_acquire 无等待队列。首次 polling startup 前即标记为
关闭未确认；startup 可能已经 spawn 但尚未返回 owner，不能等 start 返回后才标记。

仅明确 stop 成功后释放名额、清理本次随机目录。文本执行失败但 stop 成功也会释放；
startup 错误、超时、future Drop 或 stop 失败均保留目录，并使文本名额隔离至 Gateway
进程重启。这是保守的准入熔断，不是进程回收器，也不证明被取消的 child 已退出。
它只覆盖文本执行，不改变管理目录的独立 admission/worker。该可用性取舍仍须在公共
路由启用前评估；本轮没有新建 OS killer/reaper 与 SDK 争夺生命周期所有权。

## 验证

本轮证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-dispatch-20261113`。
根库 locked/offline、runtime-defaults,copilot-sdk-runtime、jobs=2、gateway test debug=0
编译通过。`focused.log` 先记录普通筛选 1 passed / 6 ignored，随后复用此次生成的
`neuro_gateway-d289f28c03903e62.exe` 显式 `--include-ignored`，
`dispatch-integration.log` 最终 **7 passed / 0 failed / 0 ignored**。
编译回执总计 21m01s，包含共享 Cargo 锁等待及 native 依赖构建，不作为纯 Rust 增量耗时。

六个合成 child 全部通过 Windows OpenProcess 退出核对，见 `process-exit.json`。
本次均已退出，返回 ERROR_INVALID_PARAMETER(87)，没有将 future Drop 当作退出凭据。
方法级审计确认：启动取消和启动后撤销均零 session.create／零 send；生成取消、拒绝、
正常完成、发布前撤销各一次 send，没有重放。两条取消路径没有 graceful shutdown，
目录保留、名额隔离；其余确认 stop 后清目录并释放名额。

受改动合成 fixture 影响的独立 runtime 回归 **11 passed**，21 个 child 均核对退出，
见 `runtime-regression.log`、`runtime-process-exit.json`。checker 31 passed、ratchet、
官方 rustfmt、scoped diff、UTF-8 无 BOM 检查通过。未修改 baseline 或依赖版本。

有效行：pipeline facade 363→364、buffered 396→417、streaming 243→248；
dispatch 71、runtime 113、主测试 173、启动测试 82；合成 fixture 116→130。
源码 SHA256 保存在 `source-final.json`。

合成测试直接使用真实 LocalRuntime/冻结身份、attempt gate 和 owned synthetic EXE，
但测试 gate 不设置正式 eligibility，以便在不解除实际禁路由的情况下验证执行内核。
外部夹具测试明确 opt-in，普通 feature 测试不要求特殊环境；本轮需显式执行 ignored 项。
测试不会访问真实账号或外部服务。SDK 仍固定 1.0.31，本轮没有联网核验新版本。

独立只读审查确认了 startup 前 arm 和终止错误路径；发现外部夹具测试最初未标 opt-in，
已在编译前修正。本轮 runtime/gate 测试未直接覆盖 sender 的一次性 Option 闭包或公共
HTTP 路径，不把内核测试扩大解释为这些入口已经通过。

## 仍未完成

公共 HTTP 入口的管理保存到推理全链、正式 eligibility/审计/计费的集成证据、两账号
并发隔离、真实 CLI 与账号、原生 EXE/Server 验收仍待推进；不增加整卡完成数。
未提交、推送、出包、部署或修改运行实例。
