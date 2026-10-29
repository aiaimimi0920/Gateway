# Search / Fetch 工具接口

## 通用路由与专用操作

`POST /v1/search` 使用 `model: "search"`，`POST /v1/fetch` 使用
`model: "fetch"` 时，Gateway 从当前路由配置中选择能执行该操作的账号。
这两个名称是保留的工具池标识，不参与模糊别名匹配，不是聊天模型。
不传 model 的历史默认路由保持兼容；需要通用路由时应显式传这两个名称。

```http
POST /v1/search
Authorization: Bearer <Gateway API Key>
Content-Type: application/json

{"model":"search","query":"Rust official documentation"}
```

```http
POST /v1/fetch
Authorization: Bearer <Gateway API Key>
Content-Type: application/json

{"model":"fetch","url":"https://www.rust-lang.org/learn"}
```

原有的 `you-search`、`exa-fetch` 等专用操作仍可显式选择供应商。新增
`you-fetch`、`tavily-fetch`、`websearchapi-fetch`。模型列表里的这些名称是操作路由，
不能通过 `/v1/chat/completions` 把它们当作聊天模型调用。

## 选择与授权

通用路由在编译路由快照时建立内存映射，不改写原始配置、数据库或凭据。
只有声明了相应专用操作或通用操作、且提供对应接口路径的搜索供应商才参与。
候选先经过 Key 权限、账号分组、具体凭据的 supported_models、禁用、冷却、
额度和协议能力检查，再沿用现有健康排序、会话黏性与有限重试。

`search` / `fetch` 表示调用方请求在合规工具池内选择；可在该候选池中故障转移，
不会跨越 Key 的授权账号范围。没有合规候选时返回失败，不回退到聊天供应商。
不会自动发起质量评测或付费探测，也不声称对不同搜索厂商有独立质量排名。

新增模板声明通用操作；旧配置中的专用操作也可派生通用内存映射。
如果旧账号明确只允许 `you-search`，不会自动给它增加 `you-fetch` 权限。
应通过现有账号/凭据池编辑器声明需要的操作，不能通过通用名称绕过限制。
You Contents 还需要上游产品 scope；Search 成功不代表 Contents 权限已通过。

`search` / `fetch` 保留在 API 模型目录和路由中，不在服务商或账号卡牌中作为独立模型显示。
卡牌仅展示具体操作；通过通用入口产生的调用数、成功率、半小时热度图归到实际执行的操作。
审计保留原始 requested_model，同时以具体 resolved_model 聚合，一次逻辑请求不重复计数。
全部上游尝试失败时，归属最后实际开始发送的服务商、具体操作和凭据，不伪造成功候选；
未真正开始发送的候选不会覆盖已发生调用的归属。历史缺失归属的数据不凭当前配置猜测回填。

## 厂商请求与响应

| 专用 Fetch | 上游接口 | URL 字段 | 默认输出 |
| --- | --- | --- | --- |
| you-fetch | POST /v1/contents | urls 数组 | formats: [markdown, metadata] |
| tavily-fetch | POST /extract | urls 数组 | 使用上游默认正文格式 |
| websearchapi-fetch | POST /scrape | url | returnFormat: markdown |

Fetch 只接受 HTTP(S) URL。通用批量请求最多 20 个；单 URL 厂商不会静默只取第一项，
而是在候选筛选阶段排除。指定单 URL 厂商却提交多 URL，直接返回参数错误。
正文响应读取上限为 8 MiB。新接入的三个接口会校验正文：HTTP 200 但空结果、
全失败、无效 JSON 或业务错误不能算成功。Tavily 的部分成功仍保留 results / failed_results；
You 的批量返回可能缺少条目，调用方仍需核对请求 URL 与响应 URL。

当前响应保留供应商原生 JSON，不把通用 model 误称为统一结果 schema。
通用请求建议使用 query 或 url/urls 这些公共字段；需要某厂商的特殊参数和
返回格式时，优先使用对应专用操作，避免跨厂商参数含义不同。

WebSearchAPI Search 的 includeContent / includeAnswer 仍作为上游参数透传，
不等于 Gateway 自动执行一次独立 Fetch。搜索附带正文与答案生成未做付费验收。

`linkup-balance` 及旧共享余额工具标识不再对外列出或路由；公共 credits/balance
接口返回明确的工具移除错误。内部运维余额查询与 Gateway 用户计费体系保留，
不删除历史审计和持久化配置。

## 探测与自动使用边界

新增 Fetch 操作的手动账号探测使用指定 URL、指定凭据和 Fetch 协议记录，不用
搜索成功冒充读取成功。新模板默认测试仍为专用 Search，automaticEnabled=false，
不会因为增加通用别名而重复产生上游调用。

这两个接口是工具执行后端，不包含完整 Agent 工具调用循环。普通聊天不会自动
搜索，也不会搜索后自动读取；上层应用需要声明工具、处理模型 tool calls，
调用这些接口，再把结果交回模型。修改 UI 分类名为 tools 不改变这个边界。

## 官方依据与本地验证

- [You Contents](https://you.com/docs/api-reference/contents)
- [Tavily Extract](https://docs.tavily.com/documentation/api-reference/endpoint/extract)
- [WebSearchAPI Scraper](https://websearchapi.ai/docs/scraper-api)

本地契约测试使用 loopback 模拟上游：`tests/search_fetch_tools_contract.rs`。
路由、分组、凭据模型限制及无聊天回退测试在
`src/routing/config/tests/search_tools.rs`。不需要真实上游 Key 或付费业务请求。
