# FreeBuff 公共 Responses／Messages 实时 SSE 合同

后续验证（2026-11-08）：[正常网页与三协议活 SSE 联合](progress/2026-11-08-provider-channel-expansion-freebuff-public-sse-ui.md)已覆盖 Local 单轮文本的正常逐次批准/取消、EOF 前增量、原 FINISH/journal 阶段的页面读取、显式原 end/再购买与 Local 结算；不是完整渠道或真实平台验收。

状态：限定文本／函数工具子链已完成无凭据实现及离线验证。最终 FreeBuff 508 passed / 0 failed / 4 ignored，共享预算／tracker／lifecycle、Bedrock 公共 TCP、两生产 lib、格式／行数／来源检查均通过；具体证据见[本批回执](progress/2026-11-08-provider-channel-expansion-freebuff-public-protocol-sse.md)。不是整卡或真实平台验收声明。

本渠道只接受已有文本／函数工具闭集，原始 `stream` 必须是合法 bool，并由转换器保留到原 `RelayStep` 的最终签名 bytes。其他未知字段、模态、内建工具和服务端 continuation 仍在报价前明确拒绝，不因流式模式悄悄丢弃。

## 唯一执行与终态

Responses、Messages 和既有 Chat 共用 `paid::execute_observed`、同一原持久 claim/run/journal、签名模型发送、费用 FINISH 与 Local 公共结算；消费者同次轮询原 future 与容量 1 mailbox，不新增 producer、购买、重试或恢复权限。permit、原 attempt continuation、deadline、协作取消、真正 Drop 和未知费用保持 [Chat SSE 合同](provider-freebuff-public-chat-sse.md) 的边界。

native finish/DONE 仍只是 provisional 观察。必须经过真实 EOF、完整 native decoder、最终协议投影、累计序列化预算及原 FINISH/journal 与动态 fence，才交付公共成功终态。投影失败必须在原 FINISH 前已知，不能先按 completed 收尾再发现协议不可表达；不在同步 tracker callback 中补异步 FINISH。

## 增量投影与原生扩展

- Responses 使用标准 created/in_progress、output_item/content_part/text 和 function_call_arguments 增量事件。最终使用完整 response、稳定 item ID 与对应 terminal status；`length`/`content_filter` 不被改写为 response.completed。
- Messages 使用 message_start、带 index 的 text/tool_use 块与 text_delta/input_json_delta。块停止、message_delta 和 message_stop 均位于原 EOF/FINISH 后的同一晚置尾部。原生 reasoning 不转成伪 thinking/signature。
- 公共 response/message ID 由原请求固定，不随着 native ID 迟到而变化；native ID/model 仍在扩展中保留。工具的真实 call ID 不改名；公共 item/block 顺序按首次交付固定，不按晚到的 native 稀疏 index 重排已经发送的事件。
- 工具关联与原 strict decoder 共用同一个 index/ID/name 规则。若 ID 晚于参数，原 ID 到来前仅保留未交付参数，不伪造 caller 无法用于下一轮的 call ID。最终仍要求原严格工具 JSON 和协议输入类型；Messages 不接受数组充当 tool_use.input。
- 每个真实 native frame 通过 `gateway_freebuff.delta` 显式扩展事件保留：`version=1`，原始已解析 frame 位于 `frame`。其中原生 finish、usage、费用及 opaque 数据不是公共成功授权，也不表示实际执行了 caller 工具。
- 最终 `gateway_freebuff` 继续使用既有严格 JSON 投影保留 native choice/message/费用与工具扩展；工具扩展与公共工具顺序对应，不能因稀疏 index 或首见顺序不同而错配。Responses 位于最终 response；Messages 位于晚置 message_delta。

上游若实际返回 JSON，则在有界完整读取后一次投影完整文本／工具，不宣称逐 token 时序，也不伪造“观察到的 native SSE frame”。未知 usage 不补 0，部分 usage 保持部分；reported SDK credits 仍由原 accounting 负责，不改成 Token 估价。

所有增量、扩展、escaping、SSE framing 与晚置终态共用累计 64 MiB 输出预算，不只限制单帧。工具 ID/名称/稀疏 index 的状态有原 1024 工具上限，未交付参数也受原累计 native body 预算约束；没有无界事件队列。

## 公共审计与未完成范围

公共 metrics、archive、usage、费用／退款和健康结果继续由唯一 `streaming::terminal::track` 负责。原付费执行已知的 native completion semantics 随同 usage 交给该 owner，不能从有损的公共 incomplete/stop_reason 反推原始 length/content_filter；普通 stream 仍使用原观察器，不因本渠道改放宽。

只有 source 实际 yield 最后验证过的 bytes 才设置 terminal flag。预取或排队不当消费；读到标准终态后直接 Drop 可完成一次正常结算，早 Drop 保留原持久事实并退款，不后台补 FINISH/end、重放或重买。

完整参数／模态／实际工具与 child、持久完整 history、未知 START/FINISH 的显式对账与安全恢复、远端 lease、精确 probe、新设备/目录应用的正常 UI 联合、Server、跨进程／原生包及真实平台仍须继续；已验的正常浏览器单轮文本 SSE 联合不再全部列为缺口。模板数仍为 17，无新增整卡 offline_ready；P50/P60/P70 和总目标不据此完成。
