# 2026-10-26 容量拒绝短退避修复

状态：源码实现与定向隔离验证完成；未生成手测包、未重启运行实例、未调用真实供应商。

## 目标与实现

用户报告单 ChatGPT 账号收到 `turn_rate_limit_exceeded / Codex is at capacity, please try again later`
后，后续请求返回 `authorized_candidates_unavailable`，需要重启恢复。新增负向回归在修改前
确认该错误被分类为 `RateLimit`，而不是已有的 `ModelCapacity`。

本轮将该固定 code 与有界供应商容量前缀的 HTTP 429 组合识别为 `ModelCapacity`。
不是对所有 429 开放例外：普通 turn-rate limit、鉴权、参数、余额和 IP 封禁保护保持原语义。

- 独立 provider × credential × upstream model 进程内状态，默认连续退避 2/4/8/15 秒。
- 明确 `Retry-After` 为最低等待时间，不受本地 15 秒上限裁短，不缩短尚有效的限制。
- 容量拒绝不打开普通分钟级 circuit，不写模型冷却，不修改 enabled，不处罚账号级健康、
  AIMD 或软排序分；请求审计继续保留真实错误和发送次数。
- 不在路由候选过滤阶段删除容量受限账号。发送前本地拒绝保留原 status/code/message 和
  剩余 `Retry-After`，不计新上游失败、不增加发送次数、不延长自身等待期限。
- 整个逻辑请求最多一次容量重试许可，继续受安全重放分类、原 deadline、发送次数和
  累计等待预算约束；工具/媒体/已移交流和 adapter 显式禁止重放的请求不借此重发。
- 首次非黏性 SSE 只开放容量专属的一次重试，不开放普通网络错误重试。
- 到期直接允许正常请求，完整成功清对应等级；generation 防止旧在途成功清新限制。
- 状态最多 8192 个身份，不淘汰有效限制或在途 lease；取消复用 Drop 清理，无新后台任务。

设计与生命周期边界见 [capacity-backoff.md](../capacity-backoff.md)。不增加配置项或存储版本，
不清理旧实例内存或旧持久化记录，不启动批量模型验证，不更换用户调用 Key。

## 实际验证

最终 Cargo 锁定、离线编译相关 lib 与 10 个 integration test targets 通过。
最终测试使用 Cargo 输出的精确测试二进制路径，记录二进制 SHA-256 与修改范围源码 SHA-256。
所有生成测试数据和日志位于下列隔离目录，不读取生产 .ng：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-capacity-backoff-20261026/`

| 范围 | 通过数 |
| --- | ---: |
| model circuit、发送 gate、请求容量许可、反馈 policy、retry、error 单元测试 | 128 |
| model_capacity_contract | 5 |
| access_key_groups_contract | 9 |
| request_budget_contract | 12 |
| request_budget_rejected_channel | 8 |
| request_fallback_authorization_contract | 4 |
| retry_wait_contract / budget / logging | 10 |
| attempt_cancellation_metrics_contract | 2 |
| pipeline_send_runtime_contract | 51 |
| Rust 合计（不重复计算） | **229** |

新增三条 loopback Responses 用例覆盖 JSON 自动重试恢复、首次非黏性 SSE 自动重试恢复、
连续拒绝只重试一次、原错误/Retry-After 返回、本地拒绝零发送、其他模型不受影响、
同进程到期恢复、完整成功重置等级和工具请求不自动重放。
三条用例另外在 Windows 默认测试栈复验通过，不重复计入上表。

Windows debug 大型 fixture 初次全套运行遇到 `STATUS_STACK_OVERFLOW`；完整
`access_key_groups_contract` 和 `pipeline_send_runtime_contract` 最终使用 16 MiB 测试线程栈。
只为新增容量场景中的大 dispatch future 与 oneshot fixture 增加堆分配，不宣称相邻整套
debug fixture 已在默认栈通过，也不把增大测试线程栈当作原生 EXE 运行证据。
一条新增边界用例的初版忽略了已有 code trim 规范化，已纠正测试预期并重跑通过。

行数检查器 31/31、ratchet、修改范围官方 rustfmt、git diff --check、UTF-8 无 BOM 检查通过。
本轮涉及的 Rust 文件均低于 500 有效行；最大为已有 `src/retry.rs` 的 444 行。
新增生产 owner 119 行、请求级许可模块 48 行，无需行数例外或修改 adoption baseline。

关键回执：`focused-tests.json`、`static-gates.json`、`source-snapshot.json`、
`verification.json`、`task-scoped.diff`；各套测试输出单独保存。历史失败输出单独保留，
不计入最终通过数。源码备份在 `before/`，最终快照在 `after/`。

## 未执行与边界

未编译/交付新的手测 EXE，未修改端口、凭据、当前数据库或正在运行的程序；
未做真实 ChatGPT/Codex 供应商调用、长时间真实负载验证、全库回归或其他 AI 修改的全面验收。
运行中的旧 EXE 不会因本次源码修改自动生效。其他原因的禁用、鉴权、余额、IP 封禁仍保留保护。
未提交、推送或部署。
