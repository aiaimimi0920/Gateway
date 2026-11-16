# Nous Portal 专属控制台模型测试

后续已补单次／临时／命名／账号保存策略的管理入口本地证据，见 [管理测试入口验收](2026-11-12-nous-console-management-probe.md)；下文未完成边界保留为本批当时状态。

## 本批范围与机制

Nous 的显式模型测试此前落入通用 OpenAI 裸 HTTP 发送路径，未复用产品生产 sender 的目标、身份头和 redirect fence。现改为专属非流式 Chat 测试，Subscription／Welcome 均使用选中凭据的固定目标及 access token。

- `src/protocol/nous_portal/probe.rs` 使用现有 Chat 请求计划与 `send_profiled_plan`，不引入独立 HTTP client、OAuth client 或刷新锁。
- 默认无测试参数、禁用凭据、非配置模型／别名不发送；模型别名和账号范围沿用现有控制台模型选择。
- 64 KiB 完整响应上限、唯一 JSON 键、已有 Chat admission、单条非空 `stop` 文本；拒绝 tool/refusal、截断和超过 8192 字符的答案，不截断后伪装成功。
- access token 在答案进入预览、评分和持久化前脱敏；缺失 usage 保留未知，不补零。
- 60 秒超时、取消安全 audit 和 health 复用现有 owner；HTTP 错误不重放，不把原始响应正文放入结果。
- `console_model_probe.rs` 原通用 `generate`／`failed` 原样提取为 `openai_model_probe_response.rs`，避免分发文件超过 500 有效行；不改变其他产品通用响应行为。

## 验证

Rust 定向命令：`cargo test --locked --offline --lib -- nous --test-threads=2`。最终 **64 passed / 0 failed / 0 ignored**，其中新增 7 项；测试执行 25.24 秒，完整命令 1031.39 秒（包含共享 Cargo 锁等待与编译，不作为纯编译耗时）。

首轮为 59 passed / 5 failed：新 probe 向生产 finalizer 传入空 headers，被 Nous 的精确 Authorization 校验在发送前拒绝。已修正为使用 Nous 自有 `runtime::headers(payload)`，未削弱 finalizer 或修改测试期望；首轮失败日志保留为 `rust-r1.log`，复验使用 `rust-r2`。

新增 7 项用例覆盖两个目标的成功／精确账号／别名、答案脱敏和真实用量、缺失用量、错误响应与边界、HTTP 错误不重放、取消释放响应，以及默认／禁用／非配置模型和外来目标／身份的发送前拒绝。现有 Nous fixture 使用 LocalRuntime 和合成 loopback 上游，不访问真实 Portal。

已通过有效行 checker（31 项）、ratchet、scoped rustfmt 和 `git diff --check`。8 个 Rust 文件的复验前后 SHA256 一致、UTF-8 无 BOM；通用 `generate`／`failed` 提取内容除可见性外逐字等价，已封存 `final-validation.json` 与基于本轮修改前快照的 `scoped.diff`。

有效行：控制台分发 500 → 418；提取模块 93；Nous runtime probe 64；协议 probe 64；测试模块 206。无新增超限文件，没有修改例外／baseline。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-probe-20261112`，包含本轮修改前快照、源码哈希、各命令日志及 receipt。

## 未完成边界

本批不修改 durable refresh owner，不在模型测试中消费 single-use refresh token，也不进行 401 隐式刷新／重放。选中快照后的并发授权轮换、完整管理计划入口、自动调度、原生 UI／EXE、Server／Redis 和真实独立 client 资格／账号／订阅／费用仍须分别验证。通用 OpenAI 响应提取不等于重跑 NVIDIA 集成验收。

这只关闭 Nous 控制台模型测试的本地发送与回复校验缺口，不关闭 Nous 整卡；全范围仍为 67 个工作项未完全关闭。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。
