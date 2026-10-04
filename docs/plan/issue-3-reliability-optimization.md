# Issue #3：可靠性与尾延迟优化执行记录

关联：<https://github.com/aiaimimi0920/Gateway/issues/3>。
该链接是 Issue，不是 PR；原始审查与验收清单保留，执行进度追加到该 Issue。

## 授权、基线和边界

- 用户要求：先记录目标和步骤，再逐项优化；每完成一项单独提交、推送，并更新 GitHub 进度，便于其他 AI 接续。
- 起点：`main / 952b85cc6d9712a305a2f7b3b49df29c6cc2977b`；2026-10-04 UTC 已 fetch，ahead/behind 为 0/0。
- PR #1 已合并；PR #2 已因其代码被主干包含而关闭。两者 HEAD 均通过本地祖先关系检查，不再按旧 Issue 日期假定仍待合并。
- 只修改独立 Gateway 仓库；不接管 S06 Rust/Gemini 专属实现，不改 Platform 计费职责，不调用真实账号或付费模型验证。
- 保留开始时三个 dirty cache/report 文件，不将它们纳入提交：`graphify-out/cache/last_query_stamp`、`node_modules/.vite/vitest/da39a3ee5e6b4b0d3255bfef95601890afd80709/results.json`、`scripts/artifacts/effective-code-lines.json`。
- 不覆盖旧 release；每个完成的运行时大项重新构建到 `../release/Gateway/<独立版本号>`，保留源码提交和包校验。
- 不修改依赖安全门禁、扫描器、行数基线或既有审批；本地手测通过不等于公开发布、CI 或全业务通过。

## 执行顺序与完成标准

采用原 Issue 推荐的 1 → 3 → 2 顺序：先修等待决策，再补观测，最后单独处理跨流程控制预算。

| 执行编号 | 对应 Issue | 目标 | 状态 | 提交与证据 |
| --- | --- | --- | --- | --- |
| S0 | 执行准备 | 冻结起点、记录 owner / 验收 / 提交与交接方式 | 已提交并推送 | `527dfa8f1cbd01876fdeabf6d1b85b8dfc87f541` |
| S1 | 第1项 P1 | HTTP 等待提示与累计等待预算 | 已提交、推送和交付本地手测候选；同提交 CI / CodeQL 成功；公开发布仍阻止 | `43ee081d669cc2da915b17e3552a517e9b71fdad`；[S1 源码检查点](../status/2026-10-04-issue3-s1-retry-wait.md) |
| S2 | 第3项 P2 | 请求 / attempt / TTFT 固定桶直方图 | 已独立提交、推送、交付本地新包；同提交 CI / CodeQL success，严格公开发布仍阻止 | `4cd118efe93ad5c58ff0fc24867baef3935486d7`；包 `gateway-product-20261004-main-4cd118e-s2-r1`；[S2 回执](https://github.com/aiaimimi0920/Gateway/issues/3#issuecomment-5977968325) |
| S3 | 第2项 P1 | 跨候选和恢复共享尝试 / deadline 预算 | 源码验证完成；本记录为提交前检查点，提交和新包回执另见 Issue | [S3 检查点](../status/2026-10-04-issue3-s3-request-budget.md)；40 聚焦 / 47 邻近 integration / 137 unit 与 all-target check 已通过 |

### S1：等待提示和有界重试

1. 复核通用 HTTP 错误构造、流式 / 非流式接线和两个重试入口；不以某个供应商特例代替全局合同。
2. 增加有界等待提示解析：标准 `Retry-After` 秒数 / HTTP-date、明确支持的毫秒头；写明头 / 正文 / 本地退避的优先级。
3. 对非法、负数、溢出、过长提示建立明确行为。超过剩余等待预算时直接终止，不能截短后提前请求上游。
4. 两个重试入口复用同一决策；取消 / Drop 后不继续等待或发新请求，重试前仍重新准入。
5. 保留 400/401/403/404 硬拒绝重试、最后真实错误与脱敏日志，不扩大媒体创建或工具副作用的重试范围。
6. 验证：纯函数 / 虚拟时钟、两个入口、mock HTTP headers/body 接线、取消与准入拒绝，随后邻近测试与构建。

### S2：尾延迟与两层观测

1. 复用现有 request / provider sum/count 与 TTFT owner，不重复建立监控系统。
2. 增加固定桶的逻辑请求端到端延迟、真实 attempt 延迟、TTFT；空流不能伪填 0。
3. 保留 provider series 上限和溢出聚合；标签只接受受限原因，不使用请求 ID、token、prompt 或 URL。
4. 验证桶边界、累计计数、sum/count/+Inf、一请求多 attempt、取消 / 中断，以及并发与基数约束。
5. 提供可执行的 PromQL 示例；mock 压力证据不得冒充真实生产性能提升。

### S3：逻辑请求统一预算

1. 明确 PipelineContext / 候选 / 恢复入口的同一 owner，记录首次响应提交边界和非幂等操作策略。
2. 共享总尝试数、截止时间与取消状态；仅真实发送前计数，准入拒绝不算已发送。
3. 候选或恢复不得重置预算；超过预算返回可解释类别，同时保留最后真实错误。
4. 保留候选白名单和官方付费渠道授权；流式已向客户端提交后不得换供应商重放。
5. 验证多候选乘多次重试、恢复、取消、付费候选拦截、SSE 提交前后与工具副作用。

## 每项交付流程

1. 修改前记录文件有效行数、直接调用方、测试与安全不变量。
2. 先补可失败的聚焦回归；在最窄正确 owner 实现，不顺手重构无关模块。
3. 完成逐文件安全、取消、资源生命周期、有界性与性能复核；执行 formatter、聚焦测试、直接编译、checker 测试、ratchet、diff check。
4. 独立审查需记录真实结果。代理不可用或返回 503 时只能标 unavailable，不得写 approved。
5. 仅 stage 本项文件，以 `Refs #3` 关联，单独 commit 和正常 push；不 force push，不打包其他人的 dirty edits。
6. 核验远程提交 SHA；更新 Issue 进度区和一条提交回执，注明本地测试、CI、release 的独立状态。
7. 对运行时大项执行当前提交的正式构建与隔离启动验证；完成后更新包路径和接续点。构建失败则保留原记录，不将源码完成写成手测交付完成。

## 当前接续点

S1 已实现等待提示解析、共用 HTTP 错误接线和单次 retry 调用的累计等待预算。
独立审查发现正文浮点解析可能提前重试，已用借用 RawValue 和精确十进制取整
修复并复核。最终 90 项单元 / 真实 HTTP 接线、21 项集成合同、locked / offline
全目标编译、formatter、31 项 checker 自测及 ratchet 均通过；新包另行验证。
等待策略和未覆盖的内部 HTTP 范围见 [等待契约](../retry-wait-policy.md)。

S1 已正常推送到 main 并核对远端 SHA；本地候选
`gateway-product-20261004-main-43ee081-s1-r1` 的官方构建、7/7 包门禁、原生窗口、
SQLite 和复制后完整性均通过。CI run `37179216751` 的 Linux / Windows product
与六个 feature shard 成功；CodeQL 同提交成功。严格 Build Windows / Docker 因
依赖和历史 Gitleaks findings 阻止，不能把本地包称作公开正式 release。

S2 已只读核实：middleware 原计时止于响应头，发送中的取消没有完成 observer；
TrackedStream 的 TTFT 是实际 attempt 起点到首个转换后非空 chunk，并非语义 token。
本项已复用这些 owner，给 request body 和 attempt await 增加 exactly-once 终结，
固定桶用同锁快照保持 buckets / sum / count 一致；原因使用枚举 / 固定白名单。
最终 72 unit / 40 integration、locked/offline all-target check、formatter、31 项
checker 自测、ratchet、23 项安全发布契约均通过。独立复核的 HEAD/empty 源释放
顺序问题已修复并回归；提交与新包按 [S2 检查点](../status/2026-10-04-issue3-s2-latency-metrics.md)接续。
S2 不新增 SaaS、配置框架、滑窗接线、真实模型压测或请求级预算。
不得把 S1 的 15 秒累计睡眠额度称作请求级 deadline。

S3 已实现共享 attempt/deadline owner、执行起点计数、SSE handoff 后禁止 replay 和
精确账户回退授权；来源包括可信非空 policy、实际命中的 YAML rule、同一 local key
事务及 validated access projection，不猜品牌的付费身份。40 聚焦 / 47 邻近集成 /
137 unit、locked/offline 全目标 check、formatter 和 diff check 通过；提交及 fresh 新包按
[S3 检查点](../status/2026-10-04-issue3-s3-request-budget.md)和 Issue 最新回执接续。
自动跨账户 fallback 的兼容性收紧、前置阶段/内部 HTTP/计费和未验证范围见
[请求预算合同](../request-budget.md)，不能将其描述为通用金额硬额度或整链硬超时。

工具状态：此前独立代理因 `503 Service Unavailable` 不可用；本次续接完成了
独立只读审查和修复复核，不将其冒充正式发布批准。FastCtx 连接关闭，本次按
精确路径使用 PowerShell 读取。在线五锁扫描仍有两条既有 GTK canonical 公告
（三条 occurrence），没有添加例外；本地手测候选不等于公开发布门禁满足。
GitHub 进度区和提交回执记录提交后实际 CI / 包状态，不能由本地测试推定。

后续 AI 必须先读取此文件的最新版本、Issue 最新进度、当前 Git status 和本项实际提交 / 测试回执。不要仅凭旧日期、包名或本文件的计划复用完成结论。
