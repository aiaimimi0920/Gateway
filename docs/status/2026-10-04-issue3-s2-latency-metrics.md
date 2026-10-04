# Issue #3 S2：固定桶与两层观测源码检查点

## 状态与范围

基于 `main / 43ee081d669cc2da915b17e3552a517e9b71fdad`；S1 同提交 CI / CodeQL
已成功，严格公开发布仍因依赖与历史 secret findings 阻止。S2 源码、独立只读
复核和下列本地门禁已完成；本检查点不代表 S2 已推送、云端 CI 成功或新包已交付。
提交后实际 SHA、CI 和不可变包回执追加到 [Issue #3](https://github.com/aiaimimi0920/Gateway/issues/3)。

仅修改 Gateway 的 request/provider metric owner、HTTP body 生命周期、retry
observer 和 stream terminal 接线。不接管 S06 内部，不改变 Platform 计费，不执行
真实模型请求，不扩大重试/候选授权；S3 请求级 attempt/deadline 预算仍未开始。

## 已完成行为

- request/provider 原 sum/count 名保留，新增同 family 的固定累计 bucket；TTFT
  新 family 直接复用 TrackedStream 的首个转换后非空 chunk，缺失不补 0。
- HTTP middleware future 把 exactly-once 终结权移交给 response body；EOF/error/Drop
  后计一次请求，取消未返回响应的 handler 也释放 in-flight。HEAD / 初始 end-stream
  在源 body 释放后结束；data/trailer 原样透传，不缓存或拼接整个 body。
- observed retry 在真正准入后的 upstream future 上建立取消 guard；sleep 和 retry
  admission 被取消不虚增 attempt。直接 stream-start / recovery / Responses bridge
  的取消 guard 与 preflight、最终 TrackedStream 之间明确移交，不双计。
- provider/model 保留 256 普通 key + 一个 overflow 聚合；失败与诊断原因使用封闭
  白名单，不导出任意错误正文。每个 histogram 在同一锁中更新/复制，格式化在锁外。
- `retry` / `fallback` 与 request/provider cancel/interruption 分别留在各自层级。
  候选分派与真实网络发送不混称；无下一个获准候选时不虚增 fallback。

完整口径、桶列表、兼容性变化和 PromQL 见 [latency-metrics.md](../latency-metrics.md)。
request 流式均值从响应头改为 body 消费终结，TTFT 不是语义 token；mock 回归
不证明生产吞吐或 p95/p99 改善。SlidingWindowMetrics 当前仍未接入生产。

## 实际本地验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-issue3-s2-20261004/`。

| 检查 | 实际结果 |
| --- | --- |
| 新 histogram 回归在旧实现上的红灯 | 4 项按预期失败：缺 bucket/type、任意原因标签；首次 linker 因临时目录缺失失败，建好目录后的 r2 才是行为红灯证据 |
| 聚焦 unit：metrics / upstream::stream / retry | 72 passed；包含真实 TrackedStream 到 provider/TTFT 接线、pending/drop/error、observer 及 S1 等待合同 |
| 七套 integration contract | 40 passed；包含边界、并发 scrape、overflow、闭合标签、HTTP body/trailer/HEAD/drain、真实 loopback buffered/streaming 发送中取消 |
| 发送中取消真实接线 | 上游收到 HTTP 后才 Drop；两种模式均 provider count/+Inf/cancelled = 1、TTFT count = 0、permit active = 0；pending fixture 在 shutdown 前解除并 await |
| locked/offline all-target check | exit 0；不是全 feature matrix 或全业务 suite |
| formatter / 行数 checker 自测 / 最终 ratchet | exit 0；31 项 checker 自测通过；没有新增源码超过 500 有效行 |
| 安全发布契约 | 23 passed；不是在线 findings-free 扫描，也不放行公开发布 |
| diff / 编码 / 既有 dirty 文件 | diff check 通过，新增/改文本 UTF-8 无 BOM，三个原有 cache/report 的 hash 3/3 保持 |

HTTP body 测试首次因 `Config::from_env` 要求 Redis 环境失败；已改用仓库既有的
显式临时 TestState，最终 r2 通过，不把失败尝试隐藏或冒充产品缺陷。

## 独立复核与逐文件审查

只读独立复核发现 HEAD / 初始 end-stream 在源释放前降低 in-flight 的顺序风险。
已改成先 Drop 源、再 request accounting，并用不允许 poll 的 ReleaseProbe 断言
源释放时 active 仍为 1、随后为 0；最终集成回归通过。复核没有剩余阻断源码意见，
但它不执行测试，也不是正式发布批准。

- histogram / registry：固定数组、饱和累计、有界 map、同 owner 一致快照；无 await
  持锁、无请求历史、无动态原因 key。provider 清洗不是秘密检测，调用方只传路由字段。
- HTTP body owner：同步 guard，无 detached task，不解析/复制 frame 内容；取消、错误、
  最后一帧和 Drop 恰好终结一次，释放上游后再归还 lifecycle。升级只计 HTTP 握手。
- retry / pipeline：guard 只观察实际 future 的生命周期，合成取消仅发给指标 observer，
  不替换返回错误或改变重试决策。recovery/bridge 的每条 await 与正常完成点已复核。
- stream owner：EOF/error/Drop 固定 terminal enum，先释放 inner 再 callback；空流和
  pending 保留 None。原业务 finalizer/lease 策略保留，不称 request 指标已等待它们完成。
- 测试 fixture：仅合成数据和 127.0.0.1；发送登记最多八条，watch 在 finish/Drop
  前解除 pending，正常 finish 有界 await；临时数据在 linshi，不操作真实账户。

`http-body 1.0.1` 原已在根锁图中，本项仅增加直接依赖边以透明保留 Frame/trailer，
没有新增版本、网络 client 或依赖安全例外。桌面只依赖 local-data，不需要根 Gateway
这条依赖边；桌面锁图没有改动。

## 有效行数

全部本次源码/测试在 500 内，最高 `src/retry.rs` 为 461（原 460）。其余主要 owner：
registry 300（原 237）、HTTP middleware 228（原 227）、新 body owner 116、stream
owner 142（原 131）、dispatch 343、feedback 226、streaming 321、Qwen recovery 202、
Responses bridge 197。ChatGPT recovery 按 buffered 与 stream-start 生命周期分开，
分别 261 / 262，避免加 guard 后制造 501–700 文件。

新 histogram/diagnostics/direct-cancel/retry-observer 模块分别 42/49/43/44；聚焦
unit helper 151、histogram contract 257、body contract 319、attempt-cancel contract 73；
loopback fixture 257（原 234）、streaming contract 171（原 125）。无软上限豁免。

## 接续要求

仅 stage S2 scoped files，独立 `Refs #3` commit、正常 push main 并核远端 SHA。
从已推送提交新建 linshi 快照，fresh 官方 build、包/原生/SQLite gates 完成后，
copy-only 交付新 `Neuro/release/Gateway/<独立版本号>`；不要覆盖 S1 或复用旧包冒充 S2。
本地包、同提交 CI 与严格公开发布分别记录，Issue 保持 open，下一开发项才是 S3。
