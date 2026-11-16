# 新增授权渠道的无凭据实施合同

> 2026-11-02 联网只读调查；不含用户凭据，没有向授权、token、refresh 或推理 endpoint 发出业务请求。
> 本文是协议证据和下一步实施输入，不是已实现、已获厂商客户端授权或已真实验收的声明。
> 关联：[渠道身份表](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-channel-identity.md)。

## 1. Grok Build OAuth：独立的 CLI proxy

官方源码：[xai-org/grok-build @ 2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8](https://github.com/xai-org/grok-build/tree/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8)，Apache-2.0。
本次通过公开 [OIDC discovery](https://auth.x.ai/.well-known/openid-configuration) 核实 issuer、endpoint 和 grant。

| 职责 | 已核实合同 |
| --- | --- |
| 发行方 | `https://auth.x.ai` |
| 浏览器授权 | `/oauth2/authorize`；code + PKCE S256 + state + nonce，loopback `/callback` |
| 设备码 | POST `/oauth2/device/code` form `client_id/scope` |
| token | POST `/oauth2/token` form；authorization_code、refresh_token、device_code grants |
| refresh | `grant_type=refresh_token/refresh_token/client_id`；存在时保留 principal_type/principal_id |
| 轮询 | pending 继续；slow_down 加 5 秒；拒绝/过期终止；有界 TTL/取消 |
| 业务受众 | `https://cli-chat-proxy.grok.com/v1`，不是 `https://api.x.ai/v1` 或 grok.com Web |
| 推理 | `/responses` JSON + SSE；不能据此宣称全部 Responses 扩展完全等价 |
| 官方发包身份 | 源码发送 `X-XAI-Token-Auth: xai-grok-cli` 及产品 headers；哪些服务端必需尚未实测 |
| 额度 | Build vs Web/API `unknown`；个人/Team principal 不能丢失 |

固定源码入口（均相对上述 pinned commit）：

- `crates/codegen/xai-grok-login/src/config.rs`：issuer、scopes、官方应用自身 client 身份。
- `crates/codegen/xai-grok-login/src/oidc/protocol.rs`：PKCE、form exchange、refresh、ID-token audience/nonce。
- `crates/codegen/xai-grok-login/src/device_code.rs`：设备码 polling/error 合同。
- `crates/codegen/xai-grok-env/src/lib.rs`：CLI proxy origin。
- `crates/codegen/xai-grok-login/src/grok_auth_credentials.rs`：业务 bearer/auth headers。
- `crates/codegen/xai-grok-sampler/src/client.rs`：实际 Responses JSON/SSE 发送。
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`：不将第一方会话 token 发到第三方 BYOK endpoint 的受众约束。

实施采用 `grok-oauth` 独立渠道，不将 token 注入 `xai-api` 池。OAuth client 应显式、准确登记；没有公开第三方注册合同，不能把源码可见等同于获准冒充官方客户端。
先实现固定目标、材料生命周期和 mock wire；官方 client/产品身份的可用与授权边界在真实登录前核实，不通过去掉受众约束强行打通。
scopes 以所选获准客户端为准；公开源码出现的 scopes 不等于每个 Gateway 用户都有对应权限。

无凭据实施已进入 [P23 独立协议底座](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p23.md)：本批新增固定issuer、显式client/scopes、PKCE/state/nonce、code/refresh/device有界transport和合成合同；最终测试状态以回执为准。transport只返回UnverifiedTokens，不代表已校验OIDC身份、完成登录/持久化或接入refresh scheduler；官方CLI身份未内置。

### 1.1 OIDC 身份验证不能照搬 payload decode

2026-11-03 固定参考复读：`oidc/protocol.rs:609-664` 的授权码非Team路径要求kid、所选JWK与允许alg匹配、验签、sub/iss/aud/exp与本次nonce；discovery当前仅声明ES256。`698-727` 只有Team豁免缺ID-token，Organization不在这个code-flow豁免中。不能把现有ChatGPT仅base64解码的owner当作已具备这些验证。

`device_code.rs:373-415` 则明确不验ID-token签名，仅提取展示字段；device可缺ID-token、没有browser nonce，单独处理Team/Organization。access-token中的principal peek同样不等于JWT验签。Gateway实现必须区分已验证身份、发行方返回的未验证材料与展示hint，不能借这些claim授予管理权限、合并账号/额度或绕过正常Secret Grant。固定JWKS网络同样需要禁重定向、body/key数量/deadline预算及固定错误；P24前的仓库基线已有ring依赖，但没有可直接复用的ES256/JWKS验证owner。

参考的`oauth2.principal_id`是consent预选择，不等于强制pin。若Gateway提供强制principal约束，必须独立命名并在真正保存前按适用证据检查；材料整个生命周期仍绑定原client/issuer/scope/principal上下文，不能把预选择变化的会话混用。不得因普通用户ID-token规则而静默取消合法Team/device渠道，也不得伪造已验证的Team身份。

[P24](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p24.md) 已新增固定JWKS/ES256验签和有ID-token的code-flow身份owner，85项定向离线验证与代表性产品feature关闭编译通过；未改变UnverifiedTokens或接成完整登录。当前明确只信任自身audience（字符串/单元素数组），存在azp必须匹配，额外受众不能因azp正确就自动信任；该策略依据2026-11-03再次联网核实的OIDC Core §3.1.3.7。Team/device acceptance、principal和完整会话持久化另行接线，不被这个end-user validator替代。

### 1.2 Grant receipt 和实例会话不是正常保存

[P25](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p25.md) 已实现实际code/device grant的receipt：只有实际hint精确Team且ID完整时，code-flow可缺ID-token并保持TokenEndpointOnly；非Team走P24，device不伪造nonce或SignedEndUser。强制PrincipalConstraint与consent预选分离；access token先按32KiB限长，opaque或带点号不强制JWT，只有JWT-shaped header才解析未验签hint，明确JWT的歧义aliases仍拒绝。

本批Console registry按Gateway实例拥有，绑定owner/provider/revision、16会话/600秒、单次callback/claim、取消/drop/TTL与退出清理。claim仅进入Claimed，不代表保存成功。Binding构造不代替HTTP鉴权或真实provider/preset/revision实查；正常Secret Grant、device/listener、SecretPatch/CAS、提交ownership、refresh及业务调用仍需后续接线。首版44项通过，opaque公开路径实际红测已复现并修补，最终v3协议/会话48项与相邻16+40项合计104项通过，代表性产品feature关闭编译通过；仍不能宣称正常Console登录或全功能完成。

### 1.3 独立业务与专用刷新已接，正常管理保存仍待接

[P26](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p26.md) 新增独立 backend preset/auth marker/profile 与固定 Responses planner/sender。原始 preset 及无 preset 的 marker/固定域名都受配置校验；最终 plan/header merge 后再次验证，不进 native Codex、compact/WebSocket、generic model lookup 或 connectivity probe。合成 loopback 实际捕获 buffered/stream 请求，最终64项产品/授权/会话及99项相邻回归通过；不是整个渠道或公共 runtime 全保真完成。

专用 refresh owner 未接通前，legacy refresh 字段明确拒绝，不能用任意 endpoint/client ID 启动通用 form refresh。此临时限制不是最终支持标准，也没有关闭 refresh 需求。下一步应从实际 receipt 保存完整显式 client/scopes/principal 上下文，并把它纳入 SecretPatch keep、refresh publication 与 persistence 的授权等价判断，再接既有 single-flight/generation/rotation/CAS；正常登录管理和保存不能只靠手工构造 Binding 或重新取得较新 revision。

上述是P26结束时的限制。[P27](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p27.md) 已接context/专用refresh：显式缩权与轮换材料共同持久化，opaque token不被强制解释为JWT，缺hint不提升identity assurance；最终155个去重测试、代表性feature关闭编译与静态检查通过。[P28](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p28.md) 的共享授权材料/独立routing wrapper、一致性ticket及pending持锁已补进程内policy/enable交接，231个去重定向测试、代表性feature关闭编译与静态检查通过。Server重启仍可能从旧document恢复材料，且当前没有发送grant前的跨实例互斥；这两项转P29，不能用cache CAS或进程内锁冒称完成。正常管理登录与Secret Grant保存亦未接通，不能宣称完整刷新或登录已完成。

[P29](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p29.md) 的正常可写Server双阶段前置已有247个去重测试及代表性编译/静态证据：authority先保存，cache失败保留pending和ticket原期望材料；旧bootstrap的新runtime可经正常恢复读取最新材料。此证据不覆盖只读Worker、跨实例grant所有权、结果不明和崩溃后派生cache修复；这些继续作为P29退出条件，不将Worker通用automation权限放开，也不将该前置当完整OAuth登录或刷新生命周期。

P29 v3最终为253个去重定向测试，新增正常Server authority的20秒总预算和runtime clone共享恢复owner；先前4项无界等待及1项旧pending丢失恢复义务的红测均已修复。失败/中断后先恢复再续存同一rotation，模糊事务保持pending，不误报成功；Local不变。已发送Redis命令仍可能在客户端取消后迟到执行，需正常backend的精确transaction撤销/恢复栅栏；它不是已完成的服务端取消、多实例fence、完整scheduler join或OAuth管理登录，完整P29及原R01–R16仍未完成。

[P31](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p31.md) 已补受限Worker的authority→cache链：私有typed ticket、有限投影、完整Prepared身份/parent/new archive绑定的不可变准入证明，所有pending先验证再settle；未知管理/普通automation或残缺journal不被Worker顺手恢复，普通写与公开startup恢复继续拒绝。最终395个去重定向测试、代表性feature关闭编译与静态检查通过，包含实际scheduler/flush和13项新Worker测试；旧通知permit造成的混合pending夹具失败与仅测试v3修正均保留。本批`refresh-intents`仅证明配置事务曾获接纳，不证明grant发送所有权；跨实例发送、结果不明和崩溃后的token/cache处置继续P32，正常OAuth管理登录也仍未完成。

### 1.4 正常管理 HTTP 与 SecretPatch 保存已接

[P35](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p35.md) 已把实例会话接入真实管理router与普通保存：create/complete/cancel/import要求管理身份和Secret Grant，status要求同owner；真实provider/preset、固定base和原expected revision分别实查。client/scopes显式输入，不开放任意issuer或借用官方CLI身份。receipt留在服务端，通过正常redacted draft、SecretPatch与原revision/CAS保存，refresh缺省明确clear旧值、expiry不延长。保存operation在registry内跟踪并join，重复import不重提交；只有normal commit返回才报succeeded，错误/超时/drop为save_unconfirmed，不猜成功。正常HTTP/保存的71项Grok、128项Console与15项管理/事务合同通过，详情含真实失败归因和扩大文档合同的旧loader失败；不能据此宣称全仓绿。精确接口与限制见[管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-grok-oauth-login.md)。前端UI/模板、device/listener、结果不明的完整恢复交互、真实客户端授权和其他既有刷新/业务/集中验收项仍保留。

## 2. Accio：找到 PKCE 与专有 JSON exchange/refresh 参考

参考：[MYCF711/magpie-accio-auth @ 62993d5d7dc872ea4f62ad50116b698c62ea513a](https://github.com/MYCF711/magpie-accio-auth/tree/62993d5d7dc872ea4f62ad50116b698c62ea513a)。
已核实 MIT；LICENSE 署名占位仍须保留，不能改成官方署名。P41 按固定 wire 独立实现 Rust 协议 owner，采用方式和原始 MIT 声明保留在 `src/protocol/accio_oauth/REFERENCE.md` 与同目录 `LICENSE`。
这不是官方集成文档；另一未确认许可的 accio-manager 候选未作为可采用源码。

| 职责 | 参考实现的具体证据 |
| --- | --- |
| gateway | `https://phoenix-gw.alibaba.com` |
| 登录网站 | `https://www.accio-ai.com` / `https://zh.accio.com`；不能外推两者产品额度相同 |
| 浏览器参数 | return_url、state、code_challenge、S256、client_id、ttid |
| code exchange | POST `/api/oauth/token` JSON `code/codeVerifier/clientId/redirectUri` |
| refresh | POST `/api/auth/refresh_token` JSON `accessToken/refreshToken`，需要当前 access token，不是通用 form |
| 返回 | 非空 accessToken/refreshToken、expiresAt；参考实现将 expiresAt 秒转毫秒，须单独测试边界 |
| 客户端 | 参考使用 accio-work；没有据此证明 Gateway 获准使用该客户端身份 |

固定源码：`src/constants.mjs`、`src/login.mjs`。参考称 refresh 会轮换，本轮没有服务端轮换实证。
Gateway 旧 `accio` 业务 wire 的 body token + utdid 继续复用；新授权和旧材料导入不因获取方式不同就自动生成两个池。
保留原需求 `accio-oauth`，后续确认同一受众/产品是否同一授权链；只有发现真正独立 Web 额度才另拆 Web 子服务商。
需要在共享 refresh 生命周期中支持“当前 access + refresh”契约，不能把 endpoint 填到旧 form transport 了事。

### 2.1 P41：独立协议 owner，不冒充完整 OAuth 登录

`src/protocol/accio_oauth.rs` 及其子模块已实现显式 client、两个固定登录站点、48-byte PKCE verifier、32-byte state 与单次 ttid；只构造非零端口的 `http://127.0.0.1:<port>/auth/callback`。授权 attempt 与 code grant 不可 Clone/反序列化，消费式 callback/换码共享 600 秒 monotonic TTL；无 listener、管理认证或后台轮询由此隐式产生。

换码/刷新分别固定发送到上述两个 JSON endpoint，不继承调用 client 的默认秘密 headers，不跟随重定向或自动重试。单次发送及 body 读取共享最多 15 秒预算，保留最多 64 KiB 响应 bytes；压缩响应、截断、取消及重定向的实际证据以 [P41 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p41.md) 为准。错误只导出本地固定码，不用上游文字推断永久失效。

响应必须 `success=true`，已知关键字段重复会拒绝；access、refresh、absolute expiry 三者缺一不可。数值 expiry 按固定参考的 `>1e12` 毫秒/否则秒规则，以原始 JSON 十进制做精确运算，最多接受 9 位小数，不经浮点舍入；日期只接受带显式时区的 RFC3339。指数、无时区、非法或超范围值拒绝，不默认一小时/六小时。有效但已过期的轮换材料保留其原绝对时间，以免丢失已轮换 refresh；这不代表允许过期 access 继续业务调用。

可选 `sgcookie` 有新有效值则替换，省略/null 时保留旧值，空白/非法值拒绝；不作为 refresh Cookie header 发送。材料类型不声称已验证账号身份，不解码 JWT 推定 ID，也不制造 `utdid` 或额度归属。

P41 当时正常管理登录、utdid、保存和共享 refresh 持久链均未接，scheduler 在 reservation 前以可恢复 gate 拒绝 Accio，防止 generic form 误用。该 gate 不是功能完成；**后续 P43 已完成下文的持久刷新接线并替换 feature 开启时的 gate，正常 OAuth 管理登录/模板/UI 仍待实现**，不能把这一历史描述当作当前源码状态。

### 2.2 P42：当前业务 token 的所有权

[P42](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p42.md) 已进入实际 planner 修补：非空 `payload.api_key`（内部 access-token 字段）优先于旧副本；缺省时兼容可信配置的 `extra_body.token/accessToken`，冲突或非法材料明确拒绝；caller 不作为认证来源。最终 body 只用 canonical `token`，`utdid` 必须来自可信 header，随后 generic merge 不得重新加入认证/设备别名或 refresh/sgcookie 材料。旧 token-only + header-utdid 配置保留，不修改存储文档，也不生成设备身份。

初轮行为红测 1 passed / 11 failed，修复后初轮 80 passed / 1 failed；后者是新 routing fixture 缺必填 `base_url`，已仅补齐夹具，不改生产或削弱断言。最终 270 个不同 Rust 测试通过（13 个新增），代表性 feature-disabled 编译、格式/ratchet/checker 31 项及五项源码冻结审计通过；范围和失败记录见回执。这是最终 RequestPlan 的认证绑定，不代表完整登录/持久刷新或真实上游调用；P42 当时仍保留 P41 gate，后续接线见 P43。

### 2.3 P43：完整材料进入共享持久刷新

[P43](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p43.md) 新增 credential-only secret `accio_sgcookie`，接正常 descriptor/redaction、SecretPatch keep/replace/clear 和完整 resolved destination 保护，不加 provider 继承、业务 payload、headers 或 extra body。无 SG 时新字段省略，旧 JSON/YAML、7 项 material ID、Sending record identity 和 locator 不变；family ID 不因 SG 改变。

feature 开启时专用 Prepared 严格绑定 Accio adapter、固定根 gateway/refresh endpoint、显式 client 与当前 access/refresh；关闭时仍在 grant reservation 前拒绝。成功后的有效最终 SG 和原绝对 expiry 按 durable Result → apply → authority → Published → cache 贯穿；replay 不重新计算 TTL、不猜 SG。过期但成功轮换的材料保存为已过期，正 lifetime 最低 1 秒仅为兼容元数据，不延长 dispatch 授权。cache 同时比较旧 SG/完整 raw，拒绝错误类型与矛盾 alias；Lua 与五重 raw CAS 算法未改。

两项实际红测后，最终 418 个不同 Rust 测试（19 新增）、代表性 Accio 关闭编译、checker 31 项及格式/ratchet/冻结审计通过。正常管理 HTTP 的 SG 替换/清空、Grant/Origin/原 revision/CAS 和旧新产品 Keep 已覆盖，但不是 OAuth attempt/callback/receipt 登录流程；真实上游、Redis Lua 本批新执行、真实凭据及正式包均未验证。正常登录/保存 UI、utdid 交互、模板与精确业务/额度仍需继续，R01/W01 不标完成。

### 2.4 P44：正常管理登录与服务端保存

[P44](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p44.md) 已完成专用管理会话与服务端保存的定向离线验证；完整接口见 [Accio 管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-accio-oauth-login.md)。create/status/complete/cancel/import 复用管理身份、Secret Grant、真实保存目标和原 revision/CAS，不把 PKCE/state 当管理权限或已验证平台账号。最终 461 个不同 Rust 测试（18 新增）通过；4 个真实 Redis opt-in 测试本批未启用，代表性 Accio 关闭编译与 checker 31 项/格式/ratchet/冻结审计通过，原失败与夹具/构建输入恢复记录均保留。

client/site/port 显式输入，只生成固定 loopback callback，当前无 listener。新增账号需显式 utdid；重授权省略仅保留目标 credential 自己已配置且无歧义的设备，不继承 provider。access/refresh/SG/原绝对 expiry 留在服务端并按正常 SecretPatch 提交；新登录缺 SG 明确 clear 旧 SG，不改变 P43 refresh 缺 SG 保留语义。旧 credential 认证副本及 empid/tenant 七别名移除，provider 存在这些已知副本时拒绝专用登录，不清共享 provider 或其他账号；其他 metadata 与未知辅助秘密保留正常 keep 安全约束。

每实例 16 会话、600 秒登录 TTL；保存由受跟踪 task 持有，20 秒预算包含 publication wait，正常 shutdown join。仅 normal commit 成功为 succeeded；错误/timeout/drop 是 save_unconfirmed，重复 import 不重投。P44 当时正常模板/UI 尚缺，后续进展见 2.5；真实 client 许可/平台登录、精确业务/probe/额度及集中凭据验收继续，不标整卡完成。

### 2.5 P45：正常空池模板与专用 UI

[P45](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p45.md) 已新增 `accio-oauth` 正常模板，兼容 preset 仍为 `accio`，最终投影只允许固定 Phoenix 地址；默认无账号、预置模型或新增路由，不重写历史配置。真实全局 CredentialDialog 接入新增/稳定 ID 重授权、显式 client/site/port/utdid、手动完整 callback 和服务端独占保存。旧手工 Access Token 入口、metadata/keep 编辑保留；有活动/未知保存时锁模式出口但不禁止关闭，重开继续查同会话。

只抽取与 Grok 相同的有界 HTTP/session 生命周期，产品 schema、输入、状态和 operations Map 分离；Accio 不接受 claimed，不要求 identityVerified。65 项本批新增测试通过；扩大组合去重 325 passed / 3 failed，三项失败用本批 before-source 覆盖独立复现，未修改 Qwen 或 Gemini 的测试/契约来掩盖。类型、checker 31 项、ratchet、隔离真实 main/组件构建及 1280/390 CSS 像素合成浏览器检查通过；不是全仓绿、真实登录、真实管理后端 E2E 或完整渠道完成。

## 3. FreeBuff：官方 CLI/browser-assisted session，不冒称标准 OAuth

官方源码：[CodebuffAI/freebuff @ 35c6bb288d2af2efc85616c572774fd8ab47e3db](https://github.com/CodebuffAI/freebuff/tree/35c6bb288d2af2efc85616c572774fd8ab47e3db)，Apache-2.0。

| 职责 | 官方公开实现 |
| --- | --- |
| 浏览器批准 origin | `https://freebuff.com`；新登录入口不继承参考项目任意环境变量覆盖，不据此推断业务 host |
| 获取登录链接 | POST `/api/auth/cli/code` JSON `fingerprintId`；不带现有业务认证 |
| 返回 | loginUrl、fingerprintHash、expiresAt；shared 注释另说明 expiresInMs，当前 interface 投影未列全 |
| 完成状态 | GET `/api/auth/cli/status` query `fingerprintId/fingerprintHash/expiresAt`；出现 user 表示完成 |
| expiresAt | opaque string，服务端时钟/HMAC 输入，必须逐字回传；不重格式化、不自行按秒或毫秒解释 |
| 等待预算 | 官方 CLI 默认 5 秒 poll、60 分钟相对 TTL；5 秒是客户端默认，不是已证明服务端 interval 指令 |
| 持久材料 | authToken、fingerprintId、fingerprintHash |
| 业务认证 | `Authorization: Bearer <authToken>`；不证明它是平台推理 API Key |

固定源码：`common/src/constants/hosts.ts`、`common/src/constants/auth.ts`、`common/src/util/credentials.ts`、`cli/src/login/constants.ts`、`cli/src/login/login-flow.ts`、`cli/src/utils/codebuff-api.ts`。
当前证据不包含标准 device_code/user_code/verification_uri/token grant；不把 issue 中的 device-OAuth 俗称当 RFC 合同。
`freebuff-web` 或 session 命名可由该浏览器批准渠道承载，不需因“登录”和“导入”两按钮而重复建池；最终界面名称在接线时冻结。
[FreeBuff Ads Key](https://freebuff.com/docs/advertisers/authentication) 属广告管理，不是推理 Key；Codebuff SDK Key 也不是 FreeBuff 产品授权证明。
仍追踪 R06 原 `freebuff-api`，不能借广告 Key、Codebuff Key 或第三方代理 Key 凑齐该需求。

### 3.1 独立批准协议已实现，正常管理保存仍未接

[2026-11-06 协议子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-login-wire.md) 已重新联网固定上述公开源码和许可，并按 wire 独立实现 `protocol::freebuff_login`，没有复制、安装或执行参考项目。创建/单步状态查询不继承业务认证，不跟 redirect 或自动 retry；复用 Gateway 统一代理入口。opaque expiry 原样 echo，相对 TTL、5 秒 pacing、401 Pending、send/read 共用预算、64 KiB 解码后边界、本地取消与单次领取均有合成 loopback 合同。

合法 2xx 不是免 schema 的成功；空/坏 response、错 attempt fingerprint 不能产生 Credentials。same-origin 浏览器链接和 fingerprint 等值是本地准入，不是官方已证保证。材料不可 Serialize/Clone，Debug/错误脱敏；不发明 agentId、token expiry 或 refresh。主配置 126 项和关闭模式 6 项按名去重 127 项 Rust 通过（15 新增），两种生产库检查与定向静态通过，扩大 Python 的既存扫描失败单列保留，不是全仓绿。

该协议子批结束时尚未有正常管理 HTTP/session owner、真实 provider/preset/revision 实查、Secret Grant、服务端 SecretPatch/CAS 或 UI；后续进展见 3.2/3.3。完整业务/agent/lease/权益继续，不因独立协议完成新增池、合池或标整卡 `offline_ready`；R06/W04、P50/P60/P70 保留。

### 3.2 正常管理会话与服务端保存

[FreeBuff 管理子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-management.md) 已接 create/status/poll/import/cancel、正常管理身份与 Secret Grant/Origin、真实 provider/preset/原 revision 及固定产品目标；具体接口见 [管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-login.md)。每实例 16 会话、600 秒 TTL，初始化/poll/save 为受跟踪任务，退出 join；20 秒保存预算包含 publication wait，HTTP waiter 丢失不丢保存，CAS/timeout/drop 不猜成功、不重复提交。

authToken 与私有 fingerprint receipt 只进入 credential 的普通 SecretPatch/CAS；receipt 独立脱敏、Keep/Replace/Clear，不继承 provider、不递归拼接、不进入业务 header/body。重授权保持原账号 ID/设备和模型/agent metadata，不推导 agentId/expiry/refresh；旧 generic adapter 保留，未支持的 folder CLI receipt 明确拒绝。

最终 190 个不同 Rust 测试通过（25 新增），含代表性关闭拒绝；两种生产库检查及 Rustfmt/checker31/ratchet4562/编码/来源通过，八阶段各 3060 项选定输入无漂移。保留实际红测、三处测试初始化编译失败及 v2 无最终回执，不报全仓绿。该批的前端缺口由 3.3 承接；真实管理 TCP/SQLite 重建、Server/PG/跨进程恢复与完整 agent/lease/业务仍缺，不新增整卡 `offline_ready`。

### 3.3 正常 UI 与稳定账号重新授权

[FreeBuff UI 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-ui.md) 已将同一 `preset: freebuff` 接入正常全局账号弹窗：默认浏览器批准、显式 poll/保存、稳定 ID 重授权和旧产品材料导入。前端不接收 authToken/fingerprint，不推导 agentId；同 Console/管理身份内未知保存关闭重开仍查原操作，不可切手工绕过或重复 import。原生 launcher 补精确 FreeBuff HTTPS origin 准入，其他外站不被放开。

新增前端 82/82、相邻组合 335/335（有重叠）、原生 8/8、类型/隔离正常 main 构建、1280/390 CSS 像素合成 API 浏览器检查及 Rustfmt/checker31/ratchet4570/来源/编码/diff 通过。真实 UI 发现的未知保存误导/重复状态经红测修补，失败记录保留。此结果不是 UI→真实管理 TCP/SQLite、跨页面/进程恢复、agent/lease/公共业务或真实平台；模板仍 16，R06/W04 与 P50/P60/P70 继续。

### 3.4 正常 UI 与真实本地管理重建

[FreeBuff 本地管理子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-local-ui.md) 新增 test-only 真实 TCP/Local host，正常 main→生产 build_app_state/router→普通 Grant/SecretPatch/CAS→持久 revision；动态合成 peer 使用服务器生成的设备，不新增生产 origin 配置或放宽 fingerprint 等值。浏览器明确检查/保存、UI 手动模型 Keep、稳定 ID/设备重授权和两次同 root/port runtime 重建通过；额外 account_name 由独立管理 TCP client 设置，不冒充 UI 字段。

最终相关 Rust157 passed/1 ignored，host随后显式1 passed，生产库检查通过，三个阶段各3214输入无漂移。正常main隔离构建895输入无漂移；上一批类型检查939输入实核一致后复用。Rustfmt/checker31/ratchet4575及来源/编码/diff通过，6项源码最大154有效行，所有本批服务正常退出。首轮Lib.exe构建失败、v2输入漂移及locator错误保留，不报全仓绿。

这不是独立进程/OS重启、跨进程receipt恢复、SQLite凭据表级审计、Server/PG/Worker、公共推理/agent/lease或真实平台。累计模板仍16，R06/W04和P50/P60/P70继续，不增整卡offline_ready。

### 3.5 业务 ownership 修复与当前产品合同差异

[业务子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-business-ownership.md) 先复现六项实际红测，再修稳定 ID 换 token/target 仍借旧 namespace、caller runtime metadata/instance 残留及非 object envelope 逃逸。相关 Rust163 passed/1 ignored，3217 项输入无漂移；两个真实 TCP 场景使用 legacy synthetic peer，不冒充 typed 登录公共调用。普通 custom metadata 保留，运行时字段独占是明确行为收紧；持久 schema/账号 ID/receipt 不变。

本次新联网固定业务来源为官方 `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`，同版本 LICENSE 的 Apache-2.0 及 blob SHA 已核；[业务参考](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/REFERENCE.md) 列实际读到的路径/类型和只参考协议、独立实现的采用方式。当前 admission 可扣 Freebucks、reuse 不买/不抢、目录 key/短效 handle/fetchId 与设备签名、业务部署 URL 都不是旧 session POST 或缺省 agent 可以代替。消费确认、当前 host 授权关系与完整 typed 业务仍未实现；旧登录固定参考不被新业务来源暗中替换。P50/P60/P70与整卡退出条件保留。

### 3.6 当前目录与设备协议子链，不自动改变登录目标

[catalog/device 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-catalog-device.md) 独立实现明确业务 origin/client、Bearer bootstrap catalog、有效目录后显式公钥注册和 signed catalog GET。固定参考的 smoke 配置明确业务 `www.codebuff.com` 与登录 `freebuff.com` 分开，CLI fallback 为 `codebuff.com`；没有正式发布 binary 的目标凭据，不自动将现有 typed token 跨 host 投递。管理登录及 Keep 原目标检查不变。

registration capability 绑定 token/origin/client/key，设备 key 显式导入 PKCS#8 DER 并比对 raw32 公钥，不新建磁盘 store、不每请求生成 key、不复用登录 fingerprint。短效 handle/fetchId 只内存，optional fetchId 兼容；未来 row 用 issuedAt，refreshAt 不当 expiry，locked 不准选择。签名 primitive 的 session/completion-shaped body 测试不是 production 业务签名接线。

正常管理 origin/client 选择与 catalog 发布、安装级持久秘密 owner、admission/reuse/逐次消费确认、agent/lease/probe/公共业务与跨进程仍未完成；无凭据回环、编译或目录 200 不代真实权益/费用/完整渠道验收。具体通过与失败检查只以子批最终回执为准，模板与整卡状态不因此增加。

## 4. 共用实施不变量

- 无凭据测试使用合成 client/token、随机回环 fake issuer/transport；不向公开 endpoint 发认证或生成请求。
- 发起、完成、取消都绑定管理身份、Secret Grant、provider/preset 和 revision；state 不能替代管理认证。
- 设备轮询/session status 要有 TTL、间隔、并发预算、取消和退出清理，不能每个账号启动无限循环。
- token/refreshed Cookie/client secret/fingerprint hash 进入既有秘密 owner，公开响应、草稿和错误不回显秘密。
- refresh 复用 single-flight、generation、轮换持久化、冲突保护和重启恢复；失败不自动换产品或账号。
- OAuth endpoint、业务 origin 与资产 origin 分别精确绑定；不跟随携密跨 origin 重定向。
- 外部来源在采用前登记版本、路径、许可、采用方式；开源代码许可不自动授予厂商服务或客户端身份使用权。
