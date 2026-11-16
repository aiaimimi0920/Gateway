# Nous 模型测试在途授权轮换验证

后续：[在途轮换后取消验证](2026-11-12-nous-probe-cancel.md)已补本地响应体释放、取消审计与新凭据后续调用证据；下文保留本批历史范围。

## 本批结果

补齐旧推理请求已抵达上游、durable refresh 随后发布新凭据、旧响应最后返回的本地竞争证据。现有 `model_probe_health::finish` 会重新核对当前凭据绑定并返回 `StaleConfiguration`；成功／失败仍按本次请求实际结果写审计。本批未发现需要改动生产代码的缺口，仅新增测试和调整测试夹具传入 HTTP client 的方式。

新增 `src/token_refresh/tests/nous_probe/inflight.rs`，使用随机 loopback 上游与 semaphore 控制响应释放，不依赖 sleep 猜测时序、不访问真实上游。两项用例分别返回旧请求的成功响应和 HTTP 401，均验证：

- 请求抵达后才启动真实本地持久 refresh；确认 grant 为 Published 后才释放旧响应。
- Welcome 旧请求仍携带旧 token，不因轮换重放；新选择的凭据为 Subscription 和新 token。
- 旧请求按成功／失败记录审计，用量分别为 5／未知；两者健康反馈均为 `StaleConfiguration`，不写入模型健康行。
- 重新选择当前凭据后的新请求通过，健康反馈为 `Applied`。总计两次推理、一次 OAuth refresh，没有重复消费单次 refresh token。
- 两条审计不包含旧、新 access token 或原 refresh token。

测试初始健康状态为空，证明旧结果不会创建健康记录，并显式检查 stale 判定；不将其描述为对所有已有健康行、所有跨存储发布时刻的穷举证明。

## 验证与修正

最终命令：`cargo test --locked --offline --lib -- nous_probe_ --test-threads=2`。

**4 passed / 0 failed / 0 ignored**：新增在途成功／失败 2 项，加上发送前轮换／禁用回归 2 项。测试执行 0.88 秒；完整命令 1489.28 秒，包含共享 Cargo 锁等待和编译，不是纯编译耗时。本批未改变生产逻辑，因此未重复全部 Nous 或全仓测试。

首轮编译通过，但两个新用例触发 Windows 测试线程 `STATUS_STACK_OVERFLOW`，未完成断言。已对测试内独立的大型推理／刷新 future 使用 `Box::pin`，减少嵌套 future 栈占用；没有修改生产行为、提高测试线程栈配置或放宽断言。`rust-r1` 失败日志与 `rust-r2` 成功回执均保留。

有效行 checker 31 项、ratchet、scoped rustfmt、scoped diff 检查通过。既有 `nous_probe.rs` 177 → 178 有效行，新模块 197 行，未修改 baseline 或例外。Rust 文件测试前后 SHA256 一致，UTF-8 无 BOM。新增文件也用本批 before 快照检查，不能只依赖 Git 对未跟踪文件的 diff。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-probe-inflight-20261112`，含失败／成功日志、源码哈希、本批 before 快照、scoped.diff 和 final-validation.json。

## 边界

本批覆盖本地请求已发送后、响应返回前完成轮换的成功／失败情形，不宣称网络发送与配置发布原子化，也不撤回已发送请求。轮换与取消组合、真实账号／独立 client 资格／订阅费用、原生 UI／EXE、真实进程重启、Server／Redis 验收仍未完成。

全范围仍为 67 个工作项未完全关闭，不据此关闭 Nous 整卡。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。
