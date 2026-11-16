# FreeBuff 目录业务的公共非流式 JSON 子集

本合同描述 Gateway 的明确支持范围，不是当前线上 FreeBuff 合同或完整渠道验收。真实平台、Server、原生包和实时 SSE 仍须分别验证。

## 入口与执行责任

`/v1/chat/completions`、`/v1/responses`、`/v1/messages` 共用既有 Local 目录业务执行链：正常调用者鉴权 → 原请求绑定 → 报价 → 正常管理端逐次明确批准 → 原持久 claim → 原 signed START/model → 显式 FINISH → Local 结算。

公共请求不会注册设备、认领旧 claim、执行 caller 工具、自动 end、重买或重放。等待批准不持有提供商并发许可；原 Grant、revision、候选、费用、取消和期限围栏不因协议转换而改变。FINISH 的 HTTP 接受不是最终结算 receipt。

转换仅适用于显式目录业务目标；不改写旧 FreeBuff 执行器、认证方式、模板数或额度池。

## 输入子集

先检查原 `raw_body` 的闭合集合，再转换到既有 `RelayStep` 并再次使用其 native 校验。不能先用宽松 normalize 丢掉字段，再声称输入受支持。原消息、工具、参数等 canonical 语义若已被 policy 修改，本 lane 拒绝 raw 恢复，不能绕过该修改。

三入口均要求非流式；`stream=true` 或非布尔形态拒绝。Chat 保持既有 native 文本／function-tool 子集。未知顶层字段、未知嵌套字段、不可表达的块和无效参数在目录刷新、报价和购买前拒绝。

### Responses

支持的顶层字段为 `model`、`input`、`instructions`、`max_output_tokens`、`temperature`、`top_p`、`tools`、`tool_choice`、`parallel_tool_calls`、`reasoning`、`user`、`stream`。

- `instructions` 为字符串；`input` 为字符串或消息／函数历史数组。
- 消息角色支持 `system/developer/user/assistant`，保留顺序，不把多个 system 搬到第一条或合成一条。system/developer 内容仅支持字符串；user/assistant 支持字符串或严格的 `input_text/output_text` 文本块。
- 支持 `function_call` 的 `call_id/name/arguments` 和 `function_call_output` 的字符串 `output`。arguments 原始字符串不被 HTML 解码或 JSON 规范化。可选函数 item `id` 仅接受与 `call_id` 相同的别名，`status` 仅接受 `completed`。
- `tools` 仅普通 function，保留 name、description、parameters、strict；tool choice 使用 native selector 校验。
- `reasoning` 仅支持 `{ "effort": ... }`，映射到 native `reasoning_effort`；`max_output_tokens` 映射到 `max_completion_tokens`，不凭 opaque 模型名猜测。
- 不支持 `previous_response_id`、server store/background、built-in/custom tools、reasoning 历史、搜索、附件或其他模态；不假造这些状态或存储保证。

### Messages

支持的顶层字段为 `model`、`messages`、`system`、`max_tokens`、`temperature`、`top_p`、`stop_sequences`、`tools`、`tool_choice`、`stream`。`max_tokens` 必须为正整数。

- system 字符串或 text 块数组逐块保留；框架允许的 in-sequence system 也保留原位置。
- user/assistant 支持字符串、text 块、assistant `tool_use` 和 user `tool_result`。按原块顺序生成 native 消息段；混合 text/result 不扁平化为一个字符串。tool input 必须为 JSON object，tool result 支持字符串或纯文本块。
- tools 支持 name、description、input_schema；tool choice 的 auto/any/tool/none 与 `disable_parallel_tool_use` 显式映射。
- 不支持 thinking/signature、cache_control、`is_error`、server tools、多模态工具结果或其他未知语义；不能用普通文本代替它们。

映射后的 native 字段继续受 `RelayStep` 的数量、名称、schema、参数、上下文预算和最终序列化字节预算约束；不会因为跨协议而放宽 native 合同。

## 输出与原生扩展

Chat 返回已严格解码的原生响应，不再为观察指标对答案文本做 XML 工具解析。Responses/Messages 只接受可表达的单 choice，显式映射文本和函数调用，复用框架 envelope／finish 映射。

原生 tool ID 缺失／重复、无法表达的多 choice，或 Messages 工具参数不是 JSON object 时，公共调用失败，不默认补 `{}`。投影发生在显式 FINISH 之前：仍使用原模型费用 ledger，FINISH 标记 failed，不重试、重买或释放 claim。

标准输出之外保留 `gateway_freebuff` 扩展：

```json
{
  "version": 1,
  "completion": { "id": "native-id", "model": "native-model", "usage": {} },
  "choice": { "index": 0, "finish_reason": "stop" },
  "message": { "role": "assistant", "reasoning_content": "...", "reasoning_details": [] },
  "tool_calls": []
}
```

- `completion` 保存移除 choices 后的 native 顶层字段，包括原 model、usage、费用明细和未知元数据。
- `choice` 保存移除 message 后的 native choice 字段，包括原 finish reason。
- `message` 保存已投影 content/tool_calls 之外的字段，包括 reasoning、reasoning_content、reasoning_details、refusal 和其他不等价数据。它们不是 Anthropic thinking/signature。
- `tool_calls` 按 native tool ordinal 保存移入标准字段之外的原工具扩展，如 extra_content、未知 function 字段。Messages 另外保存原始 `function.arguments` 字符串，因为标准 input 是解析后的 JSON object；Responses 的 arguments 本身保留原字符串。

这不是完整原生 body 的第二份副本，也不是可重放历史、认证凭据或新的执行能力。新跨协议请求仍按上述输入闭集校验，不接收此扩展作为隐藏 continuation。原始 null/省略等结构差异不承诺逐字可逆。

usage 只投影实际已报告的 token 字段，缺失继续缺失，不补 0。原费用报告完整保留在扩展中；token usage、reported SDK credits、session 钱包支出和最终结算不是同一概念。输出序列化字节也受现有上游 body 总预算限制，计数不额外建立完整序列化缓冲。

## 仍未支持的范围

实时三协议 SSE、完整模态／参数、跨协议原生 reasoning 历史、完整工具／child 执行、远端 lease、START/FINISH 未知结果的显式对账与安全续跑、Server/PG、独立进程／原生包和真实平台验收仍未完成。非流式 JSON 支持不能用来宣称这些能力已完成。

实现入口：[公共协议接缝](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/freebuff_catalog/protocol.rs)、[原 paid owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/freebuff_catalog/paid.rs)。
