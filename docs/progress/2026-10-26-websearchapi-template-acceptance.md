# WebSearchAPI 标准模板真实验收（2026-10-26）

## 结论与边界

WebSearchAPI 基础搜索已按 You、Exa、Tavily、Linkup 的同一新流程完成真实验收，登记
`flowVersion: 1` 蓝色完成标记。真实源码 UI 创建空凭据池、导入用户提供的 Key、运行该账号的
搜索连通测试，随后通过普通 Gateway query/q 搜索核对实际调用和精确凭据审计。
不是仅通过静态检查、mock 或 HTTP 200；必须取得有效标题和 HTTP(S) URL。

仅使用 `https://api.websearchapi.ai` 的 Search，不混同名称相近的其他域名。
本轮不扩展或验收独立 Web Scraper、内容提取、答案生成、多来源与全部搜索参数、
自动轮询或长期稳定性；模板没有已配置的官方站点健康适配器，保持关闭、未匹配、空 URL，
不添加假正常监控。此标记表示本次搜索调用流程验收，不表示上述能力已全部完成。

后端仅绑定 `127.0.0.1:4300`，源码前端为 `127.0.0.1:1435/ui/`。没有停止、接管或修改
用户的 4200 实例、旧桌面包或其凭据；原 `.ng`、`.ng-devtest-4300` 的 WebSearchAPI provider、
routes 和仓库 routes 哈希保持一致。Key 只进入受限 private 文件和隔离数据根，未进入源码或
公开证据。没有生成桌面 EXE 分发包、正式发布、提交或推送。

## 红测与最小修复

原模板地址、Bearer、`/ai-search` 路由和 `query` 字段配置已存在且正确，但新建策略仍继承
聊天题库与 `expectedAnswer: OK`，后端账号搜索测试返回 Unsupported。前后端定向红测均已
复现真实缺口；修复仅扩展现有搜索测试链，不重写 planner、认证或普通搜索接口。

- 前端默认 Selected + `websearchapi-search` + 无标准答案的“搜索连通”，不生成聊天 IQ 分数。
- 搜索 probe 增加 WebSearchAPI preset，映射后仍只接受精确 `websearchapi-search`。
- 复用 prepare/planner/headers/profiled-send，使用 Bearer、`POST /ai-search`、`query`、
  `maxResults: 3`、`includeContent: false`、`includeAnswer: false`，不发送本地 model。
  不启用额外提取/生成，不增加重试、跨账号或跨服务商 fallback。
- 结果路径为顶层 `organic`，使用非空 `title` 和 HTTP(S) URL。沿用 60 秒超时、1 MiB 响应
  上限；空/坏/超限/错形状/非法 URL 或含错误对象的 HTTP 200 不算成功。错误只给安全摘要，
  不回显上游错误正文、网页内容或凭据。
- 模板固定地址与三适配器保持只读，独立账号界面导入 Key；先创建显式空池，不生成隐式账号。

本轮联网核对 [官方 Search 合同](https://websearchapi.ai/docs/search-api) 和
[Quickstart](https://websearchapi.ai/docs/quick-start)。

## 真实 UI、请求与归属

UI 路径：添加服务商 → 标准模板 WebSearchAPI → 添加 → 添加账号 → 保存 → 测试账号 → 运行测试。
空池 revision 为 `r1-61b9cce6c217`，导入后为 `r2-56671f4cae0d`；回读 Key 与用户材料完全一致。
provider 为 `websearchapi`，credential 为 `websearchapi-cred-0`，名称为“WebSearchAPI 验收账号”。
没有通过 SQL 或直接管理 API 代替 UI 创建/导入。创建和导入使用上一版 API devtest，未执行
旧实现的付费搜索测试；重启为本轮修复版、重新登录后，才运行以下三次真实搜索。

| 验收项 | 实际结果 | 精确归属 |
| --- | --- | --- |
| UI 搜索连通 | Passed，3 条有效结果，7109 ms；0 失败、0 未执行 | websearchapi / websearchapi-cred-0 / websearchapi-search |
| POST /v1/search，query | HTTP 200，1 条有效 organic title/URL，7547 ms | 同一账号、同一模型 |
| POST /v1/search，q | HTTP 200，1 条有效 organic title/URL，7531 ms | 同一账号、同一模型 |

UI 测试 `checkedAt=2026-10-25T19:51:13.86773Z`，审计
`probe-74ff47ab-a485-4ee1-a8e6-fd5fb3d482f7`。`gradedCount=0`、`score=null`、
`expectedAnswer=null`、`correct=null`、`healthRecovery=applied`，只保存结果数量摘要。
额度观测仍为 unavailable，不把文档价格当作本次请求账单，不虚构 consumed 或 USD 费用。

普通 query/q 响应 request ID 分别为 `c75c2202-49c4-4ec1-b44c-e8b01097a108`、
`d544ce9f-8a5c-4f61-bd1e-0d82817119e7`；审计分别为
`gw-audit-24509e5d-0ed8-4a39-b613-1cc9fcd86acd`、`gw-audit-9354a1cc-8cab-4fef-b413-667eecfe8a78`。
三条审计均 completed、upstream 200、`search/search`、`realCredentialRef=websearchapi-cred-0`。
两次普通请求均 `routeTrace.routeAttemptCount=1`、`candidateCount=1`、
`attemptedProviderIds=[websearchapi]`、`selectedPipelineMode=direct_http`、
`selectedProtocolProfile=websearchapi`，没有其他账号兜底。UI 刷新显示 1 个可用账号与 3/3 成功。

## 定向验证与构建来源

- WebSearchAPI、Linkup、Tavily、Exa、You 五个 Rust 搜索合同各 1 项全部通过。
  新合同覆盖 16 次精确 loopback 出站，校验路径、Bearer、query/参数、移除 model、
  401/402/403/429/400/500、安全错误、空/坏/超限/错形状/错误标题/非法 URL、禁用账号、
  模型 alias、无评分、精确审计和健康恢复。合成凭据、随机回环端口、linshi 临时目录；
  默认 CI 不调用付费上游。
- 前端 8 文件、25 项、TypeScript、定向 rustfmt、31 项有效行 checker 测试、ratchet 和
  `git diff --check` 通过。新增测试最初使用了 Testing Library 不支持的 `exact` 选项，
  TypeScript 正确阻断；已改为精确正则，保留失败日志，重跑全通过，没有放宽类型检查。
- 官方 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4` exit 0；Cargo 构建报告 6m42s，
  保留既有 warning，来源前后指纹一致，产物哈希核对通过。不是正式 Release。
- 验收 EXE：`target/devtest-editions/api/gateway.exe`，SHA-256：
  `4dc4902c67f19d0c0ef562766b5d7e17526049d14e90e139513768491334f946`。
  构建源码指纹：`2638e66cad0f88f01b5066b5536c4f69a4a727d4462319680e8f53a5df0ec457`。
  蓝色登记与本文在真实成功后写入，不以旧凭据冒充之后的桌面分发包来源。

有效行：共享 probe 214→221，workflow 94→94，registry 46→51，完成列表测试 94→95；
新增 WebSearchAPI UI 合同 44、Rust 合同 238。preset 207 和搜索 planner 418 均未修改。
无新增超限、无超限历史文件增长、未申请例外。未执行全量 Cargo 回归、其他厂商付费测试
或正式发布安全审计。源码完成不代表用户正在运行的旧桌面 EXE 自动升级。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-websearchapi-adaptation-20261026/`。
包括红绿测试、构建凭据、真实请求摘要、审计、完整性、UI 截图与机器回执。
受限 private 内的导入文件、数据库、备份和完整响应均保留，不可作为公开交付内容。

## 收尾复核

- 蓝色登记及目录测试更新后，重新执行前端 8 文件 25 项、TypeScript、定向 rustfmt、
  有效行 checker 31 项、ratchet 和 diff-check，六项命令全部 exit 0。再次只读验收
  `acceptance-verified.json` 通过，没有重复调用付费上游。
- 再次回读仍为一个 provider、一个账号，revision `r2-56671f4cae0d`，Key 一致，
  自动测试关闭。蓝色模板预览中的防重名 `websearchapi-2` 草稿已取消，没有保存重复池。
  账号 UI 为 3/3、100%；无标准答案的搜索测试保持 IQ 为“—”，不伪造评分。
- 成功截图为证据目录的 `ui-live-success-final.jpg`；未评分明细为
  `ui-ungraded-result.jpg`。`ui-template-complete.jpg` 仅记录目录蓝色标记预览。
- SQLite integrity 为 `ok`，两份原数据中的 WebSearchAPI provider、routes 和仓库
  routes 哈希保持一致。源码与公开文本证据执行密钥检查，八个本轮源码/文档文件检查
  UTF-8 无 BOM；完整 Key、management token 和 relay token 不写入公开材料。
- 对本轮窄改动复核输入、认证、秘密、资源与并发边界：没有新增可控目标路径、后台循环、
  队列或进程 owner；复用既有认证和有界发送/取消链，最多 1 MiB、60 秒，错误不回显
  上游正文。未将局部代码审查冒充正式发布安全审计。
- `2026-10-25T19:58:58.1769815Z`（北京时间 2026-10-26 03:58:58）收尾完成：
  本轮 API drain 返回 202，API PID 62048、wrapper PID 31648、UI helper PID 63116
  全部退出，4300/1435 无监听，验收 tab 已关闭。4200 原 Gateway PID 2768 和局域网
  relay PID 13020 不变，`GET /healthz` 为 200；私有隔离数据和证据保留。
