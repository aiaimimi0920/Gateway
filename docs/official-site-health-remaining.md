# 剩余官方状态站适配（2026-10-23）

用户要求继续实现已找到官方状态站的剩余服务商。本轮新增 xAI、Tavily、You.com、
Azure OpenAI 和百炼 / Qwen 公告适配，共计 20 个官方适配器。
仍未确认 LongCat、WebSearchAPI 的官方状态站，没有替换为第三方监控。

## 当前组件 JSON

- **Tavily**：`https://status.tavily.com/api/v2/summary.json` 本机匿名 GET 200，无重定向，
  精确选择 `Tavily API Service`。忽略 Tavily Website、MCP 和历史 scheduled_maintenances；
  过去维护条目存在不等于当前维护。官网 HTML 声明 `expose_status_summary_api: true`。
- **You.com**：`https://status.you.com/api/v2/summary.json` 经远程 Exa 实际抓取到完整 JSON，
  本机普通请求 403；选取 `APIs - Search/Index`、`APIs - Live News`、`APIs - Agentic Search`，
  不混入 Chat Product、Inference Router。公开历史 RSS 亦实测可读，但不作为当前状态兜底。
- **xAI**：`https://status.x.ai/api/status` 经远程 Exa 实际抓取到完整 JSON，本机普通请求 403。
  读取 `products[id=api,name=API]` 及 `api-global`、`api-us`、`api-eu-west-1`，
  不采用 overall、Grok 网页或 Services 状态。自定义合同只确证 `available` 对应正常；
  未核实的其他值保持未知。官方入口可由 https://docs.x.ai/developers/debugging 核对。

远程抓取不提供原始响应头或重定向链，不能声称这些地址已经在本机无重定向读取成功。
运行时仍禁止重定向、代理和保留地址，不为 403 增加用户认证；403 不证明需要 API key。

## Azure：公开区域矩阵

固定读取 `https://azure.status.microsoft/en-us/status`，真实 GET 200。
根地址会重定向，因此实现直接使用上述无末尾斜杠路径。
本轮完整 HTML 为 6,998,420 字节，另一次 PowerShell 获取耗时 22.369 秒。
这个耗时不是 rquest/no_proxy 的实测，不作延迟承诺。

解析范围：`region-status-table` 的 `data-zone-name`，服务行首单元格必须精确为
`Azure OpenAI Service`。去除 script、comment、SVG 后，对齐表头区域和 `status-cell`：

- 严格校验地区列数、唯一名称；不支持合并单元格，出现 colspan/rowspan 非 1 则拒绝。
- 观察名称保留 zone 和 region；不根据业务 URL 猜地区，不汇总为全模型状态。
- `Good` 对应官方“没有广泛故障”；`Information`、`Warning` 独立展示；
  `Critical` 表示严重中断；`Not available`/`N/A` 为“不适用”，不是正常也不是故障。
- 本轮中国区域矩阵均为 Not available；绝不以此声称中国区 Azure OpenAI 健康。
- 这不是用户订阅的私有 Azure Service Health，也不证明任意部署或模型可用。

只为此固定官方 HTML 提供 8 MiB 响应上限、30 秒请求超时、35 秒整体探测预算；
DNS 解析仍最多 3 秒，其他请求保持原先 1 MiB / 4 秒 / 25 秒限制。
在 `spawn_blocking` 中执行有限 HTML 解析，避免占用异步调度线程；最多 32 张表、
每表 2048 行、128 列、512 条观察，名称和标签也有独立限制。即使调用方取消，
已启动的后台解析可能继续完成，但输入与工作量有界，不会执行页面脚本或创建网络请求。

## 百炼 / Qwen：公开公告观察，而非伪造产品状态

读取中国站 `https://status.aliyun.com/api/status/listEventInProgress`。
依据官网加载的 `https://g.alicdn.com/aliyun-next/status/0.0.19/index.js`，
中国站使用无 International 后缀接口，不能替换成国际站状态。
当前接口真实返回 `success:true, code:200, data:[], total:0`；无须业务密钥或签名头。
官方当前公告视图只确证 `title` 字段，因此实现不会猜测非空公告中的影响产品和地区。

已另行核查杭州、北京产品目录及杭州产品状态，并读取真实历史公告/详情。
现有目录未找到百炼/DashScope 的精确产品记录，产品状态为空映射；历史详情还涉及海外地区，
因此“中国站入口”不等于“中国区域专属公告”。

实现行为：

- 同时校验 HTTP 结果和业务 `success/code`；HTTP 200 内业务失败不能变成空公告。
- 空公告显示“未返回进行中的公开公告（不代表服务正常）”。
- 非空时读取标题并展示最多 32 条；显示总公告数和“百炼覆盖未确认”，不猜故障严重程度。
- 总数与当前数组不完整时仍保持覆盖未知；未知结构拒绝解析。
- 所有观察 `model=null`，不会因为公告为空放行 restrict + unknown block 下的模型。
- 设置页面和结果页面均明确展示范围；公告标题作为普通文本渲染，不作为可信 HTML。

## 回归与原始响应回放

新增测试覆盖网站/API 隔离、xAI 未知状态、Azure 区域列错位和合并单元格、
脚本伪造表格、Not available 语义、Qwen 业务失败/空公告/有界标题，以及普通响应
限额不被 Azure 的 8 MiB 特例放宽。前端测试覆盖 20 个选项的自动填充与两类范围提示。

本轮原始公开响应保存在：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-remaining-status-20261023/`。
显式本地响应回放，不联网、不修改网络配置：

```powershell
$env:GATEWAY_STATUS_FIXTURES='C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-remaining-status-20261023'
rtk proxy cargo test --locked -j 4 --lib site_health::official::remaining_tests::captured_remaining_official_responses_replay -- --ignored --nocapture
```

原始响应回放与运行时真实联网验证分开报告，不把 Fake-IP/DNS 拦截绕过后当通过。
没有新增依赖，没有改动运行实例、凭据或数据库。

## 最终验证记录

- 最终源码 Rust 站点健康单元测试：17 passed，1 个本地样本回放在普通运行中 ignored。
- 显式原始响应回放：1 passed；Azure 80 条区域观察，xAI 4、You.com 3、Tavily 1、
  Qwen 1 条覆盖未知观察。完整 Azure HTML 已通过实际 Rust 解析，而不只是人工查看样本。
- 六组 Rust 站点健康集成合同：21 passed，联网 smoke 在普通运行中 ignored。
- 前端七个相关文件：30 passed；typecheck、build:web 通过。
- checker 自测 31 passed，ratchet 通过；本轮涉及源码均不超过 227 有效行，
  Azure 解析器为 148 行，新增剩余适配器测试为 187 行。
- 定向 rustfmt 和 `git diff --check` 通过。
- 显式运行 20 个适配器的联网 smoke：全部在 DNS 校验阶段返回
  `Site resolves to a prohibited network address`，未通过端到端联网验收。
  日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-official-status-remaining-smoke-20261023.log`。

没有调整系统 DNS、代理或保留地址限制来让测试变绿。公开接口取证、原始响应解析回放、
安全传输链路联网验收为三个独立结论。未生成交付包、替换现有 EXE、提交或推送，
也未进行完整桌面人工验收。
