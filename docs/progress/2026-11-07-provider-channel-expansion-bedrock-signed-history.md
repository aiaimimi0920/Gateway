# Bedrock：原生请求 reasoning/signature 历史保真

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
继承 `main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981` 的未提交实现；HEAD 不是本批交付凭据。
本子批只闭合原生 Converse **请求侧**历史的有序重发与策略冲突拒绝，不是 reasoning 响应、跨协议完整往返、W06 或总计划完成。

## 改了哪些内容

1. 原生历史含 `reasoningContent` 时，重新以同一 normalizer 投影 raw body，与策略处理后的完整 canonical messages 比较，包括 system、角色、文本、工具及 opaque reasoning。仅在一致时复用原始 `messages/system`，保留多个 system 块、交错的 reasoning/text/tool、同一 user 数组中的 toolResult/text、原样空白/CRLF/signature/redacted bytes。
2. canonical 历史已被修改，或 `extra` 试图替换 `messages/system` 时，三个生产网络 sender 均在 HTTP 前返回 `400/bedrock_reasoning_history_changed`；不重放 raw 绕过策略，不把失效签名继续发出，也不自动丢弃 reasoning。无 reasoning 的旧请求继续采用 canonical 策略结果。
3. 原生 normalizer 和网络最终打包边界拒绝 reasoning union 冲突、错误类型及坏 Base64。`ReasoningTextBlock.signature` 可省略；空 text 和空 redacted bytes 合法，不伪造签名或秘密。只校验 opaque Base64，不解码后重新编码或 trim；不宣称验证了 AWS 签名真伪。
4. 保留旧非 fallible `pack_bedrock_converse` 的结构兼容用途：合法未改历史同样保真；发生冲突不恢复原 raw，仍返回原结构投影。它不是网络 admission；实际网络只使用 fallible packer，错误不降级。

源码 owner 为 [history.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/history.rs)、[normalization.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/normalization.rs)、[packing.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/packing.rs) 和根模块接线。
不修改共享 canonical response 或 S06/Gemini，不把 Bedrock 状态塞进 `vertex_turn`，不新增服务、依赖、存储版本、模板或渠道合池。

## 官方合同与本地原文

本轮匿名联网 GET 五页均 HTTP 200，URL、UTC、最终 URL、bytes、SHA-256 与完整 HTML 保存于证据根 `sources/receipt.json`，不是实时模型调用：

- [ReasoningContentBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ReasoningContentBlock.html)：`reasoningText/redactedContent` union，仅一个成员。
- [ReasoningTextBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ReasoningTextBlock.html)：text 必填、signature 可选；后续多轮原样保留。
- [Converse messages](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html)：signature 绑定会话消息，修改历史会使签名失效。
- [ReasoningContentBlockDelta](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ReasoningContentBlockDelta.html) 与 [ContentBlockStart](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlockStart.html)：为下一响应子批固定依据；本批没有据此实现 response decoder。union 以 API reference 为准，不照抄 userguide 中可能同时列出两个成员的示例。

实现沿用本仓 Anthropic 已有的“原始投影仍一致才保留 native 有序历史”的思路，不复制 SDK/第三方源码。

## 红绿与发送证据

测试 [bedrock_signed_history_contract.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_signed_history_contract.rs) 与 [wire.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_signed_history_contract/wire.rs) 使用合成数据及随机 loopback。
生产未修的 `red-v3` 实际 **1 passed / 6 failed**，包括真实 HTTP 捕获的块重排、策略修改仍发送、畸形 reasoning 接纳及 extra 替换；带循环测试只声称首个失败已复现，不把后续变体逐一算红测。
新增的工具/角色/前缀变更、缺省 system 和 optional signature 保护以最终绿测为准。

最终新合同 `signed-final-v1` **9 passed / 0 failed / 0 ignored**，在下述相邻测试 harness 修改完成后冻结源码复核；不叠加此前 `green-v2` 的9项。真实生产 client 的 JSON passthrough、buffered EventStream、SSE 三条请求发送路径分别核对上游收到的完整 messages/system；策略冲突三条路径均核上游零调用。
使用既有 legacy-auth loopback，不放宽显式 Bearer/IAM 的 AWS allowlist。SSE 分支在拿到 stream 后 Drop，只证明请求 wire，不声称解码 reasoning 响应、公共入口全链或 AWS 签名有效。
测试 server 正常 shutdown 被等待，异常路径 Drop abort；网络和停止均有超时。

## 验证与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-signed-history-20261107`。

| 检查 | 实际结果 | 回执 |
| --- | --- | --- |
| 新增请求保真合同 | 9 passed / 0 failed / 0 ignored | `signed-final-v1.json` |
| 12组相邻 Bedrock 合同 | 84 passed / 0 failed / 0 ignored | `adjacent-final-v2.json` |
| 库定向回归 | 30 passed / 0 failed / 0 ignored；5219项被筛选，未执行 | `library-final-v1.json` |
| 正常生产库、代表性关闭 Bedrock 生产库 | passed；关闭组合从当前 runtime-defaults 去除 Bedrock family/line，不是完整矩阵 | `production-final-v1.json` / `disabled-final-v1.json` |
| 七项相关源码官方 Rustfmt | passed | `rustfmt-final-v1.json` |
| checker 自测 | 31/31 passed | `checker-final-v1.json` |
| 官方有效行 ratchet | passed；4794 文件、14 immutable runtime artifacts、governed source >700=0 | `ratchet-final-v2.json` |

最终按完整测试名称去重为 **123 passed / 0 failed / 0 ignored**，包括9项新增；不叠加中间绿测、初次未执行的相邻组合或筛选掉的库测试，不报全仓绿、零warnings或完整feature矩阵。
七项最终后端/静态阶段各5107项所选输入阶段内无漂移，且与当前源码同hash；checker的scripts消费子集另核未变，没有因仅测试harness变化而重跑未变化的checker自测。该清单不是所有外部工具、动态输入或全部依赖的冻结。
七项源码官方 lexer：根模块104→105、packing187→200、normalization207→208；新增 history77、同步合同153、HTTP合同205有效行；旧生成参数wire测试仅追加sender future boxing，226→234，均低于500，不修改 baseline/exception，详见 `effective-lines-final.json`。
ratchet 报告先归档，再从**本批直接 preimage**原字节恢复共享报告，保留继承 dirty 内容；第一次见 `shared-report.json`，最终结果见 `shared-report-v2.json`。

本批直接preimages、仅七项源码变化、UTF-8无BOM、新回执本地链接、五页官方原文/四份FreeBuff新增原文hash、范围diff与最终测试来源见 `audit.json`、`scoped-diff.patch`；仓库及限定新增/修改diff的空白检查通过。
模块审查：固定安全错误不带history/signature，原opaque值仅发原目标，不增认证或出站权限；比较/验证是随既有有界请求线性处理与短期分配，不增加循环重试、后台任务、队列、进程或持久状态。继承的HTTP/stream资源owner保持，真实上游/跨进程仍不外推。

`red-v1` 是新测试 future layout 查询深度的编译错误；测试边界 boxing 后，`red-v2` 又暴露误用未直接依赖的 `futures_util` import，随后改为本仓 `futures::FutureExt`。
两者不计行为红测，未提高 crate recursion limit 或改生产 async。`green-v1` 因错误使用不存在的 `GatewayError::internal` 编译失败，修为既有 `server_error`；不算通过。全部失败日志与 receipts 保留。
扩大回归的 `adjacent-final-v1` 在旧 `bedrock_generation_wire_contract` 也暴露相同 layout 深度编译错误，没有执行测试，不算84项通过；仅把同样的sender future boxing加在该测试边界，断言/数据/超时和生产语义不变，随后 `adjacent-final-v2` 才完整执行12组并通过。

## 未完成与接力

- JSON response 的 `upstream/response.rs` 仍拒绝 reasoning block；EventStream 的 `upstream/state.rs` 仍拒绝 reasoning delta。下一步须用独立 Bedrock 响应状态/预算/错误生命周期承接，不能删除拒绝后把 text/tool 当全部响应。
- canonical response、公有 Chat/Messages/Responses 的 reasoning/signature 有序输出及后续历史桥接未接；完整多轮 round-trip 仍缺。非原生 source 不凭 raw body 冒充可恢复的 Bedrock native history。
- Bedrock 专属完整取消 audit、公共路由的签名历史联合验收、独立进程/原生包、真实 AWS/IAM 权限/模型/额度与 P50/P60/P70 未做；没有读取真实凭据、调用供应商、修改实例、创建容器、出包、提交或推送，不复用 P34 历史单容器授权。
- FreeBuff 本轮只补固定公开 root/费用调查事实，见 [业务合同 §5](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md)；不开放其公共业务 guard。

累计模板仍17，无新增整卡 `offline_ready`。R10/W06 和总目标继续执行中，不要求现在录制凭据。
