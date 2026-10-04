# 逻辑请求的共享发送预算与回退授权

关联 [Issue #3](https://github.com/aiaimimi0920/Gateway/issues/3) 的 S3；
S1 的单次 retry 累计睡眠额度仍见 [等待契约](retry-wait-policy.md)。
二者不是同一个预算，也不能用发送次数代替费用账单。

## 唯一 owner 和计数口径

`PipelineContext.request_budget` 是唯一 owner。所有候选、普通 retry、
ChatGPT/Qwen recovery、browser relay 和 buffered Responses bridge 使用它的
共享 clone，不在切换候选、刷新认证或再次进入 retry 时重新创建。

- 普通请求最多 6 次 adapter execution；这是原逐候选默认 3 次的请求级上限，
  不是另外新增 6 次 retry。默认截止时间为 Context 创建后 300 秒。
- `totalRequestTimeoutSeconds` 复用已有可信 route policy 字段，合法范围 1–300；
  从原始起点收紧，不能重新计时或放宽。非法持久化值在发送前 fail closed。
- admission 和 execution start 分离。只有成功准入后的 `begin_attempt` 消耗 slot，
  同时更新 route attempt count / 首次触达账户；被白名单拒绝、排队取消或准入拒绝
  不计已发送。retry observation 同样只在 begin 成功后创建。
- slot 检查与递增在同一短锁内，无锁跨 await；不会为预算启动后台任务。
- attempt 指 **adapter 执行起点**，不是该 adapter 内每一条物理 HTTP。adapter 的
  本地校验、认证、轮询和 S06 内部 HTTP 不因本项变成独立 slot；本项不接管它们。

## deadline、取消和响应移交

截止时间主动约束整个 send stage：候选调度、concurrency 等待、准入、credential
readiness、恢复、S1 sleep、首事件预检和 buffered bridge 累积。到期先标终止原因，
再 Drop 原 future/transport，保证回调看见真实的本地终止原因。

起点早于 auth/filter/route，所以这些阶段已消耗的时间不会在 send stage 重置；
但本项 **不主动 timeout 前置阶段，也不把业务 finalizer 放进可被丢弃的 timeout**。
它不是整个 HTTP/数据库清理链的强制硬超时；同步不 yield 的 adapter poll 也不能
被 Tokio 抢占，到 poll 返回时拒绝迟到结果。前置配额和 finalizer 的其它生命周期
约束仍归其既有 owner，不据此声称 PostgreSQL 退款/崩溃恢复已整链验收。

成功 JSON/Binary 或 SSE 都移交 owner，禁止手工重入调度。SSE 边界定义为返回流
所有权，比“客户端确实收到首字节”更保守：

- 返回 SSE 前的 start/preflight 失败可回退，但仍需预算和明确账户授权。
- 返回 SSE 后只消费原流。deadline 释放 transport，输出一次固定本地 protocol
  error 后 EOF；原 transport error / EOF / Drop 都不会重新换供应商。
- 已输出的文本或工具 delta 不缓存成第二次发送。首事件 replay buffer 仅拼回
  已读取的字节，不是网络重发。
- 本地 deadline/取消仍走 stream failure finalizer，但不降低 AIMD、不记录供应商
  健康失败。真实 transport failure 保持原供应商反馈；未完成 FreeBuff lease
  保留既有 invalidate 行为，这不等于把预算终止算作供应商健康故障。

## 非幂等请求

保守地将 tools、tool_choice、历史 tool_calls 或 Tool-role 消息，以及 image/music/
video/audio 创建和 ResearchCreate 请求限制为 **一次 adapter execution**。
普通 retry 的 max_retries 同时降为零；即使下一个账户已明确授权，也不能再次
创建或回放。这里是 replay 风险分类，不宣称 Gateway 的工具解析器执行了工具，
也不宣称任何 adapter 内部子操作已经变成全局 exactly-once。

## 精确授权，而不是猜付费身份

保留 credential ownership、account group、access catalog 和 route policy 限制。
合并后的 Redis/YAML/DB 队列重新校验 `allowed_provider_account_ids`，发送前再校验，
不能因为候选来自某一来源就越过明确限制。精确 ID 区分大小写，DB 列表按既有
normalization trim；Redis 的 `cred:<id>` 不按品牌映射为另一 DB 账户。

**首次实际发送保留既有路由权限/选择语义；已发送后跨账户回退必须有可信来源
明确点名目标 ID：**

1. 非空的 `allowed_provider_account_ids`；
2. 同一不可变 YAML snapshot 中，本请求经 alias 解析后命中的 enabled
   `ModelRoute.provider_ids`；
3. 同一次 validated local key 事务中的 `providerIds`；
4. 已认证 access key 的 validated access projection route rows。

这些来源不能覆盖其它适用限制。YAML account group、local key restriction、DB
policy 和协议过滤仍执行；access catalog 的余额、reservation、候选预扣/退款流程
保持原 owner，不增加新的计费系统。YAML 只有实际被路由阶段解析时才提供授权，
不能用同名但未使用的配置给 DB 候选授权。

无匹配规则的 supported-model/all-provider 自动发现、最终 candidates 列表、品牌、
profile、adapter、URL、模型名称、priority/weight 都 **不是** 跨账户授权来源。
空或全 blank 的 DB 名单保留首次 selection 的“无约束”语义，但不批准跨账户 replay。
这样不需要为了 standalone 显式多账户路由增加 PostgreSQL。

兼容性变化：未经明确点名的自动跨账户 fallback 被收紧，包含免费或中转账户，
并非断言它们付费。终端返回 `provider_fallback_not_authorized`；所有候选被非空
policy 拒绝且零发送时返回 403 / `provider_not_authorized`。被跳过的候选不会改动
projected row 索引或先执行 keepalive/配额切换。

仓库没有可信通用 `is_paid` 字段；本项不添加按品牌猜测的 paid gate，不把访问权限
当价格分类，也不声称拦截一切合法首次选择的付费请求。它解决的是未经明确授权的
**跨账户自动升级**，包括中转失败后自动转向官方渠道。金额硬额度仍归 Platform。

## 错误诊断与 HTTP 合同

预算终止保留最后真实 adapter/upstream 错误的 message、kind、HTTP status、code 和
provider；完整原错误也保留在 owner getter。只将 retryable 设为 false，并用固定
`FallbackHint::Abort` 原因阻止 retry/fallback，不把它改成另一条供应商错误。

HTTP body 原 message/type/code 结构不变，预算类别独立暴露为固定响应头：

```text
x-gateway-error-code: budget_exhausted
x-gateway-stop-reason: attempt_limit | deadline | cancelled | response_handed_off
```

无真实上游错误时，body 的 code 才是 `budget_exhausted`。预算错误不输出 Retry-After。
只接受上述完整固定原因，不导出自由文本 hint、headers 或请求内容；部署代理是否
保留头及浏览器 CORS 可见性不由源码序列化测试证明。

`last_upstream_error` 仅记录真实 adapter result、发送后的 stream preflight 或 bridge
错误。admission、keepalive readiness、quota/授权错误和 synthetic cancellation 不
覆盖它。当前 admission 错误仍可作为最终响应返回，但不能伪称它是最后模型错误。

## 聚焦验证入口

```powershell
cargo test --locked --offline --test request_budget_contract --test request_fallback_authorization_contract --test pipeline_send_runtime_contract -- --test-threads=1
cargo test --locked --offline --test retry_wait_contract --test retry_wait_budget_contract --test retry_wait_logging_contract --test attempt_cancellation_metrics_contract
cargo test --locked --offline --lib -- local_runtime::access_keys:: routing::config:: pipeline::stage_send:: retry::
```

测试只用虚拟时钟、内存对象、临时 SQLite 和无代理 loopback HTTP，不调用真实
账号/付费模型。执行这些 fixture 时 TEMP/TMP 必须指向 linshi 下的独立目录；
临时目录清理只允许本 fixture 创建的精确路径。具体通过/失败、提交、云端 CI 和
新包分别记录到本项 status / Issue，本文中的命令不是已通过回执。
