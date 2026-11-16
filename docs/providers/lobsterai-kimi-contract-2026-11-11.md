# LobsterAI Kimi K3 接入合同核对（2026-11-11）

本轮匿名联网核对官方固定提交 `5d6621a0aed22adb53439e290234cc2d64318caf`。结论：现有普通文本转换不能完整保留 Kimi 工具续轮所需的推理历史；继续拒绝特殊 profile，不声明 `kimi-k3-agentic-v1`。此记录不是接入完成或真实账号验收。

## 已确认的官方行为

- [modelRuntimeProfiles.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/shared/providers/modelRuntimeProfiles.ts)：`moonshot-kimi-k3` 使用 `max_tokens`、`requiresStringContent:true`、`supportsUsageInStreaming:false`。server model 只认服务端 runtime profile，不按模型名字猜测支持。
- [claudeSettings.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/claudeSettings.ts)：`evaluateServerModelRunGate` 要求 OpenAI transport、`supportsToolCalling:true` 和 `agenticReady:true`；这些是服务端声明，客户端没有根据本地字段自行推导 agenticReady 的规则。
- [kimiK3StreamWrapper.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/openclaw-extensions/lobsterai-model-compat/kimiK3StreamWrapper.ts)：删除顶层 `thinking`、`reasoningEffort`、固定采样参数，设置 `reasoning_effort:"max"`。在 onPayload 回调前后均执行，不能直接把 UI 中的 off 映射解释为上游 reasoning 已关闭。
- 同一 wrapper 对 assistant 的非空 tool_calls 检查 `reasoning_content` 是否存在：仅缺少该属性时补空字符串，已有内容不覆盖。
- [index.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/openclaw-extensions/lobsterai-model-compat/index.ts) 配置 `sanitizeToolCallIds:false`、`dropReasoningFromHistory:false`，明确保留工具 ID 和推理历史。
- [requestOptions.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/openclaw-extensions/lobsterai-model-compat/requestOptions.ts) 的用户 thinking 控制另使用 `lobsterai_options:{version:1,thinking:{level:...}}`，必须有服务端 request-options 能力声明；不能与原始顶层 thinking 混同。

## 当前 Gateway 缺口

- `CanonicalMessage` 没有消息级 reasoning 状态。Chat normalizer 仅在 raw_body 保留原文；重新 pack messages 时会丢失 `reasoning_content`。
- `CanonicalRelayResponse`、Chat JSON unpack/build、canonical-to-SSE 只保留文本与工具，没有可回放的 Kimi 推理状态。
- 原生 Chat SSE guard 会保留未知 delta 字段，但没有 Kimi reasoning 类型与有界累计合同；Chat 转 Responses 等路径会丢失 reasoning。单个原生流能透传不代表三入口完成。
- `req.reasoning` 没有到 Kimi 请求控制的统一映射。现有 `text_tools_request` 的拒绝边界有实际数据丢失依据，不能简单移除。
- 当前 LobsterAI catalog 未保存 `agenticReady`。未来需要向后兼容保存官方准入事实，并把 fresh metadata 与运行资格联系起来；不能本地伪造 true。
- 既有 Bedrock／Vertex 有序状态明确限制来源，不得借作 Kimi reasoning 容器。

## 后续实现所需闭环

完整接入需要同时处理：服务端资格与 freshness、带来源约束的历史保留、官方请求参数合同、JSON／SSE 推理内容及有界聚合、支持入口的响应回放、工具参数与 ID 无损、最终 usage 未知时的账单语义。先确定支持入口，未支持的跨协议转换须明确拒绝，不能默默丢 reasoning 后宣称成功。

本轮没有继续下载底层 SDK 追踪 `requiresStringContent` 与 `supportsUsageInStreaming` 的所有序列化细节；已确认配置声明，尚未证明多模态到字符串的规则或整个 stream_options 对象的删除规则。图片／视频与真实账号资格继续开放。

## 普通目录 Anthropic 的独立合同

`claudeSettings.ts` 使用每个模型自身的 apiFormat 与 `${serverBaseUrl}/api/proxy/v1`；[openclawConfigSync.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/openclawConfigSync.ts) 将普通 Anthropic 映射为 `anthropic-messages`；[openclawTokenProxy.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/openclawTokenProxy.ts) 将非 resolve 请求映射到 `/api/proxy${req.url}`，因此 `/v1/messages` 的目标为 `/api/proxy/v1/messages`。此路径不要求经过 yd 虚拟模型解析。

源码与 SHA-256 清单位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-declared-messages-20261111/contracts.json`；本轮新增联网源位于相邻 `gateway-lobsterai-kimi-20261111/contract-audit`，前批合同源沿用固定 SHA。
