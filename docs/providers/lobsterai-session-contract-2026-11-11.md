# LobsterAI 个人浏览器会话合同（2026-11-11）

本批是原生授权协议切片：独立 loopback 回调、exchange、回执保留与个人账号准入、401 刷新请求、动态目录。管理 API/UI、账号持久化、共享刷新调度与公共 Chat SSE 尚未接线，不登记 provider preset、manifest 或已验收调用回执。

## 官方来源与真实边界

本轮匿名联网下载官方 `netease-youdao/LobsterAI` 的固定提交 `5d6621a0aed22adb53439e290234cc2d64318caf`，11 个源码/许可文件及其 URL、获取时间、SHA-256 保存在本批 `contracts/sources.json`。未登录、未使用真实账号、未访问桌面凭据库。

- [endpoints.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/endpoints.ts)：生产 API 为 `https://lobsterai-server.youdao.com`；浏览器 Portal 为 `https://lobsterai.youdao.com/portal`。不接受可配置的 token 外发目标。
- [main.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/main.ts)：5982–6004 的 attribution、5946–5952 的 user ID、8180–8204 的浏览器授权、8295–8342 的 exchange/企业上下文、6208–6243 的目录合同。
- [authLocalCallbackServer.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/authLocalCallbackServer.ts)：动态 `127.0.0.1` 端口、`/auth/callback`、state/code；参考没有第三方 client_id 或 PKCE 协商。
- [authSessionManager.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/authSessionManager.ts)：285–350 的 JSON refresh、缺 successor 保留旧 refresh、迟到身份检查；官方代理仅将 401 当作刷新触发。
- [enterpriseAccount/context.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/enterpriseAccount/context.ts)：accountMode、enterpriseId、嵌套上下文；缺失模式在官方应用中还会请求企业上下文，不应自动推定个人。
- [claudeSettings.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/main/libs/claudeSettings.ts)：87–104 的 `modelId` 与能力字段；[modelRuntimeProfiles.ts](https://github.com/netease-youdao/LobsterAI/blob/5d6621a0aed22adb53439e290234cc2d64318caf/src/shared/providers/modelRuntimeProfiles.ts) 定义独立 Kimi runtime profile。

`source=electron` 是当前已观察到的服务端授权入口，不表示 Gateway 已注册自己的 OAuth client。Gateway attribution 使用自身 UUID、`neuro-gateway` 渠道和实际 Gateway 版本，不冒用官方桌面版本；服务端是否接受这些自有标识需要真实账户验证。MIT 代码许可不等于订阅转供或云端运营许可。

## 已实现的协议行为

`PendingLogin::bind` 先绑定动态 loopback，再构造含随机 state 的登录 URL。调用方显式打开浏览器并拥有等待 future；没有内部 detached task。取消/丢弃 future 释放 listener 和当前连接。总期限 300 秒，单个请求头读取最多 5 秒、32 KiB；精确 Host/路径、GET、重复参数和 HTTP framing 检查复用既有 DIM 的解析逻辑。错误 state 不消费授权，合法 code 只交付一次。

`CodeGrant::exchange` 消费 grant，仅向固定 `/api/auth/exchange` 发送一次 JSON；先接收业务信封，再通过 `ReceivedSession::admit` 解释账户。收到但不准入的原始回执留在调用者手中；它目前仅在内存，不宣称已保存到 Gateway 凭据池。成功准入只能交付一次可刷新会话。

初次准入要求明确 `accountMode: personal` 和 `data.user.yid` 或 `data.user.userId`，检查已知嵌套上下文中的企业身份与 `quota.subscriptionStatus: enterprise`。企业、未知模式、无模式及缺少稳定用户 ID 均拒绝；缺模式保留回执并报告 `lobsterai_account_context_required`，本批不实现额外的企业上下文查询。账户身份来自服务端端点响应，不称 JWT 验签身份。

刷新仅接受当前 exact access token 遭遇 401 的请求，不将 403、429 或 5xx 当作过期。请求为 `/api/auth/refresh` JSON，无 Bearer；响应未返回 refresh、null 或空值时沿用旧 refresh。若响应带 user，必须与旧身份连续；缺 user 时使用本次 refresh 链原有身份。新回执在准入失败时仍保留，不因失败自动重发。

目录为携带当前 access token 的 `GET /api/models/available`，要求 `code:0,data:[]`，按 `modelId` 解析，过滤明确 `accessible:false`。只导出已声明的工具、reasoning、媒体、token limit、apiFormat/runtimeProfile/requestCapabilities 等目录事实；缺失保持 `None`，不声明这些能力已可推理，不替特殊 runtime profile 选择普通 Chat wire。空目录正常返回，不使用静态模型表兜底。

发送和解码共用 15 秒预算，token 响应上限 64 KiB、目录上限 2 MiB/2048 条，重复 JSON 成员在解释前拒绝。关闭跳转与继承默认头，错误只输出固定分类，不转发可能含凭据的上游文字。没有自动重试、签到、桌面导入、更新检查或 tunnel。

## 继续接线前的硬边界

Lobster 官方 token 合同未承诺 expiresIn。本实现不捏造过期时间，也不把未知时间交给当前定时刷新 owner。共享 `TokenRefreshState`、`GrantResult` 和 publication 目前都要求 expiry；后续需要显式的 401 驱动策略与 durable grant receipt，并复用现有 generation/single-flight/CAS，不能简单注册新 adapter 后默认六小时。

本批 `RefreshGrant` 只处理单次协议请求。生产管理 owner 仍须在发送前持久化 reservation、响应后先持久化 receipt、再发布 successor，并将身份切换、取消/shutdown 和迟到结果纳入 generation 校验。当前内存回执不提供崩溃恢复保障，因此尚不开放 UI/管理保存或公共推理入口。

Chat SSE、非流式聚合、函数工具、取消退款、订阅权益与消费、真实浏览器登录/刷新、Server/Redis、EXE 和独立 feature 构建均未完成本产品验收。社区所述 `stream:false` 仍返回 SSE 没有本轮真实调用证据，不把它当作实测结论。
