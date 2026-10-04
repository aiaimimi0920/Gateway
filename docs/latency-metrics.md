# 请求与真实 attempt 的尾延迟观测

`GET /metrics` 复用进程内 `GatewayMetrics`，不上传正文，不调用模型，也不引入
外部监控服务。现有 sum/count、request/provider/errors/rate-limit 指标名保留。

## 口径与生命周期

| 指标 family | 一个样本的边界 |
| --- | --- |
| `gateway_request_duration_ms` | 一个 HTTP 逻辑请求，从 middleware 进入到响应 body 被服务器消费完、发生 body error 或被 Drop；handler future 提前取消也计一次 |
| `gateway_provider_latency_ms` | 一次真实 provider 调用；buffered 包围真实发送 future，stream 从实际发送开始到最终流 EOF/error/Drop；失败的 stream-start 单独计一次 |
| `gateway_provider_ttft_ms` | 同一次 stream attempt 开始到首个转换/规范化后非空 Bytes chunk，终结时记录；不是 tokenizer token、首文本 delta 或原始 HTTP 首字节 |

HTTP HEAD / 初始 end-stream body 在释放源 body 后结束；WebSocket 等升级只计
HTTP 握手，不把长连接会话混入 HTTP 时长。响应头不代表普通流式请求结束。
body 消费结束不保证客户端已收到或渲染最后一字节，也不等待异步业务 finalizer。
日志的 `response established` / `latency_ms` 仍表示响应建立耗时，不冒充完整 body 延迟。

本项明确纠正旧 request sum/count 的“只到响应头”口径，既有消费者看到的流式均值
将变长；provider latency 保留既有真实 attempt 口径。一个请求重试两次是一个
request、三个 provider 样本。准入拒绝、keepalive refresh、退避睡眠不是 provider
attempt；sleep 或 admission 中取消不凭空添加 attempt。发送中取消记录恰好一次，
stream-start 成功把所有权移交给最终流，preflight 失败不再重复计数。

TTFT 为 `None` 的空流、首块前错误/取消不进入 TTFT 直方图；确实在同一毫秒到达
的首块可产生合法 0 样本。role、metadata、SSE 注释和结束事件也可能是首非空 chunk。
`SlidingWindowMetrics` 仍是独立滑窗工具，当前没有生产接线，不称它已接收这些数据。

## 固定桶与有界性

三组毫秒桶统一为：

```text
5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000,
30000, 60000, 120000, 300000, +Inf
```

`le` 包含边界，bucket 累计输出，`+Inf == count`，所有真实延迟进入 sum/count。
单个直方图的 buckets/sum/count 在同一 owner 锁内更新、复制，格式化在锁外进行；
不同 owner 的 scrape 不是跨注册表事务。累计 u64 饱和而不回绕，极限值不能作为
无限精度账单。没有动态桶、无界请求历史或按请求分配指标 series。

provider/model 沿用最多 64 ASCII 字符和已有 256 普通 key 上限；额外 `overflow/overflow`
聚合条目最多使总 key 数为 257，已存在 key 不会被驱逐。两套 provider 直方图共享
同一有界 key 表，不分别绕过上限。标签清洗不是秘密检测；调用方必须仍只传路由
provider account/model，不能把 URL、prompt、token 或 request_id 填入这些字段。

`failure_class` 只接受既有十个分类，以及 `cancelled`、`stream_interrupted`；任意
正文或未知分类归 `unknown`。`gateway_request_terminations_total{reason}` 的 reason
只有 `completed`、`cancelled`、`stream_interrupted`。成功 HTTP status 的中途取消
或 body error 仍增加 request error，HTTP status 本身不被篡改。

`gateway_reliability_events_total{event,reason}`：

- `retry/same_candidate`：observed retry 调用中真实的第二次及后续发送，完成/取消时记录。
- `retry/recovery`：refresh/browser-relay 重新准入成功后开始恢复发送时记录；不计 refresh 本身。
- `fallback/<既有分类>`：前一候选失败后，下一个候选已获准并进入发送分派的次数；
  没有下一个获准候选时不虚增。它表示候选分派，不保证某 adapter 内部已经发出网络包。
- event 是枚举，reason 是固定白名单；任意新字符串归 `unknown`，不能扩展基数。

request 终结、provider 取消与 stream 中断分别留在自己的层级，不把它们相加当用户数。

## 可直接执行的 PromQL

结果单位是毫秒。先 `rate` 再按 `le` 聚合，不平均多个实例的 p95：

```promql
histogram_quantile(0.95, sum by (le) (rate(gateway_request_duration_ms_bucket[5m])))
histogram_quantile(0.99, sum by (le) (rate(gateway_request_duration_ms_bucket[5m])))
histogram_quantile(0.95, sum by (provider, model, le) (rate(gateway_provider_latency_ms_bucket[5m])))
histogram_quantile(0.95, sum by (provider, model, le) (rate(gateway_provider_ttft_ms_bucket[5m])))
sum(rate(gateway_request_errors_total[5m])) / clamp_min(sum(rate(gateway_requests_total[5m])), 0.000001)
sum by (event, reason) (rate(gateway_reliability_events_total[5m]))
sum by (reason) (rate(gateway_request_terminations_total[5m]))
```

固定桶得到的是近似分位数；超过最后有限桶会落 `+Inf`，不能宣称精确 p99。
空流没有 TTFT 样本；count 为 0 的系列不应该被当成零延迟。并发 mock 回归只证明
计数、一致性和内存基数边界，不证明生产吞吐或尾延迟改善。本项不包含 S3 请求级
attempt/deadline 预算、付费压测、S06 内部或生产部署。
