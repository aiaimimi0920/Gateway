# Vertex Express 有序签名 turn 扩展

本扩展只适用于 `vertex-express-api` 的固定 v1 文本／完整工具调用子集。
它不是 Anthropic 原生 thinking/signature、OpenAI reasoning/encrypted_content，
也不是 Google Interactions API。普通公共文本和工具字段继续保持旧协议形状；
**原始 Part 的顺序、边界、thought 和签名以扩展为准**，不能从合并后的文本重建。

## 输出与输入

三公共协议的非流式响应根对象可带 `gateway_vertex_turn`。
Chat SSE 在 finish chunk 根对象、Messages SSE 在 `message_delta` 根对象、
Responses SSE 在终态 `response.completed.response` 中各带一次相同扩展。
这条 Express 路径仍先聚合再桥接，不是实时 Google Part 或参数增量输出。
未带思考状态且没有 text/tool 混合顺序需要保留的旧响应不新增扩展。

示例（签名仅为合成 base64，不是可用平台材料）：

```json
{
  "gateway_vertex_turn": {
    "source": "vertex_express_api",
    "model": "gemini-example",
    "parts": [
      {"text": "摘要", "thought": true, "thought_signature": "YWJj"},
      {"text": "先查"},
      {
        "function_call": {"name": "lookup", "args": {"city": "杭州"}},
        "public_id": "call_public_example",
        "thought_signature": "ZGVm"
      },
      {"text": "", "thought_signature": "Z2hp"}
    ]
  }
}
```

调用者照常保存公共 assistant 文本、调用 ID/名称/参数及工具结果，
再把原扩展加入下一次请求根对象的 `gateway_vertex_history`：

```json
{
  "gateway_vertex_history": [
    {
      "assistant_turn": 0,
      "turn": {
        "source": "vertex_express_api",
        "model": "gemini-example",
        "parts": [
          {"text": "摘要", "thought": true, "thought_signature": "YWJj"},
          {"text": "先查"},
          {
            "function_call": {"name": "lookup", "args": {"city": "杭州"}},
            "public_id": "call_public_example",
            "thought_signature": "ZGVm"
          },
          {"text": "", "thought_signature": "Z2hp"}
        ]
      }
    }
  ]
}
```

`assistant_turn` 从 0 开始，按 Express 已准备历史中的 model Content 顺序计数；
system 和 user/tool-result 不计入。Responses 将一轮普通 assistant message 和相邻
function_call items 表示的工具轮，按既有公共 ID 关联规则聚合为一个 model Content；
完成该轮的所有结果按调用顺序写入一个 user Content。条目必须严格递增且不能重复。
没有工具的独立 assistant message 各计一轮；不要仅按 Responses 的 output item 数计数。

恢复前要求扩展普通文本拼接等于该轮公共 assistant 文本，工具数量、顺序、公共 ID、
名称和 JSON object 参数均一致；不按名称／参数猜身份。显式省略的完整调用 args 可按既有
无参数工具语义与公共 `{}` 匹配，恢复时仍保持省略。模型必须精确等于本次实际发送模型，
不是用户别名或 modelVersion；这是 Gateway 的保守隔离政策，不是声称 Google 密码学绑定了模型。
不允许把此状态投给其他产品，不自动补签名、不使用跳过验证哨兵、不回退为无签名历史。

只有匹配的 model Content 会替换为原顺序 Google Parts；`public_id` 仅作本地关联，
不会发到 Google，v1 的既有本地工具结果排序／ID 移除合同不变。
该扩展不会包含 Key、OAuth grant 或任意 URL，也不会被作为上游 generation 参数发送。

## 不允许绕过的输入与桥接规则

- 原始 message Parts 不能直接携带 `thought`、`thoughtSignature` 或 `thought_signature`；
  也不能用 snake-case `function_call` / `function_response` 绕开 canonical 的公共 ID 关联。
  带状态的历史必须从上述 typed 扩展进入，且先通过文本、工具和实际上游模型一致性校验。
- 存在签名历史时，Chat/Messages 已会透传的原生 `contents` 覆盖在网络前拒绝；
  Responses 仍沿其关闭式 extra 白名单忽略该字段，不为此扩展透传范围。
  两条路径都不能借 `contents` 换掉已验证历史。
- 带 `gateway_vertex_history` 的 Chat 请求和带 `gateway_vertex_turn` 的 Chat 响应保留工具参数
  JSON 字面值，不执行旧服务商的 HTML entity repair。例如 `&amp;`、`&quot;` 不会被解码。
  带 typed turn 的响应也不把字面 `<tool_calls>...</tool_calls>` 文本注入成工具。
  没有扩展的旧修复行为保持不变。
- SSE 扩展只允许在合法 finish 位置出现一次，且不与文本／工具增量同帧；提前、重复、
  错误层级、投影与已输出内容不符或扩展之后又出现新内容均拒绝，不继续输出成功终态。
  Responses→Chat 的反向转换也对实际准备输出的 Chat 帧验证，而非仅信任 terminal snapshot。
  这些检查不重新解释 opaque native passthrough。
- 桥接以增量 SHA-256 摘要核对普通文本，工具 ID／名称／参数 witness 最多 4096 项、
  累计字符串不超过 4 MiB，不再额外保存普通全文。无扩展旧流超预算仅丢弃 witness，
  不改变旧流语义；随后若声明扩展则因无法证明一致性而拒绝。

## 支持边界与预算

- 支持原始 text／完整 functionCall、独立可选 thought boolean 和 Part 级 thoughtSignature。
  空 text 有意义，不能删掉；summary 不混入普通答案，也不冒充完整内部推理。
- 签名字符串只做有界 base64 语法检查，保持原字符串，不解密、不自行验签或解释内容；
  Google 的真实性、有效期、账号／project／Key 绑定仍须真实平台验收。
  `skip_thought_signature_validator` 和 `context_engineering_is_the_way_to_go` 明确拒绝，
  后者是 Gateway 的保守禁用政策，不据此声称已重新确证其当前 Cloud API 合同地位。
- 签名或 thought 与 Preview partialArgs／willContinue 重组并存时，因公开合同未定义片段归属而拒绝；
  无签名 Preview 子集沿既有 assembler，不伪称它的参数重组保留了原始签名 Part。
- 未支持的带状态媒体／代码／额外 Part 元数据、孤立 signature-only Part、
  带状态的未知完整调用字段均明确拒绝，不静默丢掉。
- 每 turn 最多 4096 Parts、序列化不超过 4 MiB，每签名最多 1 MiB；
  历史最多 64 条、整体不超过 8 MiB。超限或不匹配的输入在网络前 400；
  未能可靠投影的上游结果为静态 502、非重试 Abort。
- 安全过滤丢弃整个候选，包括普通文本、工具、summary 与签名；保留上游报告 usage。
- 此扩展不替普通客户端自动管理对话。客户端忽略扩展时仍可读旧文本／工具字段，
  但带签名工具的后续调用可能被 Google 以 400 拒绝；不能宣称签名往返得到保留，
  也不能把这一风险只描述为推理性能下降。使用的 SDK 必须显式保留并回传扩展。
- 多 candidate、全部模态、Full Vertex OAuth、真实材料／模型／额度验收独立追踪。

## 公开依据

2026-11-05 联网核实：
[thought signatures](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/thinking/thought-signatures)
要求原 Part 回传、不合并签名 Part，并明确流式末尾空 text 签名；
[Content REST](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/Shared.Types/Content)
及 [v1 content.proto](https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1/content.proto)
定义 Part 级 thought、bytes thought_signature 和互斥 data。
本批不据此推断所有 Express 模型都有签名，也不推断跨产品／账号共享。
同日尝试的旧 Gemini API 签名 URL 已重定向到 AI Studio Interactions 文档，
未再含第二个特殊字符串，因此不作为它的当前 Cloud 可用性证据。

实现与离线运行证据见
[签名 turn 子批回执](progress/2026-11-05-provider-channel-expansion-vertex-express-signed-turns.md)。
