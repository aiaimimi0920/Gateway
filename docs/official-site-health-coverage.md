# 服务商健康覆盖核查（2026-10-23）

本文件保留首轮扩展的调查与验证记录。后续已继续实现 xAI、Tavily、You.com、Azure、
百炼公告适配，当前共 20 个官方适配器；最新范围见 [剩余官方状态站适配](official-site-health-remaining.md)。

## 范围与实现

以实际 `apps/desktop/src/features/console/providerCatalog.ts` 为准核查，不扩充不存在的服务商。
在首批 6 个官方适配器基础上增加 9 个，共 15 个。每个新增项都支持固定地址自动填充、
无需认证及独立解析；仍不加入通用自动扫描。没有修改服务商目录、用户配置和运行实例。

所有新增观察都是组件级信息，`model=null`，不继承 ChatGPT 的 Codex 全模型特例。
在 restrict + unknown block 下，缺少模型身份的模型依旧按未知阻止；建议组件信息查看
使用默认 advisory。组件名称不是模型 ID，不能因为显示名称相似就自动产生模型映射。

## 新增项的官方证据

| 服务商 | 官方来源与真实读取证据 | 实现范围 |
|---|---|---|
| Cohere | [官方 FAQ](https://docs.cohere.com/docs/cohere-faqs) 明确状态站；`https://status.cohere.com/api/v2/components.json` 匿名 GET 200 | All Endpoints；不是单一 Chat 或某个模型 |
| Groq | [官方社区员工事故说明](https://community.groq.com/discussion-forum-7/gpt-oss-120b-ignoring-tools-256) 链接 groqstatus.com；`https://groqstatus.com/api/v2/components.json` GET 200 | API |
| Poe | 官方 poe.com 子域返回 Poe 品牌及独立 api.poe.com 组件；`https://status.poe.com/api/v2/components.json` GET 200 | api.poe.com，不使用 poe.com 网页状态；未找到其他官网文档反向链接 |
| Perplexity | [官方支持说明](https://www.perplexity.ai/help-center/en/articles/10354888-need-support) 链接 .com 状态站；`https://status.perplexity.com/api/v2/components.json` GET 200 | API；旧 .ai 首页重定向不意味着其 JSON 路径可用 |
| Linkup | [官网](https://www.linkup.so/) 链接状态站，状态站直接发布 `https://status.linkup.so/api/status/summary.json`，GET 200 | Search API、Fetch API；不混入 Application、MCP Server |
| Exa | [官网](https://exa.ai/) 链接状态站；[官方公共 API 文档](https://status.exa.ai/public-api) 发布 `https://status.exa.ai/v3/components.json`，GET 200 | Search API 和 group.id=`cmni631rn07mnhouiasltgt5q` 下的子组件 |
| Jina | [官网](https://jina.ai/) 链接状态站；`https://status.jina.ai/api/v2/summary.json` GET 200 | r.jina.ai 与 s.jina.ai；不以 Reader 分组覆盖两者 |
| Together AI | 状态页发布 `https://status.together.ai/index.json`，GET 200，响应 company_url 指向 together.ai | included 中 status_page_resource，排除 Website、Playground；不使用总体状态或 availability 百分比 |
| Mistral | [官方错误说明](https://docs.mistral.ai/resources/error-glossary.md) 指向状态站；[官网订阅 API 页](https://status.mistral.ai/subscribe?tab=api) 声明 JSON 接口 | Completion API；远程 Exa 实际抓取 components JSON，完整读到目标对象，但抓取输出被截断，本机 GET 403 |

前八家 HTTP 200 指调研用匿名请求，不代表 Gateway 的安全传输链路已验收。
Mistral 的远程抓取不是搜索摘要，但也不等于完整响应已在本机解析通过。

## 不同格式的处理

- Statuspage 形状仅复用解析机制；各厂商仍有明确组件白名单，不能读取任意网站组件。
- Linkup 的 `page.updated_at` 本轮为旧日期，不能把它当快照实时刷新时间；展示的是本次
  取回的官方发布状态，不承诺服务商发布数据没有延迟。
- Exa 组件响应实际是 `{components:[...group...]}`，官方文档示例的裸数组/isParent 有漂移。
  仅确认 `OPERATIONAL` 对应正常，其余状态保守显示未知；不把 incident.impact 枚举
  猜成 component.status。子组件保留 Search API 前缀，避免 Default 名称跨分组混用。
- Together 使用 [Better Stack 官方公开 JSON 合同](https://betterstack.com/docs/uptime/status-pages/subscribing-to-status-updates/subscribing-to-api/)：
  `operational`、`degraded`、`downtime`、`maintenance` 分别归一为正常、性能下降、中断、维护；
  `not_monitored` 和未来状态为未知。当前资源主要是模型展示名称，尚未核实 API 模型 ID，
  因此只显示资源状态，不建立模型准入映射。
- 响应继续有 1 MiB 和超时上限；组件/资源数组最多 512 项。空列表、目标缺失不判定健康。
  既有无代理、无重定向、DNS 地址校验及 pin 保持不变，不携带业务或监控密钥。

## 首轮调查时尚未接入的服务商（其中五家后续已实现）

| 服务商 | 官方入口／核查结果 | 暂不增加的原因 |
|---|---|---|
| xAI | [官方调试文档](https://docs.x.ai/developers/debugging) 发布 https://status.x.ai/ 与 `/feed.xml`；RSS GET 200 | 仅确认历史事故 feed，未确认当前 API 组件 JSON；无新事故不能等于正常 |
| Tavily | https://status.tavily.com/ GET 200，`/feed.rss` GET 200 | 事故 feed 可能是 Website，未确认当前 Tavily API Service 的结构化状态 |
| Azure OpenAI | https://azure.status.microsoft/ GET 200，精确服务 Azure OpenAI Service；官方 RSS GET 200 | 区域服务矩阵，未确认匿名区域 JSON；空 RSS 不代表特定区域或部署可用 |
| 百炼 / Qwen | https://status.alibabacloud.com/ 及官网 JS 可读；`/api/status/listEventInProgressInternational` GET 200 | 事故列表为空不能证明百炼健康；产品状态接口需要 regionId，百炼精确产品和区域尚未核实 |
| You.com | https://status.you.com/；搜索可见 APIs - Search/Index 等组件，本机 GET 403 | 未取得可验证的当前状态响应结构，不能用搜索摘要实现解析 |
| LongCat | https://longcat.chat/platform/docs/ GET 200 | 未确认官方状态站；第三方状态页和需业务 token 的 API 不替代官方匿名监控 |
| WebSearchAPI | https://websearchapi.ai/ GET 200 | 未确认官方状态站，不能混同 WebSearchAPI.tech 或第三方站点 |

上述是本轮核验边界，不声称这些厂商永远没有状态接口。未添加仅能显示假正常的占位适配器。

## 定向验证入口

```powershell
rtk proxy cargo test --locked -j 4 --lib site_health:: -- --test-threads=2
rtk proxy cargo test --locked -j 4 --test site_health_official_contract --test site_health_adapters_contract -- --test-threads=2
rtk proxy npm run test --prefix apps/desktop -- --run src/features/console/officialHealthAdapters.test.tsx
```

公开联网 smoke 现在汇总所有适配器的失败，不因首个 DNS 错误跳过其他厂商；
仍为显式 ignored 测试，普通回归不会联网。未放宽 Fake-IP 保留地址拦截。

### 本轮验证结果

- Rust 站点健康单元测试：12 passed。
- 六组站点健康集成合同：21 passed，1 个联网测试在普通回归中 ignored。
- 前端七个相关文件：28 passed；覆盖全部 15 个选项的实际选择和配置 schema。
- 前端 typecheck、build:web 通过；定向 rustfmt、`git diff --check` 通过。
- checker 自测 31 passed；有效行 ratchet 通过，governed source >700 为 0。
  本轮改动源码最大为 policy.rs 的 217 有效行；official.rs 120→137，parsing.rs 153→133。
- 显式执行联网 smoke 后，15 个入口均在 DNS 安全校验阶段失败：
  `Site resolves to a prohibited network address`。没有记为通过，没有修改 DNS 或放宽策略。
  日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-official-status-expanded-smoke-20261023.log`。
- 初次前端测试命令误从仓库根目录启动，未加载桌面项目的 jsdom 配置，出现
  `document is not defined`；改用上述 `npm run test --prefix apps/desktop` 后全部通过，
  未通过修改业务代码或测试断言绕过。

本轮没有生成交付包、替换运行实例、提交或推送；没有完整桌面人工验收。
源码测试和 Web 构建完成，不等于当前网络环境下的官方状态端到端验收完成。
