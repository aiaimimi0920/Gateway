# Command Code Provider API 合同

实现线 `commandcode-provider-api`，preset `commandcode-api`，profile `commandcode_chat`，auth mode `commandcode_api_key`。使用 Studio 手工创建的 API Key，不使用 Go 套餐私有生成接口。已验范围及真实账号边界见[交付与验收](first-three-closure-2026-11-13.md#1-command-code)。

## 官方来源与实际目录

- [Provider 文档](https://commandcode.ai/docs/provider)：Provider API 的 Chat、Responses、Messages、Models 与 System One 五条路由；Bearer API Key 可用于所有 Provider 路由，`x-api-key` 是 Messages 的另一种认证方式。
- [Studio API Keys](https://commandcode.ai/docs/studio#api-keys)：用户在 Studio 获取自己的 Key。官方允许 API 套餐在第三方兼容客户端使用；没有已核实的 Gateway OAuth client 注册合同。
- [Go 套餐](https://commandcode.ai/docs/plans/go)：Go 不具备第三方 API 权限；GOAT、Pro、Max、Team、Provider 的具体权限仍取决于真实账号。
- [官方 Pi provider](https://github.com/CommandCodeAI/pi-commandcode-provider/tree/26e5fe68c2ab4c4d92216cc6114b27eb9401b516)：固定 SHA `26e5fe68c2ab4c4d92216cc6114b27eb9401b516`。README 对单路由的描述与当前多路由目录不一致，以目录的 `supported_endpoints` 和源码处理为准。源码采用 `max_tokens`，并把 developer 转为 system；其默认 reasoning 标记不能当成每个模型的官方能力证据。
- [公开 Models API](https://api.commandcode.ai/provider/v1/models)：本轮匿名 GET 得到 97 条，85 条包含 `/chat/completions`；12 条仅 Messages，72 条同时 Chat/Responses，13 条仅 Chat。数量不写死，未来刷新按实际目录筛选。

目录字段为 `id/object/created/owned_by/name/context_length/supported_endpoints`，没有价格、reasoning、模态或最大输出承诺；`created` 随请求变化，不作为模型发布时间。官方文档承诺流式末尾 usage，但本轮没有用真实账号生成，不能将合成 SSE 测试当成真实帧采样。

六份官方资料已保存 URL、UTC、SHA-256 和原文，位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-20261110/contracts/`。目录快照时间为 `2026-11-10T14:51:29.792679+00:00`。

## Gateway 行为

1. 目录读取固定的 `https://api.commandcode.ai/provider/v1/models`，不发送 Key，也不保存配置。导入与专属探测按 `supported_endpoints` 保留 Chat、Messages 和 Responses 能力，不按模型名猜协议；目录不是账号权益证明。手工模型仍由普通配置机制管理。
2. Gateway 对具备相应声明的模型调用原生 Chat／Messages／Responses，支持 JSON／SSE；同协议保留已支持的富历史与工具，Chat 转换回退不放行会丢失语义的输入。System One/Jev 是独立非流式结构化端点，现金预付在发送前拒绝。
3. 生成只使用账号自身凭据，不接受 caller 身份，不读取 CLI 凭据、不刷新、不跟随重定向、不回退私有 `/alpha/*`。Key 创建、轮换／保留用既有秘密槽和 CAS；保留凭据受产品与目标约束。专属探测见[内部审计合同](commandcode-server-probe-audit-contract.md)。
4. HTTP 4xx／5xx 保留状态，不自动重放；3xx 不跟随。坏 JSON、截断、错误尾部不会伪造成功或完成本地计费。本地取消与预算释放不表示上游退款。ZDR 约束贯穿候选选择与发送。

## 2026-11-23 公开合同复核：图片与 reasoning

已读取 [Provider 文档](https://commandcode.ai/docs/provider) 与匿名 [Models API](https://api.commandcode.ai/provider/v1/models)。文档明确接受文本和图片，排除音频、文件和文档片段；模型不具备视觉能力时由上游拒绝，不按模型预筛。匿名目录本次返回 104 条，仅含 `context_length/created/id/name/object/owned_by/supported_endpoints`，没有逐模型视觉或 reasoning 能力字段；数量不作接入常量。公开说明指向各原生协议 schema，但没有给出逐模型的 Chat reasoning 参数或历史扩展合同。

Gateway 原生 Chat 路径允许用户消息中的标准 `image_url`（HTTP(S) URL 或 `data:image/...;base64,...`），保留图片描述字段 `detail`。Gateway 不下载图片，也不根据模型名称猜测视觉能力。此变更不放宽跨协议 Chat fallback、音频／文件／文档或非用户角色图片；原生 Messages／Responses 的现有媒体与 reasoning 能力、账号权限和真实上游结果继续按各自的验收证据判断。总体状态以 [首批收口清单](first-three-closure-2026-11-13.md#1-command-code) 为准。
