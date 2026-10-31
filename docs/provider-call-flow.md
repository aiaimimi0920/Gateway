# 服务商三适配器调用框架

## 运行合同

三项是独立配置、串行协作，不是三选一，也不意味着向上游发送三次请求：

1. **调用适配器**确定目标数据格式。标准 API 沿现有 OpenAI、Anthropic、Gemini、
   Cohere 编码器构造请求；`auto` 沿 preset 和账号已有协议发现证据选择，不在推理中联网探测。
2. **指纹适配器**沿现有身份层应用请求体默认值、应用层请求头和已有的传输策略。
   不改变所选凭据、鉴权目标、路由、计费，不承诺任意 TLS/网络指纹可模拟。
3. **专用适配器**拥有产品专属的执行过程。`none` 是普通 API 发送；Web 等执行器可以
   将 canonical 请求展开为会话引导、挑战、发送、轮询或流式读取，指纹在其实际发送步骤由
   会话 owner 应用，不能把它简化成“再改一次 JSON”。响应和流沿已有 owner 反向返回。

逻辑执行顺序是请求规范化、解析三层策略、构造标准请求或专用调用过程、应用对应指纹、
发送和响应回流。指纹必须落在最终发送动作上，不能在多步骤 Web 过程展开之前只盖一次章。

入口 `protocol::call_flow::prepare` 返回请求局部的 `PreparedCallFlow`：有效 payload、
调用编码器、执行 owner、指纹归属。配置编译先解析一次；buffered、stream、JSON、binary、
native Codex/Claude 快捷入口在任何网络或会话动作前重新检查。已有编码器和专用执行器通过
这个兼容桥接接入，而不是本轮重写所有模板。没有修改 Gemini 执行器内部流程。

Redis 与 Hosted 的账号、凭据合并结果在原始模型策略仍可读时，经 `routing::call_flow`
准备有效 adapter，再计算允许的协议能力。模型 allow/block 结果为空时排除候选，不解释为
“不限协议”。请求级协议选择前再次同步候选元数据，发送资格检查前验证准备结果，避免
keepalive、会话和 FreeBuff 直达执行绕过校验；候选与 access-catalog 额度来源行成对过滤。
Redis 只新增读取 typed flow/指纹，不信任 raw `discovered_protocols`；协议发现证据仍须经过
原有地址、凭据绑定与模型证据校验，不能通过配置任意改发所选凭据。

ChatGPT official 的所有权以有效 OpenAI adapter 为前提，而不只依赖地址识别。其非流式、
流式及强制流聚合均复用统一 planner 后处理与 profiled send，包括 CPA 传输准入；响应解析、
流式输出、大小限制及 official/Codex 错误归因仍由原 owner 负责。

ChatGPT 的界面账号测试也复用 `execute_stream_with_wire`，按精确凭据发送，不重新选路或
回退到其他账号。它沿相同的三适配器准备、planner、指纹和 native/product dispatcher，
并要求收到成功完成事件及非空助手文本；超时 60 秒、探测层 SSE 上限 256 KiB。
专用执行器自身仍有独立输入上限（Basispoints 为 16 MiB），内部探测不等于外部 CLI wire 验收。

## 加法式配置

```yaml
providers:
  - id: example
    adapter: openai_compatible
    base_url: https://example.invalid
    explicit_credentials_only: true
    credentials: []
    call_flow:
      call_adapter: anthropic
      fingerprint_adapter: claude_cli
      special_adapter: none
```

- `call_adapter`：`auto`、`openai`、`anthropic`、`gemini`、`cohere`。
- `fingerprint_adapter`：`auto`、`codex_cli`、`claude_cli`、`workbuddy`、`cpa`、
  `cpa_claude_code`、`session`。API 身份能力仍以现有文本接口支持范围为准。
- `special_adapter`：`auto`（继承 preset 的执行 owner）、`none`、`basispoints`、
  `gemini_web`、`gemini_canvas`、`chatgpt_web`、`aistudio_web`。

缺少整个 `call_flow` 时不合成新策略、不额外序列化字段，完全沿旧配置路径。存在时，
三项策略为准；旧 `adapter/preset/request_client_profile` 保留为兼容投影和原生身份默认值。
账号级 `request_client_profile` 可覆盖指纹；账号级 Basispoints 开关可覆盖继承通道。
现有前端指纹写入函数同步新策略，避免修改后仍被旧值覆盖。不升级存储版本、不迁移数据库。
凭据池卡片也沿这套写入函数保存，显示时优先读取 flow 指纹，再兼容无 flow 的旧字段。
卡片读写支持合法的单层 `callFlow/requestClientProfile` alias，修改时保留现有键名，不生成重复
字段。同层重复 alias、非法 flow 类型和未知指纹在写入前拒绝，失败时不替换编辑草稿。
显式 `session` 或明确指定专用适配器时，客户端控件显示“站点会话”并锁定；写入函数和编辑
hook 同时保护，不能仅依赖控件 disabled 来阻止生成不支持的 API 指纹。

Hosted 在服务商与凭据 JSON 合并时保留来源：凭据自己的显式 flow 指纹优先于同层旧字段；
凭据只有旧 profile 时，将它同步到继承的 flow；凭据没有覆盖时，仍以服务商 flow 为准。
`call_flow/callFlow`、`request_client_profile/requestClientProfile` 可跨层混用，合并时统一键名；
同一层重复 alias、未知选项、非法类型仍拒绝。旧字段同步不会纠正原本非法的 flow，
也不把 null profile 写成 null 指纹，不合成原本不存在的 flow，不重写持久化记录。
Redis 的扁平 payload 已不含字段来源，继续以 flow 为准，不盲目将旧 profile 反向覆盖新策略。

新配置仍保留 preset：它还负责认证方式、固定地址、路径、产品参数和会话初始化，不等同于
三项适配器策略。专用适配器必须匹配其 preset/transport，不能把普通 API Key 自动当 Web 会话。
Basispoints 继续校验原生 Codex 地址、编译 feature 和凭据来源。

## 本阶段支持范围

标准 API 可以组合已有调用格式与 API 指纹。专用执行器目前接收 `call_adapter=auto` 的
canonical 输入，使用 `fingerprint_adapter=auto/session` 的执行器内指纹；不支持的显式 API
指纹或固定调用格式会报 `Invalid call_flow`，不是悄悄忽略。未知枚举和拼错的策略字段拒绝解析。
后续可以逐模板扩展允许组合，只有真实适配后才开放编辑项。

固定调用格式会约束请求级协议发现选择，调试强制协议也不能越过这个约束。发现证据仍完整
保留，包括模型负证据和认证目标绑定；Gemini/Cohere 的 discovery-only 证据不会因此被提升为
已经支持自动桥接。当前 auto 是已有发现能力的组合框架，不是所有产品的万能协议探测器。

新建面板保存真实 `call_flow`，同时维护旧 transport 投影；Web 和 Basispoints 的显示、
保存指纹归属一致，不声称 API 指纹控制浏览器会话。`TEMPLATE_ADAPTATION_RECEIPTS` 当前仅登记
已完成真实验收的 `nvidia-openai`、`openrouter-openai`、`you`、`exa`、`chatgpt`、`tavily`、`linkup`
与 `websearchapi`、`jina`、`poe-openai`、`groq-openai`、`cohere-chat`、`openai`、`anthropic`；框架单测、loopback 和界面测试不会批量标蓝。
其中 `openai`（显示名 `openai-api`）按用户明确批准的余额受限标准完成：官方目录导入成功，
精确账号 UI、非流式及流式请求取得上游额度拒绝，不代表生成或 SSE 输出成功。见
[OpenAI 验收](progress/2026-10-27-openai-template-acceptance.md)。
`anthropic`（显示名 `anthropic-api`）也按用户批准的余额受限标准完成：正常 UI 建空池、导入官方目录，
精确账号 UI 及原生 Messages 两种请求模式取得 HTTP 400 / InsufficientQuota；未证明成功生成。
其 UI 测试复用生产 dispatcher 和原生 SSE parser，限制 60 秒与 256 KiB，必须有完成事件和非空文本才 Passed。
见 [Anthropic 验收](progress/2026-10-27-anthropic-template-acceptance.md)。

You 使用 `search_api_compatible`、标准 API 指纹和 `direct_http`，不是 Chat/Responses
执行器。其显式账号测试经同一 prepare/planner/profiled-send 链发送低数量搜索，要求
`results.web` 中存在有效标题与 HTTP(S) URL；60 秒超时、1 MiB 响应上限、不自动重试，
只保存安全结果数量摘要。搜索审计使用 `search/search`，复用精确凭据的健康恢复 owner；
新模板默认无标准答案，不生成聊天 IQ 分数。首轮 403 经复核来自占位符材料；
用户提供真实密钥后，经 UI 重新导入独立凭据、显式搜索及普通 query/q 搜索均已通过。
旧凭据换 key 后仍继承 protected 禁用状态的问题未修复，详见
[You 重新导入验收](progress/2026-10-25-you-key-reimport-acceptance.md)。

Exa 与 You 共用搜索测试的 prepare/planner/profiled-send、超时、响应上限及审计链；
Exa 使用 `x-api-key`、`numResults` 和顶层 `results`，不混用 You 的 `count/results.web`。
Exa 默认测试仅选择 `exa-search`，映射后的 `exa-fetch` 也会被搜索测试拒绝；普通抓取继续
使用 `/v1/fetch` → `/contents`。真实 UI 创建、导入、搜索及普通搜索/抓取均已通过，见
[Exa 验收](progress/2026-10-25-exa-template-acceptance.md)。不提供聊天能力或伪造搜索 IQ 评分。

Tavily 复用同一搜索测试链，使用 Bearer、`max_results` 和顶层 `results`；默认仅测试
`tavily-search`，固定 basic、关闭自动参数和答案/原始网页/图片返回，不发送本地路由 model。
432/433 分别给出计划额度和按量额度的安全错误，不回显上游正文。真实 UI 创建、导入、
无评分账号测试和普通 query/q 搜索均已通过，见
[Tavily 验收](progress/2026-10-26-tavily-template-acceptance.md)。不扩展为 Chat、Extract 或 Research。

Linkup 复用同一搜索测试链，使用 Bearer、`POST /v1/search`、`q`、`maxResults`、
`depth: standard`、`outputType: searchResults`，关闭图片；有效结果标题字段为 `name`，
不是其他搜索商的 `title`。默认仅测试 `linkup-search`，无标准答案与 IQ 分数，映射后也拒绝
Fetch/Research/Balance 操作；普通 API 继续透传各 endpoint 的显式参数。真实 UI 创建、导入、
账号搜索测试及普通 query/q 搜索、Fetch、Balance 均通过，见
[Linkup 验收](progress/2026-10-26-linkup-template-acceptance.md)。Research 保留原有路由但未实测。

WebSearchAPI 复用同一搜索测试链，使用 Bearer、`POST /ai-search`、`query`、`maxResults`，
明确关闭 `includeContent` 和 `includeAnswer`；结果按顶层 `organic` 中的 title/URL 校验。
默认仅测试 `websearchapi-search`，不生成聊天分数；真实 UI 建池、导入、账号测试和普通
query/q 搜索均通过，见 [WebSearchAPI 验收](progress/2026-10-26-websearchapi-template-acceptance.md)。
不扩展成 Web Scraper 或 Chat，也不虚构未配置的官方站点健康源。

Jina 统一为一个 `jina` 模板、一个凭据池和同一 Key 下的 `jina-search`、`jina-fetch` 两种操作。
复用搜索 API 的 prepare/planner/profiled-send；Search 使用 `https://s.jina.ai/search`，Reader
仅对精确官方 base 切到固定 `https://r.jina.ai/`，网页目标留在 JSON 中，不是携带凭据的出站 host。
默认双模型测试使用公开 URL，无标准答案或 IQ 分数；Reader 额外要求非空正文。统一 profile
要求两条实现线均编入，请求级另按 endpoint 检查 feature。旧两项不再出现在新建目录，后端旧
preset/实现线保留兼容，不自动改写存量数据库。真实 UI 建池、导入、双能力测试及普通搜索
query/q、读取 url/单元素 urls 均已通过，见 [Jina 验收](progress/2026-10-26-jina-template-acceptance.md)。
不扩展为 Embeddings、Reranker、OCR、全部参数或长期稳定性证明。

ChatGPT 标准模板默认原生 Codex 通道已完成 UI 创建、现有 access token 导入、显式账号测试，
以及普通非流式和流式 Responses 验收；三次均为同一凭据的真实 `gpt-5.6-luna` 回复。
此次没有验收重新 OAuth 登录、刷新生命周期或可选 Basispoints 公网通道，见
[ChatGPT 验收](progress/2026-10-25-chatgpt-template-acceptance.md)。

Poe 标准模板使用固定 `https://api.poe.com/v1` 与 Bearer API Key，正常导入只读公开
模型目录并同步账号池、服务商池与路由，不把目录发现当生成验证。自动测试关闭，默认
手动测试仅选择本次公开零价且真实成功的 `gemma-4-31b`。已通过正常 UI 建池、导入、
单账号测试及 Chat/Responses 两种模式验收；首轮付费模型的订阅拒绝保留，不推断所有
目录模型都有调用资格，见 [Poe 验收](progress/2026-10-26-poe-template-acceptance.md)。

Cohere 使用固定官方 `GET /v1/models?endpoint=chat&page_size=1000`、Bearer 认证与分页目录，
正常双栏导入同步账号池、服务商池及路由，不把目录当成生成或权限证明。生成沿原生 `/v2/chat`
和独立 Cohere SSE owner，支持连字符事件、工具增量、真实完成状态及实际 `usage.tokens`。
强制流聚合和普通 SSE 均由传输层设置 `Accept: text/event-stream`，不受入站 JSON 偏好影响；
Responses 沿 canonical 桥接到 Cohere Chat，不宣称 Cohere 提供原生 Responses 接口。
账号 UI 测试复用同一 prepare/planner/profiled-send dispatcher，锁定精确凭据，不选路或回退；
60 秒、256 KiB，仅完整且非空回复通过，无标准答案时不生成 IQ。真实 UI 建空池、导入 17 项目录、
一个模型的账号测试及四模式普通调用均已通过，见
[Cohere 验收](progress/2026-10-27-cohere-template-acceptance.md)。其余目录模型、真实工具调用及长期稳定性未验收。

## 定向验证与运行边界

```powershell
rtk proxy cargo test --locked --lib call_flow --jobs 4 --no-default-features --features runtime-defaults
rtk proxy cargo test --locked --lib routing::config --jobs 4 --no-default-features --features runtime-defaults
```

loopback 只使用随机回环端口和合成凭据，验证实际路径、请求体、身份头和凭据保持；
不使用 4200/4300，不读取 `.ng`。逐模板实测显式使用 4300 与独立数据目录；手测构建
本身默认 4200，覆盖与隔离边界遵循 `docs/devtest-build.md`。
框架测试不启动真实模板、浏览器账号或付费上游；另行授权的逐模板实测见对应验收记录。
新增配置不提供向任意第三方服务器自动转移凭据的能力。
