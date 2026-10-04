# Issue #3 / S1：等待提示和累计等待预算源码检查点

关联 [执行计划](../plan/issue-3-reliability-optimization.md) 和
[等待策略契约](../retry-wait-policy.md)。本文件记录 S1 的实现和提交前验证；
提交后远端 SHA、CI、不可变手测包和下一接续点由 Issue #3 的进度区及回执追加。
源码前置提交为 `527dfa8f1cbd01876fdeabf6d1b85b8dfc87f541`，本地 / 远端已复核为 0/0。

## 实现边界

- 毫秒头优先于 `Retry-After` 秒数 / HTTP-date，再落回现有正文 / 类别 hint 和本地退避。
- 普通入口委托 observed 入口，二者使用唯一 `WaitBudget`。默认累计睡眠预算为
  15 秒；不能容纳完整提示时保留最后真实错误，不能截短后提前发送。
- 不增加重试次数、候选或付费渠道，不改变永久状态硬拒绝和重试前再次准入。
  Drop 原 future 同时取消等待，没有为睡眠派生后台任务。
- HTTP 错误接线覆盖共用 buffered / streaming、强制累积、JSON / binary
  passthrough 和 ChatGPT official / Codex；正文复用解码后 64 MiB 上限。
- 不接管 S06 Gemini 专属实现；内部 OAuth / browser / recovery HTTP 头并非全部接入。
  跨候选 / recovery 的共享 attempt 和墙钟 deadline 属于 S3，不在本项冒充完成。

## 验证回执

证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-issue3-20261004/`。

| 验证 | 当前结果 | 回执 |
| --- | --- | --- |
| 修改生产逻辑前的等待回归 | 0 passed / 2 failed，确认两个入口提前重试 | `s1-red-retry-hints.json` |
| 最终错误、等待、真实 HTTP 和官方 API 邻近回归 | 90 passed / 0 failed | `s1-review-fix-unit-wire-r2.json` |
| 最终三份重试集成及 pipeline / HTTP 分类邻近合同 | 21 passed / 0 failed | `s1-review-fix-integration.json` |
| 最终 locked / offline 全目标编译 | exit 0 | `s1-review-fix-all-targets.json` |
| 最终 Rust formatter | exit 0 | `s1-review-fix-format.json` |
| 行数 checker 自测 | 31 passed / 0 failed | `s1-line-checker-tests.json` |
| 最终行数 ratchet | exit 0；governed source >700 为 0；保留原有不可变运行资产 | `s1-review-fix-ratchet.json` |
| 依赖覆盖 / 发布 / 报告安全契约 | 30 passed / 0 failed | `s1-final-security-contracts.json` |
| 最终在线五锁 OSV JSON / SARIF 扫描 | 报告验证 exit 0；findings_free=false，仍阻止公开发布 | `s1-final-osv.json`、`osv-final/status.json` |

全部测试使用内存、虚拟时钟或 loopback，不调用真实账号和付费模型。
九种真实 HTTP 入口的等待头接线，以及两种重试 API 的实际发送间隔均已覆盖。
原有 S06 等区域的 unused / dead-code 警告没有因本项而被顺手改动。

首次单元测试 88/89 通过，失败源于 Windows `SystemTime` 的 100 ns 表示精度：
最小探针证明减去 1 ns 得到的实际差值为 0。测试改用可表示的 1 微秒输入，仍验证
向上取整为 1 ms；生产日期算法没有为了测试改写。原失败与探针回执均保留。

首次 ratchet 指定仓库外 JSON 报告路径，被 checker 的路径边界拒绝。随后直接运行
同一官方 checker 的 `--mode ratchet`，将控制台与退出码保存在 linshi；不修改
checker、基线、例外文件或原有 tracked 报告。

## 逐文件复核

权威 lexer 结果见 `s1-final-lines.json` 和修复后的 `s1-review-fix-lines.json`；
本项所有 Rust 文件均不超过 500 有效行。

| 文件 / owner | 有效行数（前 → 后；新文件仅列后值） | 复核重点 |
| --- | --- | --- |
| `src/retry.rs` | 477 → 460 | 单一循环、准入顺序、最后错误和脱敏日志 |
| `src/retry/wait_budget.rs` | 24 | checked subtraction；不截短 hint；本地 backoff 也受预算约束 |
| `src/error.rs` | 280 → 291 | hint 只改变等待建议，不提升 retryable 或修改错误身份 |
| `src/error/classification.rs` | 182 → 171 | 仅委托正文解析 owner，不做无关分类改写 |
| `src/error/retry_after.rs` / `body.rs` / `tests.rs` | 52 / 85 / 189 | 单值头、日期；借用原始正文数值、精确十进制取整和有界溢出处理 |
| `src/upstream/response_error.rs` | 29 | 消费 body 前仅保留等待数值；资源准入错误不被吞掉 |
| `src/upstream/response_error/tests.rs` / `fixture.rs` / `targets.rs` | 140 / 86 / 99 | 无代理 loopback、有限记录、正常路径 joined shutdown、Drop abort |
| `src/upstream/client/request_plan_part_2.rs` | 403 → 399 | 仅共用错误接线，Gemini 专属分支不动 |
| `src/upstream/client/streaming.rs` / `passthrough.rs` | 258 → 254 / 300 → 300 | 只调整失败响应；不改变成功 / SSE 提交边界 |
| `src/upstream/chatgpt/official_api/execution.rs` / `execution/body.rs` | 362 → 357 / 44 → 18 | 三种入口保留原 provider 与 body label；JSON reader 不动 |
| `src/upstream/mod.rs` | 158 → 159 | 注册单一响应错误 owner |
| `tests/retry_wait_contract.rs` / `retry_wait_budget_contract.rs` / `retry_wait_logging_contract.rs` | 51 / 196 / 66 | 虚拟时间、累计边界、取消 / 准入 / 永久状态、秘密字段负向断言 |
| `Cargo.toml` / `Cargo.lock` | 配置 | 既有 `httpdate 1.0.3` 显式依赖；serde_json 仅启用 raw_value；Tokio test-util 仅测试启用；锁差异只增加 root dependency |

没有新建线程、进程或长期状态；热路径仅扫描固定三种头和有限提示，不保留响应头集合。
日志不输出 message、code、provider URL、hint reason 或响应头。测试日志中的 secret
字符串均是合成夹具，不使用真实凭证。原有三个 dirty cache/report 的 SHA-256 必须
在提交 / 交付前与 `preserved-before.json` 比较，并排除在本项 stage 范围之外。

## 独立审查与修复

此前审查代理返回 503，只能记 unavailable。本次独立只读审查实际发现 P2：
正文 `1e400` 回落为默认 5 秒，高精度小数被 f64 舍短。原函数的隔离探针真实
0 passed / 2 failed；正文等待解析改为 RawValue 借用及精确十进制运算，新增
极大 / 极小指数、高精度、u64 边界和 null / invalid 字段优先级回归。
独立复核未发现新的阻塞缺陷；完整范围与未覆盖项见 `s1-independent-review.md`。
修复初次编译因 re-export 可见性失败，窄修正为 crate::error 可见后，最终门禁
全部通过；保留失败回执，不将中间失败写成最终通过。

## 尚未完成的交付边界

独立源码复核不是发布批准。新提交的 GitHub CI、正式构建、包完整性和隔离
SQLite / 原生启动必须逐项追加真实回执。不可变本地手测候选不等于公开发布、安全
门禁全绿、真实上游验收、Docker 部署或 S2/S3 完成。

OSV-Scanner 2.5.1 的已核验二进制哈希及实际版本已再次检查，完整五锁 JSON / SARIF
报告一致。两条 canonical 公告仍为 glib 的 RUSTSEC-2024-0429（GHSA alias）和
proc-macro-error 的 RUSTSEC-2024-0370，共三条 occurrence；没有 suppression 或
新安全例外。development 报告验证成功不能冒充 findings_free 或 release 成功。
