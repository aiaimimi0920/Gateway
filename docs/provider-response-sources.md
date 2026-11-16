# Gateway 来源列表扩展

> P39 合同：来源列表不是 inline citation annotations。当前接线与验证进度见本批回执。

## 公共形状

`gateway_sources` 是 Gateway 明确命名的扩展，不是 OpenAI 标准字段，也不是厂商签名或事实核验声明。每项只包含 `name`、`url`、`snippet` 三个字符串；保持顺序、重复项和原字符串，不抓取 URL，不把摘要拼进答案，不制造 citation offset、工具调用、模型身份或 token usage。

```json
{
  "gateway_sources": [
    {"name": "来源标题", "url": "https://example.org/article", "snippet": "来源摘要"}
  ]
}
```

- Chat/Responses 非流式 JSON：响应对象顶层。
- Chat SSE：最终带 `finish_reason` 的 chunk 顶层，只发送一次；不是文本 delta。
- Responses SSE：最终 `response.completed` / `response.incomplete` 的 `response` 对象顶层。来源归属整份答案，不附在某个 `output_text`，因此 `content_part.done` / `output_item.done` 不重复携带。
- 内部 canonical：独立 `sources` 集合，旧对象缺省为空，空集合不序列化。公共投影为空时也省略扩展，保留旧无来源输出。

来源字符串是不可信内容，消费者须按文本/链接正确转义；不要当 HTML、脚本、命令或自动抓取目标执行。HTTP(S) URL 不代表安全站点或授权访问目的地。

## 边界与限制

跨 Gateway 解包时，只有这个精确扩展名被读取；不猜测上游同名 `sources`、`citations` 或 arbitrary metadata 的含义。不投影条目中的其他字段。来源最多64项，每项 URL2048、name1024、snippet8192 UTF-8字节；URL须有host且无userinfo，仅HTTP(S)。已声明但类型、字段或预算无效的扩展明确报错，不静默清空。

Chat→Responses 转换器只接收带终止原因的来源终帧，不累加历史快照。此帧自身可以携带最后正文/工具 delta；一旦冻结来源，等待 usage 期间也不再接受后续正文/工具修改，但允许 usage-only 尾帧。本转换器发出终态前，第二份列表或非终帧来源会中止转换，不能借此将旧来源绑定到新答案。终态后仍沿原有 drain 合同读取剩余传输，不解析或验证后续材料，也不修改已经交付的来源。原生 Responses SSE 仍按原有 passthrough 合同运行，不替换为重打包。

Perplexity 当前仅有最终 snapshot 的来源列表，没有可信的文本偏移；因此不能改称标准 `annotations/url_citation`。它的最终答案映射保持 usage 和实际模型声明未知。`turbo` 是 pinned 请求策略，不证明实际后端模型身份。

本合同的验证面是 Chat/Responses JSON、聚合后的 Chat SSE、Responses SSE及其反向解包/聚合；不据此外推 Anthropic、Gemini 或实时 token streaming 已支持。Perplexity dispatcher、普通模板、材料提交、精确probe和公共HTTP集成仍须分别接线验收。这里不修改现有 native JSON passthrough，不增加独立来源数据库或缓存。
