# 上游等待提示与重试等待预算

关联 [Issue #3](https://github.com/aiaimimi0920/Gateway/issues/3) 的 S1；执行记录见
[优化计划](plan/issue-3-reliability-optimization.md)。本文只规定共用重试循环的
等待决策，不把逐候选预算冒充整个逻辑请求的 deadline 或费用额度。

## 等待来源和优先级

共用 HTTP 错误 owner 在消费响应体前，只提取规范化的等待毫秒数；不保存或
记录响应头集合。依次选择第一个有效的单值头：

1. `retry-after-ms`：非负整数毫秒。
2. `x-ms-retry-after-ms`：非负整数毫秒。
3. `Retry-After`：非负整数秒，或者 HTTP-date；过去的日期为零等待。
4. 原有错误分类生成的 `FallbackHint::Retry`，包括 429 JSON 正文的
   `retry_after` / `retryAfter`，以及原有按错误类别提供的默认提示。
5. 没有 `Retry` hint 时，才使用原有指数退避与 jitter。

正文支持顶层及 `error` 内的上述字段，单位为秒。HTTP-date 采用已有锁图中的
`httpdate 1.0.3` 解析器，支持标准和历史 HTTP 日期形式；它现在是显式依赖，
没有复制第三方源码或增加新解析包。该包许可证为 `MIT OR Apache-2.0`。

优先级是确定规则，不取多个冲突提示的平均值或最小值。重复的同名 singleton
头、负数、非整数毫秒头、非法日期、非文本值会跳到下一来源。数值溢出或超过
128 字节的提示保留为不可实际等待的大值，随后由预算拒绝；不能因为解析失败
而缩成一次很短的重试。日期的小数毫秒向上取整，避免提前请求。

正文等待字段借用 `serde_json` 的 `RawValue`，按原始十进制系数和指数精确
向上取整，不经过 `f64`；因此 `1e400` 保留为过大提示，`1e-400` 为 1 ms，
`1.000000000000000000000001` 为 1,001 ms。非零正数 token 超过 128 字节
同样由预算拒绝；数值严格为零的 mantissa（包括长指数）为零等待，不分配
数字缓冲。整个正文仍受既有 64 MiB 上限保护。字段存在但为 null / 非数字时
保留原有默认提示，不改选低优先级字段；重复正文键按非法提示回退。

原有类别默认提示保持兼容：无明确正文提示的 RateLimit 为 5 秒，
ServiceUnavailable 为 2 秒，Timeout 为 1 秒。此次变更使循环真正消费这些
已有提示。`max_delay` 仍只限制本地指数退避，不截断上游明确要求的等待。

## 预算和生命周期

`RetryPolicy.max_total_wait` 默认 15 秒。这个上限可容纳默认两次 5 秒限流
等待，同时明确拒绝长时间睡眠；不是照搬其他网关的 60 秒常数。

- 普通入口和预准入 / observed 入口委托给同一执行循环，使用同一个
  `WaitBudget` 决策。
- 每次睡眠前从剩余额度扣除完整等待时长；不能容纳时立即返回最后一次真实
  上游错误，保留 kind、status、code、provider 和 message。
- 累计申请的睡眠时间不超过预算；它不包括网络、队列和调度延迟，不声称提供
  墙钟硬超时。跨候选 / recovery 的总 attempt 和 deadline 属于后续 S3。
- 400、401、403、404 的硬拒绝重试不变，收到等待头也不改变错误可重试性。
- 第一次请求仍由调用方先准入；等待结束后、下一次真实发送前重新准入。
  准入拒绝直接返回准入错误，不产生 transport observation。
- 没有为等待启动后台任务。丢弃 / 取消整个调用 future 会同时取消睡眠或
  正在等待的准入，不会留下定时器继续发请求。
- 重试日志只输出 attempt、获准的等待时长和受限错误类别，不输出原始错误
  message、code、provider 字符串、hint reason 或响应头。

## 已接入的 HTTP 边界

共用 buffered / streaming、强制流式累积、JSON / binary passthrough，以及
ChatGPT official / Codex 的普通、强制累积和流式入口共用响应错误 owner。
共用错误正文读取复用现有 64 MiB 解码后上限和 charset 规则；过大正文或分配
失败保留资源准入错误，普通不可读正文保留 `<unreadable body>` 兼容行为。

Gemini S06 专属实现、浏览器 worker、独立 OAuth/keepalive/recovery 的内部
HTTP 不在此次响应头接线范围内。它们如果已经返回 `FallbackHint::Retry`，
外层共用循环也会遵守；这不代表所有供应商内部 HTTP 头都已接入。

本项不扩大重试次数，不新增候选，不改变付费渠道授权、媒体创建 / 工具副作用
策略，也不在 SSE 已提交后新增重放路径。

## 验证入口

```powershell
cargo test --locked --test retry_wait_contract --test retry_wait_budget_contract --test retry_wait_logging_contract
cargo test --locked --lib -- error::retry_after:: upstream::response_error:: retry::
cargo test --locked --lib -- upstream::chatgpt::official_api::execution::tests::
cargo test --locked --test pipeline_send_runtime_contract --test http_client_classification_contract
```

测试仅使用注入的时钟、Tokio 虚拟时钟、内存错误和无代理的 loopback HTTP；
不调用真实供应商或使用真实凭证。实际执行结果、commit、CI、审查与手测包
分别记录到执行计划和 Issue，不能把本文中的命令视作已通过回执。
