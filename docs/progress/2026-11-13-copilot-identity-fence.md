# Copilot 冻结执行身份与 Local Key 复核

本块提供 `pipeline/copilot_identity.rs` 的内部身份原语，尚未接入公共 sender，也没有启动
SDK 或解除 compiled credential 禁路由。不把身份测试当作公共推理、生命周期或计费完成。

## 绑定与复核

- 仅接受已认证 PipelineContext 的 Local Key、本地配置账号和冻结 `cash_route_snapshot`。
  拒绝 PG、Hosted、DB credential ID、缺失快照／显式 credential，以及非专用 Copilot provider。
- 模型和过滤后 prompt 通过上一块单轮请求契约；provider／credential 从内部 candidate 取得。
  不从 caller headers、metadata 或 session hints 构造身份，不把 token 放入通用 payload。
- 身份对象借用上下文／candidate，保留 immutable snapshot 的 Arc，不派生 Debug／Serialize。
  token 只从绑定的快照按精确 provider／credential ID 读取；检查禁用与过期状态。
- `token_for_send` 重新校验 Local Key 的 active／revoked／expiry、endpoint scope、model、
  provider identity aliases、Key 账号组与请求账号组限制，再复核 route revision、共享预算、
  shutdown／draining 和当前 caller-cancel 状态。错误不重试、不替换身份进行 fallback。
- 本切片仅是时点栅栏：并非跨 RPC 的事务锁，不取消已经抵达上游的请求，不替代 attempt gate
  的计数／配额／上游资格，也不是持续监听取消的任务 owner。

## 同一 Local Key 权限版本

独立只读审查发现初稿分别读取 metadata 与 model/provider 授权，会把并发更新前后的
不同授权版本组合起来。已改为复用 `authorize_local_candidates_with_metadata`：在原 policy
owner 的同一事务中读取一次 key，复用现有有效性、model/provider 和 alias 检查，并返回
同次读取的 metadata 用于 scopes/groups。没有新建认证存储或平行的 restriction parser。

原 `authorize_local_candidates_with_fallback_authorization` 保持原签名与返回语义，只包装
新方法取 provider grants；其现有调用方不需要改动。独立复核确认该混合版本缺口已消除。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-identity-20261113`。
定向最终累计 **10 passed**（8 项新身份测试、2 项已有 Copilot 配置测试）：
`focused-r2.log` 的 9 项成功保留，修正删除账号夹具后仅重跑受影响用例，
`rotation-r3.log` 为 1 passed / 0 failed / 0 ignored；不是一次全绿的十项回执。
Local Key 相关回归 **20 passed / 0 failed / 0 ignored**，见 `local-key-regression.log`。
复用本轮成功编译的 `neuro_gateway-391168db0cd875e0.exe` 执行该回归，之后仅改测试夹具，
未更改生产代码。根库测试编译为 locked/offline、runtime-defaults、jobs=2、gateway debug=0。

首次失败均来自夹具清理目录缺少既有安全前缀，修正为 `gateway-local-`，未放宽清理器。
第二次剩余失败来自删除 credential 却保留账号组引用，被配置校验正确拒绝；夹具同步清除
引用后通过。没有修改生产逻辑去迁就测试，也没有宣称失败回执通过。

官方 rustfmt、checker 31 项、ratchet、scoped diff 和 UTF-8 无 BOM 检查通过。

新增测试使用真实隔离 SQLite LocalRuntime、合成 token 和现有配置存储，覆盖 token
同 ID 轮换／删除／禁用、Key 撤销／禁用／过期、动态 scope/model/provider/group、缺失或
不可信上下文，以及取消／shutdown／预算。并发回归在两个各自不完整的授权版本之间
切换 32 次、读取 64 次，全部必须拒绝；它不是所有调度交错的穷举证明。
夹具仅为身份测试构造 candidate，明确断言实际路由队列为空、compiled SDK credential 仍禁用。

有效行：pipeline facade 362 → 363；原 Key policy 254 → 264；新身份模块 161，
fixture 116、主测试 154、负例 73。未改 baseline、依赖或存储版本。

## 接线剩余责任

在实际 attempt gate 内调用身份复核；将 SDK 启动／执行／stop 保持在现有 caller-owned
cancellable send future 中，不能移到 detached generation task；长 await 后复核，明确关闭
SDK 非 HTTP 错误的自动重放路径。还需 bounded runtime／目录清理、canonical 未知用量、
审计及公共 API 合成集成测试；完成这些后才评估解除禁路由。
