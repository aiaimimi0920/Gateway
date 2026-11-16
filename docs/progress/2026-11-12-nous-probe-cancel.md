# Nous 在途轮换后取消验证

## 本批完成

在前批在途成功／401 测试基础上，新增“旧推理请求已经抵达上游，持久刷新发布新凭据，随后取消旧调用”的本地验证。生产逻辑没有修改，继续使用现有取消安全审计和健康反馈 owner。

测试夹具将首个响应的等待点从 handler 移到响应体；semaphore 控制释放，响应体 Drop 发出关闭通知。取消用例从不发放释放许可，因此关闭通知不是模拟成功响应结束，也不靠固定 sleep 判断网络释放。后续请求不等待首个请求的许可。

新增用例验证：

1. 上游确认收到旧请求后才执行真实本地 durable refresh，并在 grant Published 后取消调用。
2. 丢弃调用 future 后，上游持有的响应体被释放；等待有 5 秒上限。
3. 取消审计最终落为 `cancelled`，用量及上游状态未知；没有写入模型健康行。审计最终完成采用有界轮询，不假定异步 finalizer 已同步落盘。
4. 取消不撤销已发布的刷新结果；从原 route snapshot 重新选择能获得 Subscription 目标和新 token。
5. 新凭据后续调用成功，健康反馈为 `Applied`；旧、新两次请求各使用自己的 Bearer，总计两次推理和一次 refresh，无重放或额外消费单次 refresh token。
6. 最终审计恰为一条取消和一条成功／5 tokens，均不泄露旧、新 access token 或 refresh token。

这证明本地取消能够释放测试中的在途响应体，不保证真实上游撤销已经接受的生成或退还费用，也不证明客户端在轮换前已读取响应头。

## 验证

`cargo test --locked --offline --lib -- nous_probe_ --test-threads=2`

**5 passed / 0 failed / 0 ignored**：新增取消组合 1 项，回归在途成功／失败 2 项与发送前轮换／禁用 2 项。测试执行 1.02 秒，完整命令 984.20 秒，包含共享构建锁等待和编译。首轮通过；未重复全 Nous 或全仓测试。

有效行 checker 31 项、ratchet、scoped rustfmt、scoped diff 检查通过。共享夹具模块 197 → 215 有效行，新增取消测试 124 行；未修改 baseline／例外。测试前后 Rust 文件 SHA256 一致，UTF-8 无 BOM；新增文件使用本批 before 快照核对。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-probe-cancel-20261112`，含源码快照、命令回执、日志、scoped.diff、final-validation.json。

## 剩余边界

本批只完成该本地在途取消组合，不穷举所有轮换／取消时序。真实账号、独立 client 资格、订阅费用、原生 UI／EXE、真实进程重启及 Server／Redis 验收仍开放。全范围仍为 67 个工作项未完全关闭，不关闭 Nous 整卡。

未读取真实凭据，未提交、推送、出包、部署或修改运行实例。
