# 官方站点健康适配器

2026-10-23：官方站点采用手选的独立适配器，不加入通用自动扫描，也不借用业务凭据。
选择后自动填写固定地址和无需认证；禁止自定义 origin、私网许可、监控分组和模型映射。
既有凭证组、刷新间隔、有效期及独立准入策略不变。旧监控密钥仍必须通过已有授权 Clear
流程清除；切换适配器只丢弃未提交明文，不擅自删除持久密钥。

## 已核验来源与读取方式

| 适配器 | 固定地址 | 读取 | 数据范围 |
|---|---|---|---|
| `openai-status` | https://status.openai.com/ | `/api/v2/components.json` | 仅 Codex API |
| `anthropic-status` | https://status.claude.com/ | `/api/v2/components.json` | Claude API、Claude Code |
| `deepseek-status` | https://status.deepseek.com/ | `/api/status-page/6410630422455/summary/active` | Flashduty API 组件和当前变更 |
| `nvidia-status` | https://status.ngc.nvidia.com/ | `/api/v2/components.json` | NVIDIA Build、Cloud Functions |
| `gemini-status` | https://aistudio.google.com/status | 官方页面 bootstrap + ListIncidentsHistory RPC | Gemini API 轨道，不混入 AI Studio UI 事件 |
| `openrouter-status` | https://status.openrouter.ai/ | `/config.json` | Datadog API - Gateway 子组件，不混入网页故障 |
| `cohere-status` | https://status.cohere.com/ | `/api/v2/components.json` | All Endpoints，不推断具体模型 |
| `groq-status` | https://groqstatus.com/ | `/api/v2/components.json` | API |
| `poe-status` | https://status.poe.com/ | `/api/v2/components.json` | api.poe.com，不混入 poe.com 网页 |
| `perplexity-status` | https://status.perplexity.com/ | `/api/v2/components.json` | API，不使用旧 .ai API 路径 |
| `linkup-status` | https://status.linkup.so/ | `/api/status/summary.json` | Search API、Fetch API |
| `exa-status` | https://status.exa.ai/ | `/v3/components.json` | Search API 及其明确 parent ID 下的子组件 |
| `jina-status` | https://status.jina.ai/ | `/api/v2/summary.json` | r.jina.ai、s.jina.ai，分别保留观察 |
| `together-status` | https://status.together.ai/ | `/index.json` | Better Stack 资源状态，排除 Website、Playground |
| `mistral-status` | https://status.mistral.ai/ | `/api/v2/components.json` | Completion API |
| `xai-status` | https://status.x.ai/ | `/api/status` | API 产品及明确的 Global / US / EU API 组件 |
| `tavily-status` | https://status.tavily.com/ | `/api/v2/summary.json` | Tavily API Service，不混入 Website、MCP |
| `you-status` | https://status.you.com/ | `/api/v2/summary.json` | Search/Index、Live News、Agentic Search API |
| `azure-status` | https://azure.status.microsoft/en-us/status | 同路径 HTML 区域矩阵 | Azure OpenAI Service，按 zone 和 region 分别显示 |
| `qwen-status` | https://status.aliyun.com/ | `/api/status/listEventInProgress` | 阿里云公开公告；百炼产品/地域覆盖未确认，不推断模型健康 |

目录扩展的官方来源、联网证据和暂不能可靠接入的服务商见
[服务商健康覆盖核查](official-site-health-coverage.md)；后续五家实现见
[剩余官方状态站适配](official-site-health-remaining.md)。目前共 20 个官方适配器。

以上均为公开状态读取，不发模型推理请求。DeepSeek 的旧 Statuspage summary 已返回 404；
OpenRouter 的旧 summary 返回 403，不能套统一路径。NVIDIA 使用 NGC 官方状态站，
而不是猜测 `status.nvidia.com`。Claude 不将 `claude.ai` 网站状态当 API 状态。

Gemini 没有本轮确认的稳定匿名 GET JSON feed。实现读取官方页面 `application/json` 中
顶层 `WIu0Nc` 公开应用 key，不硬编码该值、不执行页面脚本。仅向固定
`https://alkalimakersuite-pa.clients6.google.com/` 的
`$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/ListIncidentsHistory`
发送只读 POST `[]`，使用 `application/json+protobuf` 和固定 Referer。
这是官网前端的内部 RPC，不承诺长期稳定。接口漂移、缺失 bootstrap、HTML 冒充 JSON、
超时和未知状态全部保持未知，不能回退为正常。

## 状态与模型准入

新增可选 `probes[].observations`，包含 `name`、`status`、可空的 `model`。
旧 JSON 继续可读，旧模型百分比字段不变。官方状态不虚构成 100%/0% 成功率；
官方适配器使用分类状态，因此 UI 禁用百分比阈值输入，但保留原数值供切回通用适配器。

- ChatGPT 特例：经用户明确批准，仅 `Codex API` 状态统一用于本方案覆盖的全部模型。
  UI 明示统一继承；不是逐模型测量。`operational` 放行，明确降级/中断/维护拒绝；
  未知、缺失、过期和 binding 改变按 `unknown_policy`。`advisory` 不拦截。
- DeepSeek：通过官方模型文档 https://api-docs.deepseek.com/ 和
  https://api-docs.deepseek.com/api/create-response 核对模型身份，再绑定状态组件。
  `01KY4MVS8BM3F9JSYWACGQVG7A` + 精确名称绑定 `deepseek-v4-pro`；
  `01KY4MVS8BSBSVW6053QJ37RJE` + 精确名称绑定 `deepseek-flash`，以及官方明确继续接受的
  `deepseek-v4-flash`、`deepseek-v4-flash-vision-exp` 旧别名。
  不凭显示名生成 `deepseek-v4.1-flash`。组件改名或 ID 不符时保留组件信息但不强行匹配模型。
- 其他组件没有官方明确模型身份时仅报告组件状态；模型级信息缺失仍按未知策略处理，
  不把 OpenAI 特例扩大到所有厂家。选择 restrict + unknown block 会阻止缺少模型数据的模型。
- 不修改人工启用/禁用、真实调用熔断、业务密钥、quota 或测试结果。站点恢复只解除自己这一层限制。

DeepSeek 当前无 active change 的公开响应已实测；非空结构依据官方历史 change 的
`affected_components` 合同，活跃 payload 若改变则报告未知。多个公告采用最严重状态，
未知或较轻的后续公告不能覆盖已知中断。处理有总条目上限，不做组件×公告×条目的无界遍历。

OpenRouter 另有按模型 endpoint 的 `uptime_last_30m` 等数据；本轮适配的是官方站点状态，
未扫描其全部模型与上游 endpoints，也不把各 endpoint 的分钟可用性随意汇总为模型成功率。

## 安全和验证

固定官方入口复用既有环境/Windows 代理和 bypass；自定义站点仍禁止代理。所有入口禁止重定向，
直连采用 DNS 校验并 pin 地址，普通响应 1 MiB、单请求 4 秒、
整体 25 秒；仅 Azure 约 7 MiB 的公开矩阵采用 8 MiB、30 秒请求和 35 秒整体预算，
HTML 在有界后台解析任务中处理，不扩大其他适配器限制。Gemini 第二 origin 同样独立校验，绝不信任页面中的动态目的地址。
报告不包含 bootstrap key、监控 key 或业务 key。路由热路径仍只读缓存。

离线验证：`cargo test --locked --lib site_health::`、`site_health_official_contract` 和既有
站点健康合同；前端 `officialHealthAdapters.test.tsx` 及既有方案/密钥回归。
显式真实公开状态验证（默认测试不会联网）：

```powershell
cargo test --locked --test site_health_official_contract official_public_status_smoke -- --ignored --nocapture
```

本功能不是生产部署或正式发布，不改变运行中的 Gateway、凭证和数据库。

### Fake-IP 恢复与代理信任边界（2026-10-26）

`198.18.0.0/15` 仍不作为真实目标连接，`allow_private_network` 也不豁免。系统 DNS
非空、最多 16 项且**全部**为该网段（含 IPv4-mapped IPv6）时，才尝试恢复；IP 字面量、
单标签及 `.local`/`.internal`/`.home.arpa` 等本地名称不发给公共解析器，混合禁用地址不丢弃后放行。

恢复使用固定 `https://dns.google/resolve`，仅该固定解析器可复用既有可信代理；无重定向，
直连/bypass 时以 `8.8.8.8:443` 固定 bootstrap，
保留正常 TLS 域名/证书校验。仅发送目标 hostname 和 A/AAAA 类型，不发送 URL 路径或任何账号/监控密钥。
每请求 4 秒、16 KiB，A/AAAA 并发，每类仅瞬态 I/O 错误可再尝试一次（总计最多两次），
恢复请求等待最多 8 秒且受原有整体检测预算约束；校验状态、question、记录类型、全部返回地址和总数，
最多保留 16 个去重公网地址。失败保持未知，不回退连接 Fake-IP。恢复会让 Google Public DNS
看到查询域名；不改系统 DNS、hosts 或代理设置，也不是绕过网络封锁的保证。

自定义站点始终 `no_proxy`，按恢复后的公开 IP 固定连接，并保留原 Host/TLS SNI。
官方入口由 `HealthAdapter` 固定表选择，Gemini 第二 RPC 域名也是单独的编译期常量，
不接受配置或响应中的动态 URL。它们在 DNS 校验后可复用已有代理合同；代理启用且未 bypass 时，
**远端解析及最终出口由用户配置的代理负责，不能宣称最终代理目标 IP 也被本地 pin**。
无代理或 bypass 时仍按已校验的地址直连，不因官方身份放开保留网段。官方请求不带业务或监控密钥；
Gemini 仅带页面提供的公开 bootstrap key。超时、响应上限、禁重定向及整体检测预算保持不变。
