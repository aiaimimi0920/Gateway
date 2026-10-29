# Jina 统一模板真实验收（2026-10-26）

## 结论与边界

沿 You、Exa、Tavily、Linkup、WebSearchAPI 的标准新流程，统一 Jina 模板已完成真实源码 UI
创建空池、独立导入用户 Key、同一账号的 Search 与 Reader 测试，以及普通 API 四种输入验收。
六次真实请求全部成功，精确属于 `jina / jina-cred-0`；成功后才登记 `flowVersion: 1` 蓝色标记。
不是仅通过静态检查、mock 或 HTTP 200。

新建目录仅保留 `jina`，不再展示 `jina-search`、`jina-reader` 两个入口。模型 ID
`jina-search`、`jina-fetch` 是同一服务商的操作标识，不是两份凭据，也不是聊天模型目录。
后端旧 preset、两条 Cargo 实现线与 manifest 保留兼容；本轮不删除用户运行实例的数据，
不升级数据库版本，不隐式迁移已有两池。

验收后端仅绑定 `127.0.0.1:4300`，源码 UI 为 `127.0.0.1:1435/ui/`，使用 linshi 内独立数据根。
没有修改、停止或接管用户 4200 程序、旧桌面分发包、其凭据或数据库。本轮仅构建 API devtest，
没有生成桌面分发包、正式发布、提交或推送。源码完成不代表旧桌面 EXE 自动升级。

本轮未验收 Embeddings、Reranker、OCR、EU 域名、全部参数、自动轮询或长期稳定性。
额度观测为 unavailable，不把文档单价当本次扣款，不虚构 consumed、USD 费用或 IQ 分数。

## 官方合同与最小修复

本轮联网核对 [Reader](https://jina.ai/reader/)、[Docs](https://docs.jina.ai/) 和
[OpenAPI](https://docs.jina.ai/openapi.json)，保存的公开合同版本为 `2026.10.13.1624`。

- 同一 Bearer Key；Search 为 `POST https://s.jina.ai/search`、JSON `q`；Reader 为
  `POST https://r.jina.ai/`、JSON 单个 `url`，Accept 为 application/json。
- 统一 preset/profile 为 `jina`，固定 base 为 `https://s.jina.ai`，双模型；新建显式空池，
  不生成隐式账号。路径、字段、品牌、卡片类型与 `jina-status` 归属均已接线，自动测试默认关闭。
- 复用 `search_api_compatible`、prepare、planner、headers 和 profiled-send/direct_http，
  不新增 adapter、持久化字段或旁路 transport，不把本地 model 发给上游。
- 只有精确官方 base 的 Fetch 切到固定 Reader host；自定义 base、loopback、伪域名与带路径
  base 不跨 host 改写。被读取的 URL 只进入请求体，绝不用作携带 Key 的 Gateway 请求地址。
- Reader 拒绝多 URL、同时传 url/urls、非 HTTP(S)、URL 内嵌凭据，不能静默截取首个目标。
- 统一模板/profile 仅在 search、reader 两条 feature 均启用时注册/放行；planner 再按 endpoint
  核对对应实现线。旧独立实现线继续保留，不将未知 profile 的默认放行当成 feature gate。
- 默认测试选择两个模型、一个公开 URL 用例；无标准答案。无参数且默认模型为 Reader 时使用
  公开 URL；明确提供的输入原样使用，不能悄悄替换用户目标。alias 先映射，再判断操作。
- Search 取 `/data` 数组，Reader 取 `/data` 对象；均要求 `code=200/status=20000`、有效
  title/HTTP(S) URL，Reader 另要求非空 content。空/坏/超限/错形状、错误对象或错误状态码的
  HTTP 200 不算通过。60 秒超时、1 MiB 上限、无自动重试，只保存安全数量或读取成功摘要。
- Search 测试限制 num=3、fallback=false；Reader 限制 timeout=30、tokenBudget=10000；
  不保留图片、不启用生成式图片描述。审计分别为 search/search 和 search/fetch，复用精确
  凭据健康恢复 owner，不回显上游错误正文、网页内容或凭据。

前端红测复现旧两入口与新唯一入口不符；后端红测复现 `Unified Jina preset is missing`。
窄审发现无参数 Reader 默认输入仍是搜索文字，新增合同再次复现
`Cannot build the search test request.`，随后在默认输入 owner 修复，不放松 URL 校验。

## 真实 UI、请求与归属

UI 路径：添加服务商 → 唯一标准模板 jina → 添加空池 → 添加账号 → 选择 Jina AI → 填入
账号 ID、名称、双模型与用户 Key → 保存 → 测试账号 → 运行测试。没有用 SQL 或管理 API
代替创建/导入。表单第一次因账号 ID 留空被正常阻止，补齐后保存；没有重复池或重复账号。

空池 revision `r1-0a67f287cc97`；导入后 `r2-6eebc14399c3`，一个 provider、一个 credential，
Key 回读完全一致，名称“Jina 验收账号”。默认双模型、无标准答案、自动测试关闭均回读确认。

| 验收项 | 实际结果 | 模型 |
| --- | --- | --- |
| UI Search | Passed，3 条有效结果，19937 ms | jina-search |
| UI Reader | Passed，有效网页，1201 ms | jina-fetch |
| POST /v1/search，query | HTTP 200，Learn Rust，有效 URL，客户端 3156 ms | jina-search |
| POST /v1/search，q | HTTP 200，Learn Rust，有效 URL，客户端 3032 ms | jina-search |
| POST /v1/fetch，url | HTTP 200，Example Domain、有效正文，审计 489 ms | jina-fetch |
| POST /v1/fetch，单元素 urls | HTTP 200，Example Domain、有效正文，客户端 547 ms | jina-fetch |

UI checkedAt 为 `2026-10-25T21:19:15.5689022Z`；两模型 callable，gradedCount=0、score=null，
两用例 expectedAnswer/correct=null、healthRecovery=applied。UI 刷新显示一个可用账号，
总计 6/6、Search 3/3、Reader 3/3，均为 100%。这不是 IQ 评分。

普通 Reader url 首次验收脚本错误要求正文包含单独返回的标题 `Example Domain`，造成
assertion failure；真实 HTTP、envelope、标题、URL、正文和审计均成功。保存的正文为有效
文档示例域说明，标题已独立位于 title，不能要求 content 重复标题。修正脚本后只读核对原始
响应和唯一成功审计，未重发该请求；已验证的 query/q 也未重发，仅发送尚未执行的 urls。
`relay-url.json` 明示 reconciliation、durationSource=gateway-audit、requestId=null，
没有补造丢失的响应头或客户端耗时。六条审计而不是额外重试，机器回执验证通过。

UI 审计：`probe-b34d3451-b9b8-4d66-90d3-31864c4a96d1`（Search）、
`probe-0024c0cf-cd51-4cd3-8beb-6e358ecb3d08`（Reader）。普通审计：
`gw-audit-ed969e71-ce40-4f6a-a8e1-ff6e48428fd1`、`gw-audit-846f0f70-7c9e-44bd-9047-d6251f516876`、
`gw-audit-9d548e10-c8f2-4c32-b149-f868006c4fcd`、`gw-audit-934db040-44bc-42fe-acea-40bd6a1d7dc5`。
六条均 completed、upstream 200、精确 jina-cred-0；普通四条均 candidateCount=1、
routeAttemptCount=1、attemptedProviderIds=[jina]、direct_http、profile=jina，无其他账号兜底。

## 定向验证、构建与安全边界

- Rust 六个相邻搜索合同、Jina planner 3 项、preset 50 项、registry 22 项、feature 37 项，
  共 118 项通过。Jina 合同验证 31 次回环发送/审计，含双响应形状、401/402/403/429/500、
  空/坏/超限/错 envelope/错路径/非法 URL/空正文、禁用、错误 alias、双模型无评分、两种
  无参数默认输入、精确审计及健康恢复；不调用真实上游。
- 前端 14 文件 59 项、TypeScript、定向 rustfmt、31 项有效行 checker 测试、ratchet、
  `git diff --check` 和 line manifest 校验通过。蓝色登记后，Jina 测试首次因按钮无空格拼接
  完成文案而失败；已把定位改为精确允许 jina 或其完成状态名称，不匹配旧两入口，保留失败
  日志并重跑。蓝色登记后的最终检查另见收尾记录。
- 官方 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 2` exit 0；因已观察到内存压力采用
  2 jobs，未停止其他项目编译。Cargo 报告 9m14s，完整 wrapper 590.09 秒，保留既有 warning。
  构建期间未写仓库，来源前后指纹一致，产物哈希/大小已核对；不是正式 Release。
- EXE：`target/devtest-editions/api/gateway.exe`，SHA-256：
  `b3e4e398d6de44bce7d6df6265273efebfb93204c332bc2a8f9c155448ec4027`。
  构建源码指纹：`ce41d2346b03b82c6f98e90fa713b5d19fd6bf89d7bf8801e52dac6c3c7ab113`。
  蓝色登记与本文在真实成功后写入，不拿这份 API 构建凭据冒充新桌面整包来源。
- 窄审输入、认证、凭据目标、响应上限、feature 与 cleanup 边界；没有新增队列、后台循环或
  重试 owner。独立只读交叉核验找出的默认输入缺口已用红绿测试修复。非正式发布安全审计。
- SQLite integrity=ok；原 `.ng`、`.ng-devtest-4300` 中 Jina provider、routes，以及仓库
  routes 哈希均保持一致。旧 4200 的两个配置没有被自动删除。

有效行：catalog 480→465，workflow 94→98，search preset 207→214，preset registry 171→178，
共享 probe 221→279，console probe 317→316，recording 140→147，generic planner 418→421，
profile inference 247→255，profiles 223→225，implementation_lines 114→129。
新 Jina planner、Rust/UI 合同均低于 500；无新增超限或历史超限文件增长，未申请例外。
未执行全量 Cargo 回归、其他厂商付费复测或正式发布流程。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-jina-adaptation-20261026/`。
包含红绿测试、构建凭据、请求摘要、审计、完整性和 UI 截图。private 内 Key、原始响应、
数据库与备份仅为本机验收材料，不得公开。`acceptance-verified.json` 为只读验收回执，
再次运行不会发送上游请求。

## 收尾复核

- 蓝色登记及按钮定位测试修正后，重新运行前端 14 文件 59 项、TypeScript、定向 rustfmt、
  31 项 checker、ratchet、diff-check，六项命令均 exit 0。完成标记 UI 已重新加载并确认；
  `jina-2` 仅为防重名预览草稿，已取消，回读仍为一个池和一个账号。
- `ui-live-success.png` 显示同一账号 Search 3/3、Reader 3/3；`ui-template-complete.png`
  记录唯一 jina 入口的蓝色标记。Key 仅留在 private 和隔离数据内，源码/公开文本秘密扫描及
  本轮源码、文档 UTF-8 无 BOM 检查通过。
- `2026-10-25T21:25:15.4942471Z`（北京时间 2026-10-26 05:25:15）收尾：drain 返回 202，
  隔离 API PID 27240、UI helper PID 4548 已退出，4300/1435 无监听，浏览器验收 tab 已关闭。
  原 4200 Gateway PID 2768、局域网 relay PID 13020 不变，healthz=200；数据与证据保留。
- 启动 shell 的附带 health 命令因外层 PowerShell 等待后台子进程而延迟到主动 drain 后才执行，
  因而报告 connection refused、exit 1；这不是启动或业务失败。运行期已独立验证 4300
  healthz=200，且六次真实请求均成功；不把该延迟命令写成通过，也没有为了消除它重启实例。
- `verify-acceptance.py --final` 只读核对六次真实调用、五组 Rust、六项本地检查、构建哈希、
  源码扫描和 cleanup 回执，不重复上游调用。没有全量回归或新的桌面分发包。
