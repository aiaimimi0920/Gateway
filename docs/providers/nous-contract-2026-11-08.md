# Nous Portal：本地原生接入合同与客户端边界

2026-11-08 匿名联网核对时尚未实现；2026-11-09 已新增协议与 received-grant 前置切片，
见[前置协议证据](../progress/2026-11-09-nous-received-grant.md)及后续
[typed storage 与未接入范围](../progress/2026-11-09-nous-typed-material.md)，不代表完整产品或真实授权、目录、推理验收。
后续 [自动 Received worker 与目标旋转](../progress/2026-11-09-nous-worker-lifecycle.md) 也已通过本地定向验证，仍非完整产品验收。
后续 [独立产品与公共推理切片](../progress/2026-11-09-nous-public-runtime.md) 已通过本地合成上游验证。
[管理登录、明确保存与目录 UI 切片](../progress/2026-11-09-nous-management.md) 已补本地定向证据；
初始 device receipt 仅有界进程内保管，已保存账号 refresh 仍由 durable owner 负责。
后续 [双目标推理与实现线清单](../progress/2026-11-09-nous-inference-metadata.md) 已补
Subscription／Welcome 的 Embeddings 与有限 legacy Completions 公共链路，最终 Nous 库 57 项通过。
真实产品／独立 client 资格、浏览器视觉、Server 与原生包验收仍未完成。
没有读取 Hermes profile、账号文件或 token，没有安装/执行参考包或进行真实上游推理。

来源固定到 [Hermes 3e82260b](https://github.com/NousResearch/hermes-agent/tree/3e82260ba041b75cf9eb20abf13bec937bb32e27)
和 [社区插件 93dacd73](https://github.com/magpie-community/plugins/tree/93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/nous)。

## 独立客户端资格没有被代码参数证明

官方 `hermes_cli/auth_nous.py:1460-1478` 接受显式 client ID；但同一提交的
`skills/autonomous-ai-agents/hermes-agent/references/portal-auth-for-third-party-apps.md:39-49`
明确称没有第三方 App 可注册的通用 Portal SSO，现有流程是 Hermes-specific。
社区 README 也声明尚未确认独立客户端可使用 `hermes-cli`。
因此本地实现只能要求操作者明确提供获准 client，不内置/借用该身份；真实资格单列，
不把参数可配置等同于厂商已开放注册。Dashboard 的 `agent:{instance_id}` PKCE/JWKS
是另一合同，不能作为 inference device grant 的注册或验签依据。

## Device 与单次 refresh

- `auth_device_flow.py:260-275`：form POST `/api/oauth/device/code`，显式 `client_id`，
  `scope=inference:invoke`。读取 device/user code、验证地址、有效期和 interval。
- `:399-428`：form POST `/api/oauth/token`，device grant type、相同 client 与 device code。
  首次等待 interval，pending、slow_down、取消、到期和未知网络结果分别处理。
- `auth_nous.py:702-708`：refresh 也 POST `/api/oauth/token`，
  **form 仅 grant_type 与 client_id，RT 放 `x-nous-refresh-token` header**。
  不照搬 Dashboard 将 RT 再放 form 的不同合同。
- `:747-759` 明确 RT single-use，重复使用可撤销整条 session chain；
  `:868-879,925-934` 的纯状态刷新先持久化 successor，再做后续 inference JWT 验证。
  Gateway 应接既有 durable grant reservation/result/apply/replay，不另建仅进程内锁。

失效分类存在真实源码分歧：官方 `:737-743` 把无 OAuth error 的 401/403 转为 invalid_grant；
社区只把明确 `invalid_grant`、`invalid_token`、`refresh_token_reused` 判为失效，保留未知拒绝。
两者均区别 WAF/暂时错误。后续测试必须保留明确终止、WAF、未知拒绝三类，
不能让代理/HTML 拒绝清空账号，也不能在结果不明时重用旧 RT。

## 身份、目录与推理

- 账号标识取 inference JWT 的非空 `sub`，缺失时是 unknown，不把两个 unknown 视为同账号。
  只 decode JWT 不等于验签；没有找到 device inference 必须调用的 userinfo/me 合同。
- `auth_nous.py:830-838`：GET `https://inference-api.nousresearch.com/v1/models`，
  Bearer 为 inference access JWT。保留含 `/` 的原始模型 ID，成功空目录与失败要分开。
  不把社区未知能力/价格默认值复制成免费或全部工具支持的厂商事实。
- 官方 `hermes_cli/proxy/adapters/nous_portal.py:30,87-102` 与社区源码的已见路径是
  `/chat/completions`、`/completions`、`/embeddings`、`/models`。
  Access token 本身就是 inference token，不需要 mint agent key、HMAC、DPoP 或闭源 signer。
- 未找到原生 Responses/Messages 合同；Gateway 这两种入口必须走既有 Chat 转换，
  不能向 Portal 猜测原生路径。Chat 工具与 SSE 可复用既有 wire，而不是把历史拼成普通文本。
- 当前 Gateway legacy Completions 也使用 Chat 桥接，只支持单字符串 prompt 与已有通用生成参数。
  批量/token prompt、echo、logprobs、suffix、多候选和非空 logit_bias 无法保真，发送前明确返回
  `nous_legacy_completions_unsupported`，不宣称已实现上游原生 `/completions` 的全部能力。
- 本地 Embeddings 走 JSON passthrough，保留数据、编码、参数与上游 model；结算先保留旧 usage
  识别，再补 input-only 用量，不给 Chat 的缺失 completion 伪造零。以上均为本地合成证据，
  不证明真实账号可调用某个模型或获得某种权益。
- 社区的一次 401 refresh/replay 不是 Gateway 盲重放许可；必须遵守已有未知消耗/部分输出边界。
  Token 不能随网络下发的任意 inference host 流出；固定域名、路径、headers 和 redirect fence
  仍由 Nous 产品 owner 负责。

[官方 Portal 文档](https://hermes-agent.nousresearch.com/docs/integrations/nous-portal)
另说明 OpenRouter 专属 `provider`、`session_id`、顶层 `cache_control` 不是 Portal 合同。
本地切片不包括 Hermes 凭据共享、匿名权益/浏览器挑战、Dashboard 登录、Tool Gateway、
真实价格与独立客户端注册资格；真实 client、授权、旋转、订阅和费用验收另列。
