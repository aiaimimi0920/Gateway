# FreeBuff 公共 Chat 实时 SSE 合同

后续验证（2026-11-08）：[正常网页与三协议活 SSE 联合](progress/2026-11-08-provider-channel-expansion-freebuff-public-sse-ui.md)已覆盖单轮文本的逐次批准/取消、EOF 前增量、FINISH pending/accepted 的正常页面读取、原 claim 显式 end/取消 end/再购买及 Local 结算。该联合缺口不再全部列为未验；不代完整参数、工具 UI、Server、原生包或真实平台。

本合同覆盖目录 CLI 业务账号的公共 `/v1/chat/completions` 文本／函数工具流式子链，复用正常请求的逐次购买批准、Local claim/run、签名发送和费用责任。它不是整张 FreeBuff 交付卡、完整 agent harness 或真实平台验收完成的声明。

后续更新（2026-11-08）：Responses／Messages 的限定文本／函数工具实时 SSE 已由[三协议接线合同](provider-freebuff-public-protocol-sse.md)承接，复用本合同的同一原执行 owner；旧批次“这两个入口的流式请求在报价前拒绝”仅是当时停点。没有整段缓存伪装实时流，其他参数／模态仍按既有闭集校验，不悄悄丢弃。

## 一个付费执行 owner 跨越 HTTP handoff

- 原请求先等待正常管理 UI／API 的独立一次性批准，再取得并发 permit；没有闲置购买入口。
- `paid::Input` 保存所需的 owned 快照；JSON wrapper 与实时 source 都调用同一 `execute_observed`。购买前后的费用、资格、RequestBinding／Grant／revision、原 claim、START、model、journal 和 FINISH 不另造实现。
- `RelayStep` 保留合法 bool `stream`，原 serializer 签最终真实请求 bytes；SSE 调用确实向同一个模型请求原生流，不是在网关缓冲完 JSON 后模拟 token。
- consumer 轮询 `Source` 时，同时轮询原付费 future 与容量为 1 的 mailbox。没有 `tokio::spawn` producer；原 future 内的 observer 等待 mailbox，背压计入原步骤预算。
- 并发 permit 由该原 future 持有至完成／失败／Drop，而不是发出 HTTP headers 后就提前归还。

## 预算能力与取消

只有原购买已计入 attempt 后，才能取得绑定该 attempt 序号的 `AdmittedStream`。它只允许原操作在 handoff 后继续阶段检查，仍服从 first stop cause 和原 deadline；不同 attempt 不能沿用。普通 `check`／`check_inflight`／`begin_attempt` 和新的 capability 获取仍保留 handoff 围栏，不新增发送槽或延长期限。

已确认 START 后，原执行 owner 复用既有最多 20 秒的显式 closeout。caller、watch channel 关闭、shutdown 和 deadline 在原 decoder／observer 中协作停止，并返还原 run；已观察费用走原 FINISH，未知费用拒绝 FINISH，不填 0、不重放、不重买或自动 end。FINISH 已在途时等待同一次 ack，不靠同步 stream callback 补异步 FINISH。

FreeBuff source 不套用会到期先 drop inner 的通用 `BudgetStream`；普通 stream 仍使用该 wrapper，不能借本次接线放宽其他渠道。

真正 Drop 同步销毁原 future、上游 body、mailbox 与 permit，没有后台补偿或续跑。停止 poll 且不 Drop 也不能承诺主动推进 FINISH；协作取消、consumer Drop 与进程退出不是同一个保证。已持久的 claim/run/费用事实保留，不因此恢复执行权限。

## provisional 帧与唯一成功终态

1. 原 native decoder 逐帧严格校验，并先经原 journal 保存 changed reported cost，之后才调用 observer。
2. native 文本、`reasoning`／`reasoning_content`／`reasoning_details`、函数参数片段与工具扩展按原生 Chat 字段保留，不造 Anthropic thinking 或 signature。
3. provisional choice 的 `finish_reason` 固定为 null；native `[DONE]` 不向公共端透传。早到的 finish、DONE、usage 或完整工具参数都不单独宣告成功。
4. 只有真实上游 EOF、原 strict finish 与最终工具 JSON 校验通过后，才准备 late finish chunk 与 `[DONE]`。最终响应转换和累计序列化字节预算必须在原 FINISH 前成功；不能先报 completed，再发现无法输出。
5. 原 FINISH 和 journal 成功、原请求及动态 fence 仍有效后，才交付公共成功终态。上游尾部坏帧、未知费用、FINISH 拒绝、取消／期限均不能得到该终态，也不能重新提交模型请求。

若上游实际返回 JSON，则完整有界读取后将原 message 转成一次完整 delta，保留文本、reasoning 和工具字段，再交付终态。这是明确的 buffered fallback，不声称它有逐 token 的时序。

所有输出帧、JSON escaping、SSE framing 和最后 DONE 共用累计 64 MiB 上限；不是只限制单帧，也不因 mailbox 有界就放弃累计预算。未知 token usage 保持未知；reported SDK credits 继续来自原 accounting，不当成真实供应商账单。

## 公共终态只有一份

普通 stream 与此 catalog stream 都调用 `streaming::terminal::track`，复用原 metrics、latency、健康／circuit、usage、completion semantics、archive、审计及扣费／退款 owner。没有第二份 FreeBuff 公共结算回调；旧 lane 的 lease release 不传给此 catalog lane。

严格 owner 仅在 source **实际 yield 最后终态 bytes** 时设置 validated-terminal flag。预取、FINISH 完成或把终态放入 mailbox 不算公共消费。因而 consumer 读到 DONE 后直接 Drop 可以完成一次正常终态，早 Drop 仍按消费中断退款；错误仍通过原 typed stream error 与唯一 callback 处理。

## 接力边界

Chat 原子批离线证据和真实失败分类见 [公共 Chat SSE 回执](progress/2026-11-08-provider-channel-expansion-freebuff-public-chat-sse.md)，Responses／Messages 的后续证据见[三协议接线回执](progress/2026-11-08-provider-channel-expansion-freebuff-public-protocol-sse.md)。完整参数／模态、实际工具与 child 执行、未知 START／FINISH 结果显式对账和安全恢复、远端 lease、Server、独立进程／原生包及真实平台仍须继续；限定正常浏览器单轮文本流式联合见本合同开头的新回执。

模板仍为 17 个，无新增整卡 `offline_ready`；P50／P60／P70 和总目标未完成。认证、额度池、账号权限与存储版本未因本合同改变。
