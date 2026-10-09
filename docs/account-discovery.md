# 自定义账号的模型与协议识别

## 使用方式

在“自定义 API 服务商”中填写服务地址和 API Key，无需手填模型列表或补写 `/v1`。
现有通用凭据池新增账号、替换 API Key 时也会执行识别。
从账号卡牌右上角进入编辑弹窗，底部“刷新协议”重新识别；按协议展开实际验证成功的模型。
刷新结果暂存在该次编辑中，点击“保存”后沿用配置自动保存链路提交；取消不提交。
API Key 可直接编辑，眼睛按钮按需读取当前账号的有效密钥（含 Provider 继承），仅查看不会替换密钥。
内建 OAuth、浏览器逆向等专用适配器继续使用各自的接入流程。

识别成功后，模型目录、各协议实际 API base、验证样本和时间写入账号的
`discovery` 字段，仍经现有带修订号的配置事务自动保存。
推理请求只使用编译后的账号配置，不重新访问模型目录、不逐次试探协议。
识别失败不提交任何配置；提交冲突遵循原有配置修订冲突处理。

## 当前发现范围

- 接受 HTTP(S) 根地址或带路径前缀的地址；已含 `/v1` 时不会重复拼接。
- 在标准路径之外，OpenAI 协议尝试 `/openai`，Messages 尝试 `/claude` 和 `/anthropic`，Gemini 尝试 `/gemini`；与各自版本及无版本路径组合，Gemini 包括 `v1beta`、`v1`。不扫描任意目录。
- 输入已含协议别名前缀时保持该部署范围，不重复添加别名或探测同级部署；全部候选去重、同源，每协议最多八个基础地址、三个模型样本，最多 24 条尝试记录。
- 合并 OpenAI / Anthropic 模型目录、Gemini `models[]`、Ollama `/api/tags`、Cohere `models[]` 和 Bedrock `modelSummaries[]`，分页最多四页，总目录最多 2048 项。
- 标准目录成功不会抑制别名目录的发现；模型目录和生成端点独立识别，允许共享目录与不同协议前缀组合。目录发现整体预算 60 秒，达到预算后使用已取得的目录，不把截断结果宣称为完整服务商清单。
- 分别探测十项：Chat Completions、Responses、Anthropic Messages、Gemini GenerateContent、Gemini Interactions、Ollama Chat、Ollama Generate、Cohere Chat v2、Bedrock Converse、旧版 Completions。
- 每个协议在同源候选地址上最多试三个目录模型，找到生成成功样本后记录该协议。并发最多四个协议，全部完成后返回；任何一个成功都不会提前结束其他协议识别。
- `probes` 记录所有协议的状态及尝试端点、模型、HTTP 状态，不保存上游错误正文或密钥。HTTP 200 但响应不符合该协议生成结果形状，不能算成功。
- 401/403 记录认证问题，429 记录限流，超时记录超时；未确认不等于不支持。普通临时失败不写入模型永久负证据。
- `protocols` 保存每个已确认协议的地址、`verified_models` 和同地址上此前失败的 `failed_models`。
- `models` 是目录可见模型，不是逐模型验证成功清单。旧的 `protocol/api_base/verified_models` 字段保留为首项投影，兼容旧读取器。

这不是逐模型付费验收，也不证明目录中所有模型均有额度、支持流式或拥有相同工具能力。
生成探测可能产生少量上游费用。本轮探测非流式生成，不证明 SSE、NDJSON、WebSocket、实时语音或图像能力。
不猜测未提供模型目录的服务，不承诺识别任意私有路径或每个目录模型的全部协议。
Bedrock 在这里只探测 API-key Bearer 接入，不能从单个 Key 推导 AWS SigV4 所需的区域及成对凭据；认证失败不代表没有 Converse。

### 发现与转发分离

`protocols` 是生成验证成功的协议清单；`probes[].routing_ready` 表示当前通用自动路由是否接通该协议。
现阶段通用自动路由仍只启用 Chat、Responses、Messages；其他七项完成端点发现，不据此假称已有完整桥接。
现有 Gemini/Cohere/Bedrock 专用适配器与自动发现接线是不同的验证范围，尤其现有执行器可能强制使用未被本次非流式探测验证的流式接口。
只有新发现协议而没有可自动路由协议的账号可以保存发现结果，但不产生可执行的通用候选；不得把它冒充 OpenAI 协议调用。

客户端请求优先选择已发现的同协议，避免不必要的 Canonical 转换；对应模型有明确失败样本时排除该协议。
没有同协议时，优先选择模型已验证的可转换协议，然后使用账号级发现证据。
同协议请求保留原始 JSON 字段和工具，仅替换映射后的模型及流式标志；JSON 响应保留未知字段，SSE 数据不重新编码。
这里的原生转发不是绕过 Gateway：仍执行鉴权、配额、重试、审计和用量采集，也不承诺透传全部 HTTP 响应头或 JSON 字节排版。
跨协议仍走已有转换链路；包含 `previous_response_id`、`conversation`、后台任务或非 function 工具等不能保真转换的请求不选跨协议候选。
这些检查不是任意私有扩展的完整转换保证。客户端 Completions 仍使用已有文本转换，发现原生 Completions 不自动启用未验证的双向桥接。
模型别名和模型映射以实际上游模型名匹配发现目录，不通过旧兜底逻辑绕过目录限制。
没有 `discovery` 的老账号保持旧配置行为；只有单协议 `discovery` 的老账号自动作为单项能力读取。
显示名称、分类和新建默认 ID 中性化为自定义 API 服务商，不重命名已有账号或池 ID。

## 生命周期与安全

管理端 `POST /v1/internal/gateway/console/account-discovery` 接受
`{baseUrl, apiKey}` 或 `{credentialId}`，返回 `{discovery, revision}`，自身不写配置。
必须同时具备有效管理会话与敏感信息访问授权。
账号编辑的 `POST /v1/internal/gateway/console/credential-api-key` 同样要求管理会话和敏感信息授权，
接收 `providerId`、`credentialId`、`expectedRevision`，只返回匹配账号的有效 `apiKey`，响应 `no-store`。
账号、配置修订或登录授权变化会丢弃迟到的读取结果；明文不写入浏览器持久存储。
使用存量账号时不允许同时覆盖地址或密钥，避免将存量密钥发送到另一个地址。

探测禁用重定向，目录请求超时 10 秒、生成请求超时 100 秒、每协议预算 200 秒、整体预算 720 秒；管理请求并发上限 2、每次最多四个并行协议，
响应体上限 1 MiB、目录上限 2048 项。上游原始错误体和 API Key 不进入错误响应。
地址与密钥的绑定摘要用于发现陈旧配置，不是服务商能力的密码学证明。
更换地址或密钥需要重新识别；刷新失败保留旧结果。

对话框取消、切换会话或配置版本变化会丢弃迟到结果。刷新期间锁定相关编辑和自动保存，
识别结束后使用原有自动保存链路提交，不另建配置或密钥存储。

## 回归入口

- Rust `provider_discovery` 单元测试：根地址、协议回退、绑定与响应形状。
- `tests/account_discovery_contract.rs`：权限、保存/重载、三种 HTTP 客户端协议转发、
  无热路径重复发现、刷新失败不覆盖、存量密钥地址保护和文本协议选择。
- `tests/account_protocol_routing_contract.rs`：全部协议、不同 API base、持久化后原生 JSON/SSE、模型负证据和有损转换拒绝。
- `tests/provider_discovery_inventory_contract.rs`：十协议、原生模型目录、伪成功拒绝、发现与路由隔离；默认忽略的 `live_hy_inventory` 仅在明确授权时探测黑与白，密钥来自环境变量且不写入证据。
- `tests/provider_discovery_alias_contract.rs`：根目录与别名目录合并、Gemini 版本后缀、Claude 别名、实际地址保存/重载、显式前缀不重复拼接。
- 前端 `accountDiscovery.test.tsx`、`useAccountDiscovery.test.tsx`：新增、取消、错误、
  配置修订隔离和刷新；`ProviderAccountCard.test.tsx` 覆盖刷新按钮。
