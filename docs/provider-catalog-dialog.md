# 添加服务商：界面与逐模板适配

本轮先落实已确认的添加面板、配置保存与兼容边界，再逐模板验证新的调用流程。
界面已存在不表示模板已通过真实调用验收；未验收模板不显示完成色。

## 界面与状态归属

- 左侧“标准模板 / 自定义模板”为并列互斥按钮，列表只显示 Provider ID。
  切换分类仅过滤列表，不重置右侧草稿或当前页签；点选模板才创建该模板的新草稿。
- 顶部只保留 Provider ID、Base URL。标准模板的地址、调用流程技术项和站点健康
  适配器只读；Azure OpenAI 的资源地址是必要例外。自定义模板可以配置技术项。
- 五个页签为调用流程、模型映射、自动测试、号池流程、站点健康。
  弹窗外框、操作栏固定，长内容在内部滚动；只有右下角“添加”，关闭使用右上角叉或 Escape。
- 新建不输入首个账号、API Key、显示名称或 vendor；旧模板元数据保留给卡片与既有运行时。
  创建后再使用现有“添加账号”流程获取凭据和发现结果。

## 三类适配器的当前边界

三个维度通过独立 `call_flow` 接入运行框架，仍复用现有执行器，不宣称任意组合都已逐模板验收。
串行合同、兼容规则与验证入口见 [三适配器调用框架](provider-call-flow.md)。

| 维度 | 本轮保存链 | 后续验收边界 |
| --- | --- | --- |
| 调用适配器 | 保存 call_adapter，运行时与发现链共同解析；旧 adapter/profile 保留兼容投影 | auto 复用已有发现证据；固定格式不能被发现链或调试协议静默改写 |
| 指纹适配器 | 保存 fingerprint_adapter；标准 API 使用已有身份层 | Web/Basispoints 保存 session；Antigravity 使用独立 antigravity；不支持的指纹组合明确拒绝 |
| 专用适配器 | 保存 special_adapter，并绑定已有产品 preset/执行 owner | 保留多步骤会话、响应与取消归属；逐模板上游验收仍需后续完成 |

自动测试使用现有 `test_policy` 与测试集校验，只保存策略；打开面板不调用上游。
号池驱动来自后端真实驱动列表，并按 Provider ID 精确白名单匹配；无匹配驱动时，
不启用补号/清理。存储、归档复用 local/WebDAV/S3 合同，不在创建时执行搬迁或删除。
站点健康按 preset 对应的具体产品匹配，不把同品牌的某个健康源套给所有 API/Web 产品。
已匹配官方适配器的标准模板默认开启站点健康；自定义模板默认开启公开接口识别。
默认每 300 秒刷新、900 秒有效，仅提示、未知放行，不携带业务凭据，也不启用账号自动测试。
未匹配健康来源的标准模板保持关闭，不使用搜索/推理请求代替健康监测；用户仍可关闭支持的监控。
模板默认值只影响新建草稿，不会自动改写已有服务商；已保存服务商需经正常配置事务单独开启。

## 仅创建空服务商的兼容合同

创建链 `addCatalogProvider` 在现有路由文档中追加服务商，保存
`explicit_credentials_only: true` 与 `credentials: []`，不生成 discovery_job 或账号。
支持模型和有目标的映射可生成/合并模型路由，但没有账号时仍不能获得可调用候选。

`explicit_credentials_only` 是加法式字段，默认 false，false 不额外写回旧配置。
旧配置无显式凭据时仍保留原 provider-default 账号语义。新空池没有默认账号、
默认权益组成员、路由候选、探测对象或测试目标；添加首个凭据后正常使用，删空后仍为空池。
不升级 storage version，不改写已有数据库或凭据。

认证信息走现有 `secretPatches` 和敏感信息授权链，不进入普通路由 JSON。
创建时按 Provider ID 和目标身份绑定，提交时重新解析数组位置；目标改变则拒绝重用，
owner 删除则丢弃。WebDAV 密码必须同时提供用户名。

## Web 服务商的专用账号导入

“添加账号”按已保存的 preset 选择专用控件，不依赖可改的 Provider ID：

| 模板 | 专用导入方式 |
| --- | --- |
| `chatgpt-web` | 保留既有网页登录／网页会话 JSON 控件 |
| `gemini-web` | 账号名称（可选）＋ 网页登录／专属网页会话 JSON |
| `gemini-canvas` | 账号名称（可选）＋ 网页登录／专属网页会话 JSON |
| `aistudio-web` | 账号名称（可选）＋ 网页登录／专属网页会话 JSON |
| `flow-web` | 账号名称（可选）＋ Google Flow 网页登录／专属网页会话 JSON；生成未完成，已暂停 |

上述 Google Web 模板新建账号不再显示通用 API Key 表单；编辑已有账号仍保留模型选择和已存凭据。
Canvas 历史 preset 的专用导入入口保持兼容。账号库“手动添加”和全局“添加账号”
复用同一认证会话及 revision/CAS 保存链；导入期间不启用账号自动测试。

AI Studio 本地 Windows 流程先打开没有自动化／远程调试连接的普通专用浏览器，用户
正常登录后关闭该窗口，再短暂重开以导入必要 scoped Cookie。只有 Gateway 正常对象
存储 owner 保存 Cookie；管理响应仅含对象引用、账号名称和支持范围内的模型目录。
此入口不导出完整浏览器状态，不发送生成请求；当前未接远程宿主／容器登录入口。
本轮新增管理 UI 链已做合成回归，尚未通过新入口重新执行真实 Google 登录验收。

Google Web JSON 支持粘贴或选择文件，再显式导入；要求敏感信息授权、当前修订及
目标模板绑定。该分支不启动浏览器、不联网，不把材料解析当账号可用验证。Canvas／
AI Studio／Flow 只返回 Cookie 对象引用；Gemini Web 沿已有 secret patches 保存，不进入
普通配置。格式和边界见 [网页会话 JSON 导入](google-web-json-import.md)。

## NVIDIA 目录与调用证据

NVIDIA 新账号导入在保存前读取官方 `GET https://integrate.api.nvidia.com/v1/models`；
已有账号可在编辑面板使用“刷新模型目录”，结果先进入草稿，再走普通 revision/CAS 保存。
目录接口要求管理会话与敏感信息授权，目的地址固定、禁止重定向，不执行生成或通用协议探测。
目录不可用、响应无效、账号或配置 revision 已变化时，保留上次已保存配置，不回退到模板默认模型冒充完整目录。

账号可选 `model_catalog` 保存 `sourceUrl`、`checkedAt`、`models`，不包含 Key 或生成验证结论。
刷新同步账号 `supported_models`、服务商模型并集和精确模型路由；保留目录外手工模型、其他账号、
其他服务商、既有路由优先级和禁用策略；不启用账号自动测试，也不改写模型健康状态。
首次刷新时，目录中的既有声明转为目录管理；后续消失项仅在没有其他账号或目录外声明需要时移出池及路由。
旧配置没有此字段仍可读取，不升级本地数据版本。新字段需要配套新版本，不能向旧 EXE 提交后假定其已保留。

卡片统一称“模型池”，分别展示“目录已发现”“有成功记录”“尚无成功”“未验证”。
成功记录只来自该服务商或精确归属账号的保留调用审计；目录公开可见不是账号权限证明，
历史成功也不保证当前权限或实时可用性。不为目录中的全部模型制造 `verified_models`，不批量调用模型。

## 蓝色完成标记

唯一登记入口为 `providerTemplateAdaptation.ts` 的 `TEMPLATE_ADAPTATION_RECEIPTS`。
原始登记包含 `nvidia-openai`、`openrouter-openai`、`you`、`exa` 与 `chatgpt`，分别见
[Nvidia 实测记录](progress/2026-10-25-nvidia-template-acceptance.md) 和
[OpenRouter 实测记录](progress/2026-10-25-openrouter-template-acceptance.md)及
[You 重新导入实测记录](progress/2026-10-25-you-key-reimport-acceptance.md)及
[Exa 实测记录](progress/2026-10-25-exa-template-acceptance.md)及
[ChatGPT 原生通道实测记录](progress/2026-10-25-chatgpt-template-acceptance.md)。
历史 canary、模型列表成功、界面测试成功都不能自动写入。

完整当前名单以该登记表为准，避免文档摘录漏项。2026-11-01 补齐此前漏记的
`gemini-canvas` 和 `aistudio-web`，分别依据
[Canvas HTTP-only 实测](progress/2026-10-31-gemini-canvas-http-only-r32.md)和
[AI Studio 公共 API 实测](progress/2026-11-01-aistudio-http-public-api-r36.md)。
`gemini-web` 已有登记，保持蓝色。`chatgpt-web` 于 2026-11-01 使用当前独立手测实例的
指定账号与 `gpt-5-6` 完成生产 HTTP 提示词测试，返回 `OK`，现已登记蓝色；见
[本轮真实验收](progress/2026-11-01-chatgpt-web-current-acceptance-r39.md)。此前的 403
历史保留，不把当前成功扩张为所有账号、模型和模态均可用。
蓝色表示登记支持范围的验收，不扩张为所有模型／模态、长期免登录或新导入入口的真实验收。

`flow-web` 保持 `pending`，不登记完成凭据、不标蓝；用户于 2026-11-01 暂停其生成开发。
目录状态为“未完成（已暂停）”，仍可从标准模板创建独立服务商并使用其专用账号导入控件。
创建服务商不生成账号、生成模型目录或模型路由；导入也不执行图片／视频生成。
具体边界见 [Flow Web](flow-web.md)。

`antigravity-oauth` 是独立 OAuth 模板，不属于 Google Web Cookie 导入 family。
账号对话框提供官方 Google OAuth、回调完成和显式保存入口；模型及项目从 Cloud Code 读取。
调用与保活都是 HTTP，不依赖运行中的浏览器；专用指纹由 Antigravity executor 管理。
当前不以本地合同测试替代真实上游验收，模板不标蓝。见 [Antigravity OAuth](antigravity-oauth.md)。

You 的空池创建、凭据导入和搜索专用测试经真实 UI 操作；首轮占位符认证导致 403，
用户提供真实密钥后重新导入独立账号，UI 搜索及普通 query/q 搜索均返回有效结果，现已登记。
旧账号换 key 后继承 protected 健康记录的恢复问题仍未修复，不以本轮标蓝掩盖该边界。

Exa 保留 `exa-search`、`exa-fetch` 两个普通路由；默认测试策略只选择 `exa-search` 的
无标准答案搜索用例，不把 Contents 模型拿去做搜索测试。UI 创建空池、导入真实密钥、
手动搜索测试，以及普通搜索和正文抓取均在独立 4300 环境完成。

每个模板完成新调用流程的定向开发和真实验收后，记录该模板的 `templateId`、
证据文档路径 `evidencePath`、验收时间 `verifiedAt` 与 `flowVersion: 1`，再加入登记。
列表使用现有 `--nt-info` 技术信息蓝，并提供 title 和可访问状态文字，不只依赖颜色。
验收应明确三类适配器各自生效、官方锁定项、账号加入/删空以及已支持的调用行为；
真实上游测试必须在隔离的 4300 环境中明确授权执行，不能以 mock 成功代替。
