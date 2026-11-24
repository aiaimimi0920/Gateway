# SenseNova Token Plan Key 合同

本地实现使用操作者在 SenseNova 控制台明确创建的 Token Plan API Key。控制台 OAuth、积分池读取和小浣熊账号是独立范围；不读取浏览器、桌面客户端或社区插件的登录材料。

## 官方依据

2026-11-10 匿名联网复核 [官方 API 文档](https://platform.sensenova.cn/docs)、[Token Plan 页面](https://www.sensenova.cn/token-plan) 与 [API Keys](https://platform.sensenova.cn/console/keys)。文档是客户端渲染页面，实际读取其公开引用的 `2582-7ac5809c83d50574.js` 中 `82285` 模块的 Markdown 字符串，未执行 JavaScript。原文快照保存在 `linshi/gateway-sensenova-20261110/contracts/official-api.md`，含来源、UTC 和 SHA-256。

- 固定 Base 为 `https://token.sensenova.cn/v1`，使用 `Authorization: Bearer KEY`。同一 Key 支持 Chat、Messages、Responses；各自使用独立的原生端点。
- `GET /models` 返回 `data[]`，包含 `id`、`input_modalities`、`output_modalities` 等。目录不是订阅权益保证，不能从可见模型推断实际推理权限。官方当前模型名已不同于固定社区快照，因此不内置旧静态列表。
- Chat 支持 SSE 与函数工具；工具结果使用 `role:tool` 和 `tool_call_id`。官方 GLM 小节明确 `data: [DONE]`，结束原因包括 `stop`、`length`、`tool_calls`、`content_filter`。文档没有逐字 Chat SSE 返回样本，不能声称所有模型的终帧布局均已真实验证。
- 用量的 completion/output tokens 已包括 reasoning tokens，不能重复相加。积分池、现金余额和 Token 用量不是同一个数值。
- Responses 具有独立模型支持集和无状态限制。2026-11-22 的公开文档复核及原生接线见 [Messages](../progress/2026-11-22-sensenova-native-messages.md) 与 [Responses](../progress/2026-11-22-sensenova-native-responses.md)。
- 文档错误例为 `{error:{type,code,message}}`；匿名 `/models` 实际返回 HTTP 401、`{"error":{"code":16,"message":"Authorization Not Found"}}`，不能强制所有错误都有字符串 type/code。

固定社区 `magpie-community/plugins@93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/sensenova/README.md` 明确声明 `pool-usage`、浏览器截获授权码、自动创建 Key 使用未公开控制台接口。官方当前资料未提供独立第三方 OAuth client 注册合同；本实现不借 `client_id=nova` 或自动管理账号 Key。

### 2026-11-22：区分两套公开 API 合同

- 本仓库当前目标仍是 `https://token.sensenova.cn/v1`。当天匿名、不跟随重定向地复查 `GET /v1/models`：返回 HTTP 401、错误码 16；没有 `Location`。旧官方文档页面当前引用的 `210-35a7025cd2eaebe6.js` 哈希仍与下文一致；未发现 `video_url`、另一域名或公开的控制台授权合约。这里只描述复查范围，不断言商汤没有其他私有或账号内资料。
- 另有 [商汤 TokenPlan 官方文档](https://token.sensetime.com/docs) 及其 [Chat 合同](https://token.sensetime.com/docs/api-reference/endpoint/llm/chat)，写明 `https://token.sensetime.com/api/v1`、`stk-` Key；Chat 内容样例列文本和 `image_url`，没有给出可用于本产品的 Chat 视频对象。其 [模型目录](https://token.sensetime.com/docs/api-reference/endpoint/models/list) 使用 `/api/v1/models`，按 `type=llm|img` 区分模型；该目录匿名实测返回 HTTP 401、无重定向。没有用任一 Key 跨域测试，也未找到可确认两套 Key 互通或完整迁移的官方依据。
- 不把另一端点的 Chat、媒体或计量合同拼入原三协议接入；不自动改域名、改 Key 类型、尝试跨域重放或用控制台内部登录材料。若另一个端点也在接入范围，须按独立凭据与能力合同验收。

## Gateway 行为

- preset `sensenova-token-plan`、profile `sensenova_token_plan`、feature `line-sensenova-token-plan`；UI 入口 `sensenova-token-plan-api`。固定目标、普通 Key 表单、SecretPatch、grant 与 revision/CAS 复用现有 owner，不新增存储版本。
- 目录使用独立管理读取：显式账号或显式新 Key、敏感信息 grant、固定 `/models`，30 秒期限、2 MiB 响应和 2048 项上限，禁跳转。仅导入 `output_modalities` 明确含 `text` 的条目，排除 U 系列图像生成；失败保留旧目录。应用表单后仍需正常 CAS 保存，不自动发起模型推理。
- 公共 Chat、Messages、Responses 分别发往固定 `/chat/completions`、`/messages`、`/responses`；最终 sender 再校验目标、协议和选中 Key，清除其他身份头，关闭 ambient headers 和 redirect。目录模型健康探测仍使用 Chat，不是原生三协议的验收。禁止 Native Codex、XML tool fallback 绕过产品 owner。
- 非流式回复按各自的原生 wire 验证并有界读取；Chat 流以合法 finish、`[DONE]` 和 EOF 判定完成，Messages／Responses 守卫按各自的原生终态校验并等待干净 EOF。错误包、截断、非法 JSON 和错误尾部明确失败；已收到生成响应的协议失败不重放。
- 共享 Chat 守卫新增工具字段到齐校验：已出现的工具必须具有非空 ID、名称和 arguments 字段，不能由转换器生成默认工具身份。只保留字段存在标志，不积累第二份参数正文。Qwen workspace 也复用此修复，其原错误码和重试／fallback 策略保持不变。
- 文本模型手工选择仍可使用；不默认模型、价格、积分池余额或自动测试，不写真实接入验收标记。远端会话、后台任务、hosted tools、legacy Completions、Embeddings 与独立媒体生成入口未开放。Chat、Messages、Responses 的图像输入按各自合同处理，不等同于图像生成能力。
- 2026-11-22 复查上方官方文档静态资源 `210-35a7025cd2eaebe6.js`（SHA-256 `b4cec554990cb8deef4eeb05949bbdec4b1a85bdf45e7e09e1a982e442eae813`）。Chat 对 Flash／Flash Lite、DeepSeek Flash 和 Kimi K3 按解析后的真实模型校验图片来源和 Data URI MIME 类型；Kimi K3 仅收内嵌图片，不接受 URL。Gateway 只向固定 Chat 端点转发输入，不自行下载或解码图片，不替上游验证图像内容。2026-11-22 复核官方文档页面的公开资源 `210-35a7025cd2eaebe6.js`（SHA-256 `b4cec554990cb8deef4eeb05949bbdec4b1a85bdf45e7e09e1a982e442eae813`）：DeepSeek Flash 只在 Chat 提及视频输入，但没有足以确认对象类型、字段和限制的请求示例，Gateway 不猜测视频 wire。
- 同一官方文档要求 DeepSeek Flash 在带 `tools` 的多轮请求中回传历史 `reasoning_content`。Gateway 在原生 Chat JSON／SSE 中保留该字段；下一轮仅恢复能够与最终 Chat 打包结果安全对应的 assistant 字符串，允许 Gateway 改写 system 消息，但不覆盖或补回其他策略改写。非法字段或无法安全对应的历史在上游发送前拒绝。计费仍用 Chat 的 prompt／completion 用量，推理 token 不另加。
- Chat 内容块在原始输入、canonical 打包前和最终发送计划分别校验：文本／合法文本扩展及已批准的 user 图片保留；未确认媒体、未知块、非法 text 与非法 content 形状以 `sensenova_chat_content_unsupported` 拒绝，图片仍使用原图片错误码。原始非法输入即使后来被策略删除也不静默接受；空 assistant 工具内容和 reasoning 历史不受影响。这不是所有顶层媒体参数的白名单，也不是上游不支持视频的结论。源码公共 40 条拒绝后同账号 JSON／SSE 正对照及直接计划注入测试已通过；新补丁尚未出包，见[合并源码回执](../progress/2026-11-23-sap-stored-responses.md#拒绝记录与消息预付恢复)。

- Server 显式及定时文本探测先写内部审计，再发送；终态存储失败不报成功、不重放。该审计不进入公共请求或用户计费，并已在隔离 PostgreSQL／Redis 中完成双 AppState 和重建验证。见 [Server 探测回执](../progress/2026-11-22-sensenova-server-probe-audit.md)。

本地合成测试只证明 Gateway 合同。真实 Key、订阅权限、实际 SSE 字节、费用／上游退款、浏览器视觉、原生包、Hosted 公共推理和完整产品验收另列。Gateway 本地预扣恢复不代表商汤返还上游消费。
