# Nous 模型测试发送前凭据轮换保护

后续：[发送中轮换的成功／失败反馈](2026-11-12-nous-probe-inflight.md)已补本地定向证据；下文保留本批当时的验证范围。

## 实际缺口与修复

控制台／测试计划捕获的 CredentialProbeTarget 是 payload 快照。durable refresh 发布新 access token／inference target 后，已有 model_probe_health 会将旧身份反馈视为 stale，但此前 Nous probe 仍直接用旧 payload 发送请求；健康反馈拒绝不等于发送被阻止。

本批仅在 `src/provider_runtime/nous_model_probe.rs` 的异步审计初始化之后、请求发送之前，复用现有 `model_probe_identity` 校验选定凭据与当前配置／运行时覆盖是否一致。失配返回明确 `credential binding changed; call not sent` 失败结果，审计保留失败和未知用量，沿用已有 stale health 处理。不会擅自改用另一账号、另一订阅目标或新 token，不会创建刷新 owner 或隐式重试。

选择拒绝而非静默重捕获，是为了保留本次测试已选账号与目标的含义。后续重新选择当前凭据可正常发送；自动调度的后续轮次仍由既有 target 选择负责。

## 本地证据

新增两项测试复用既有 durable refresh fixture、真实本地配置 owner、LocalRuntime 和隔离 loopback 上游：

1. 捕获 Welcome 旧凭据后，通过真实 refresh 流程收到并发布新的 Subscription 目标和 access token，确认 grant 为 Published。旧快照测试失败且无上游调用；从已持有的 route snapshot 重新选择凭据能看到运行时新材料，随后测试通过。实际推理只有一次，Bearer 为新 token，refresh endpoint 也只调用一次。审计分别记录失败／未知用量与成功／5 tokens，均不泄露旧、新 access token 或 refresh token。
2. 捕获 enabled 账号后由配置 owner 禁用账号，旧快照不发送；保留失败审计，没有写入模型健康状态。

本批不声称网络发送与配置发布跨线程原子化，也没有实现撤回已发出的请求。发送已经开始后发生轮换的成功／错误反馈竞争仍须独立验收；本次测试覆盖已发布轮换／禁用后继续使用捕获快照的边界。

## 验证结果

`cargo test --locked --offline --lib -- nous --test-threads=2`

最终 **71 passed / 0 failed / 0 ignored**，其中新增 2 项；本次改变生产发送路径，因此重新覆盖 Nous 协议、管理、自动调度及刷新测试。测试执行 104.43 秒，完整命令 1440.30 秒，包含共享 Cargo 锁等待与编译，不是纯编译耗时。

首轮编译失败于新测试夹具引用了未引入的 public_wire 模块，尚未执行测试。已将夹具收窄为本场景的非流式文本响应，不改生产协议；保留 `rust-r1` 失败记录，最终验证为 `rust-r2`。

有效行 checker 31 项、ratchet、scoped rustfmt、git diff --check 均通过。生产 probe 64 → 73 有效行，测试注册文件 246 → 248，新测试 177 行；三个 Rust 文件最终复验前后 SHA256 一致、UTF-8 无 BOM，未修改 baseline／例外。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-probe-rotation-20261112`，含源码快照、scoped.diff、命令日志、receipt、哈希和 final-validation.json。

## 未完成范围

真实独立 client 资格／账号／订阅费用、发送中轮换、原生 UI／EXE、真实进程重启和 Server／Redis 验收仍开放。全范围仍为 67 个工作项未完全关闭，不将本批当作 Nous 整卡完成。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。
