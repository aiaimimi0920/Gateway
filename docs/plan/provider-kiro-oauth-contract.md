# Kiro Builder ID / IAM Identity Center OAuth 调查合同

> 2026-11-04 联网调查与 P47–P49（含管理后端/UI）实施边界；属于 R02/W02 的依据，不是完整 OAuth 功能完成记录。
> [渠道身份表](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-channel-identity.md)和[总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)继续有效。本轮没有登录、读取凭据或调用真实模型；Social 不在本次调查范围。

## 1. 官方已证与仍未证的产品事实

[AWS 官方 scope 表](https://docs.aws.amazon.com/singlesignon/latest/userguide/customermanagedapps-saml2-oauth2.html)将以下 scope 明确关联到 Kiro，并列出 AWS Builder ID / IAM Identity Center 支持：

- `codewhisperer:analysis`：代码分析。
- `codewhisperer:completions`：inline suggestions。
- `codewhisperer:conversations`：chat。
- `codewhisperer:taskassist`：软件开发 Agent。
- `codewhisperer:transformations`：代码转换 Agent。

这不代表 Gateway 应默认申请全部五项，也不证明取得 token 后已有订阅、某个 profile 或具体模型权益。`sso:account:access` 仅对应 AWS 账号/permission-set 访问，不能替代上述 Kiro scope。

[Kiro IdC 指南](https://kiro.dev/docs/enterprise/identity-provider/iam-identity-center/)与[支持的 Region](https://kiro.dev/docs/enterprise/supported-regions/)明确区分：组织提供的 Start URL 和 OIDC Region 对应 Identity Center directory；profile Region 是 profile、数据及推理的位置，两者可以不同。不能把业务 base 的 Region 自动用于 directory 登录。[认证指南](https://kiro.dev/docs/getting-started/authentication/)还说明组织可针对 profile 禁用 device flow；OIDC 成功不是所有 profile 的准入证明。

## 2. 官方 OIDC JSON wire

所有下列请求使用 `Content-Type: application/json` 和 camelCase，不进入 Gateway 现有通用 form/snake_case refresh sender：

| 操作 | 路径 | 关键输入与约束 |
| --- | --- | --- |
| 注册 public client | `POST /client/register` | `clientName/clientType:"public"`；可声明 `scopes/grantTypes/issuerUrl/redirectUris/entitledApplicationArn` |
| 设备授权 | `POST /device_authorization` | `clientId/clientSecret/startUrl` |
| 设备换 token | `POST /token` | `clientId/clientSecret/grantType:"urn:ietf:params:oauth:grant-type:device_code"/deviceCode` |
| 刷新 | `POST /token` | `clientId/clientSecret/grantType:"refresh_token"/refreshToken` |

来源：[RegisterClient](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_RegisterClient.html)、[StartDeviceAuthorization](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_StartDeviceAuthorization.html)、[CreateToken](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_CreateToken.html)。

- 注册响应的 `clientIdIssuedAt/clientSecretExpiresAt` 是独立生命周期，不等于 access token expiry；返回的 authorization/token endpoint 不能未经受众校验就变成携密请求目标。
- device 响应的 `deviceCode/userCode/verificationUri/verificationUriComplete/expiresIn/interval` 需要有界保存、明确 TTL 和遵守 interval；处理 `authorization_pending/slow_down/expired_token/access_denied`，不能无界轮询。
- token 响应有 `accessToken/expiresIn/refreshToken/tokenType`，refresh 可缺。官方明确 token 请求中的 `scope` 参数无效果，实际 scope 来自注册，不能在 refresh 时凭空扩权。
- `idToken` 不受支持，不能伪造 OIDC 用户验签身份；`invalid_client/invalid_grant` 等确定性错误不进入通用无限重试。

[AWS General Reference](https://docs.aws.amazon.com/general/latest/gr/sso.html) 当前列出 34 个商业分区 OIDC directory Region，另列 GovCloud/FIPS/dualstack；这不是 Kiro 推理 Region 表。本批明确准入该 34 个商业 Region，发送固定标准 `https://oidc.{region}.amazonaws.com`，不开放任意 issuer、China/GovCloud/FIPS 或响应自报 endpoint。

设备错误串经官方 [botocore service model @ ddaf46e8bfed119924359798579bfdd343bf3739](https://github.com/boto/botocore/blob/ddaf46e8bfed119924359798579bfdd343bf3739/botocore/data/sso-oidc/2019-06-10/service-2.json) 核实：`AuthorizationPendingException.error` 为 `authorization_pending`（148 行）、`SlowDownException.error` 为 `slow_down`（609 行），均为 HTTP 400。不依赖未证的 `x-amzn-errortype`，不匹配任意 prose。

[RFC 8628 §3.2](https://www.rfc-editor.org/rfc/rfc8628#section-3.2) 规定缺失 `interval` 时使用 5 秒、设备 `expires_in` 必须提供；`slow_down` 每次增加 5 秒，不延长设备期限。access token 的 `expiresIn` 缺省则保持 unknown：[RFC 6749 §5.1](https://www.rfc-editor.org/rfc/rfc6749#section-5.1) 要求以其他证据确定缺省期限，当前 AWS 合同未给出可采用的默认 3600 秒。

[AWS CLI SSO 配置](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-sso.html) 给出 `https://device.sso.us-west-2.amazonaws.com/` 和 `https://device.sso.us-west-2.api.aws/` 示例。本地登录 URL 策略只允许绑定 directory Region 的这两种根路径格式，以及 Builder ID 固定 `view.awsapps.com/start`；complete URI 必须同 origin。**未取得当前 Builder ID 的完整真实 verification URI 样本**，允许固定站点并不把 start URL 当作其实际返回证明。

## 3. 固定许可参考与采用边界

### 3.1 官方历史客户端

仓库：[aws/amazon-q-developer-cli](https://github.com/aws/amazon-q-developer-cli/tree/15cc8f3cd18c4272925ce1c7053268eedff1ea0a)，固定 commit `15cc8f3cd18c4272925ce1c7053268eedff1ea0a`。已核对根 `Cargo.toml:12` 的 `MIT OR Apache-2.0` 声明及完整 [LICENSE.MIT](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/LICENSE.MIT) / [LICENSE.APACHE](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/LICENSE.APACHE)。

- [`crates/chat-cli/src/auth/consts.rs:5–28`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/chat-cli/src/auth/consts.rs#L5-L28)：Builder ID 的 `us-east-1`、`https://view.awsapps.com/start`，以及前三个 scope；`taskassist/transformations` 为注释，不请求 `sso:account:access`。
- [`builder_id.rs`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/chat-cli/src/auth/builder_id.rs)：87–97 按 directory Region 构造 `https://oidc.{region}.amazonaws.com`；138–154、175–203 核对注册 expiry/Region/flow/scopes 后才复用；241–271 使用组织 Start URL/Region 或 Builder ID 值；390–424 刷新使用原注册/Region/refresh 并保留上下文；552–563 为设备换 token 调用。

该固定版本的 [`amzn-codewhisperer-client/operation/list_available_profiles.rs`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/amzn-codewhisperer-client/src/operation/list_available_profiles.rs)（202、213、218–224 行）给出 `POST /`、`application/x-amz-json-1.0` 与 `x-amz-target: AmazonCodeWhispererService.ListAvailableProfiles`，subscriptions 对应 `AmazonCodeWhispererService.ListAvailableSubscriptions`。[`api_client/profile.rs:20–27`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/chat-cli/src/api_client/profile.rs#L20-L27) 遍历旧客户端的 endpoints 后合并 profiles；[`api_client/mod.rs:415–419`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/chat-cli/src/api_client/mod.rs#L415-L419) 的 generate 使用可选 `profileArn`，SDK 只在 `Some` 时序列化。已核对相应普通及 streaming client 的 Apache-2.0 许可。以上是固定旧客户端的有限 wire 证据，**不是当前服务端一定接受 Builder ID 无 profileArn 的证明**，也不是已经实现 profile/subscription 发现。

同一固定 SDK 的 profile 输入包含 `max_results/next_token`，输出包含 `profiles/nextToken`；[`types/_profile.rs`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/amzn-codewhisperer-client/src/types/_profile.rs) 包含必填 ARN 与 profile name。旧 [`api_client/endpoints.rs`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/chat-cli/src/api_client/endpoints.rs) 从 ARN 的 Region 段选择 US East/FRA 两个旧 Q 业务端点，不使用 directory Region 代替。

**订阅目录不等于账号权益。** 旧 `ListAvailableSubscriptions` 输入为空，输出为 `subscriptionPlans` 和 disclaimer；[`types/_subscription_plan.rs`](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/crates/amzn-codewhisperer-client/src/types/_subscription_plan.rs) 主要描述 name、pricing、description 与 subscription type。这些字段不能证明当前账号已订阅或有某模型额度。旧 `GetUsageLimits` 的 `subscription_info` 是另一个结构；当前合同与权限仍未核实，不据旧方法名实现假的“订阅验证”。

**限制**：该固定版本的 [README.md:4](https://github.com/aws/amazon-q-developer-cli/blob/15cc8f3cd18c4272925ce1c7053268eedff1ea0a/README.md#L4) 明确当前 Kiro CLI 是闭源产品，旧仓仅维护关键安全修复。以上三 scope 与 Builder ID 默认值是历史固定实现证据，不能称为当前闭源 Kiro CLI 的最新精确参数，也不授权借用其他产品的 clientId。

### 3.2 非官方 Kiro 参考

[nopperabbo/kiroxy](https://github.com/nopperabbo/kiroxy/tree/2ac2ea9232634d718f5a99aad88036f78e75aebf)，固定 commit `2ac2ea9232634d718f5a99aad88036f78e75aebf`，已核对完整 [MIT LICENSE](https://github.com/nopperabbo/kiroxy/blob/2ac2ea9232634d718f5a99aad88036f78e75aebf/LICENSE)。

[`internal/builderid/builderid.go`](https://github.com/nopperabbo/kiroxy/blob/2ac2ea9232634d718f5a99aad88036f78e75aebf/internal/builderid/builderid.go)自行注册 `clientName:"kiroxy"`，请求五项 scope，使用 public Builder ID Start URL 和 JSON wire。它不是官方合同；不照搬默认补 expiry/interval、无界 `io.ReadAll` 或错误中输出原始 body。文件声明派生自 `Quorinex/Kiro-Go @ 940dc782`，本轮未独立核对该派生源，不将短 SHA 算作已审查参考。

本轮没有复制第三方源码；若后续实际采用，仍需在相应 source owner 保存精确参考、采用范围及完整许可。

## 4. P47 已实现的协议子链与下一批边界

本批 [kiro_oauth.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_oauth.rs) 自行实现 public registration、单步 device grant 与一次性 JSON refresh 协议；来源采用范围在 [REFERENCE.md](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_oauth/REFERENCE.md)。调用方显式选 scope；注册 secret 有独立绝对/单调期限；设备 TTL 从请求开始计、最多 600 秒且受注册期限约束。发前 pacing、pending/slow_down 状态与取消均由可变 capability 拥有，不创建后台轮询。发送后取消或不明确失败不再复用同设备 capability；refresh grant 按值消费，无自动重试。

HTTP 发送目标固定，禁止重定向及继承默认认证头，send+read 共用最多 15 秒预算，解码后响应最多 64 KiB；重复安全字段由直接 typed serde 解码拒绝，错误不回显任意 body。token 只表示 opaque 材料，access expiry 缺省保持 unknown。网络由既有 Kiro line feature 控制，关闭时拒绝发送，纯解析仍可用。

现有 [业务 planner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/kiro.rs) 不等于 OAuth 接线完成。P47 当时尚无 registered secret、directory/认证方法/scopes/注册 expiry 的 typed durable 身份，因此先在 [grant.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/grant.rs) 对 Kiro adapter 或 AWS OIDC endpoint **在 reserve 前**返回 recoverable `kiro_oauth_refresh_context_required`，阻止旧通用 form 误发送；该 gate 当时不是成功刷新。

P47 交给下一批的接线要求现已由下面的 P48 子链承接；P47 原定向验证与历史边界见 [P47 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p47.md)。

发布默认授权参数/产品模板或标记完整产品登录可用之前，仍须核实并记录：当前 Kiro 各渠道所需最小 scope、profile 发现/选择与订阅检查的 exact API、profile ARN 与业务 Region 的绑定、第三方客户端使用边界。后续 UI 只允许操作员显式选择现有协议参数，不提供已证默认 scope 或权益保证。不能只凭通用 OIDC 或旧 Q CLI 测试通过就把 R02/W02 标为 `offline_ready`，也不要求现在录制真实凭据。

## 5. P48 typed 授权与既有持久刷新子链

credential-only optional `kiro_oauth` 保存完整注册上下文和固定 machine ID；只有 client secret 是 nested secret，沿原 redaction/SecretPatch/Keep/clear 保存，不把整个 context 当 Grok 的非秘密 metadata 直接展示。过期注册可读，但只从原绝对注册 expiry 恢复剩余 deadline，过期不发送；清空 secret 后保留 metadata 不会生成 generic refresh owner。

完整 context 进入 material identity 和授权比较，而 consumable RT family 不增加可编辑 metadata。refresh 不允许更换 context；原 prepare→reserve→execute→Result→authority→cache/restart 负责实际刷新与重放，缺 access expiry 不补默认寿命。typed 路径拒绝 legacy auth aliases、header 覆盖、provider access 继承、external keepalive 与非 direct 执行，投影固定 machine ID；业务最终发送禁止 Kiro redirect。旧无 typed context 的材料不被静默迁移，仍保留 reserve 前的 recoverable 拒绝。

主配置445个不同Rust测试（31新增）、代表性feature-off的15项及最终静态/45项Rust来源核对均已通过，11 ignored未计入通过；失败归因和复验边界见 [P48 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p48.md)。P48 当时尚缺的 folder 转换由下节 P49 承接；旧 route/未变 source hash 的 DB 升级、普通 Console 登录/receipt/poll/cancel/UI、profile/subscription 发现和真实产品/额度验收仍未完成，不将当前配置与刷新子链当整卡登录完成。

## 6. P49 folder 材料转换合同（本子批定向离线验证通过）

专用转换进入已有 `kiro-compatible` folder 分支，只处理操作员显式导入的材料，不重写用户 JSON、现有数据库或未变 source hash 的历史行。当前可接受的认证方式是本地 typed enum 的 `builder_id/identity_center`，不是对任意厂商导出格式的自动识别承诺。完整 context 沿用第 5 节；旧字段转换必须显式给出相同的全部注册 metadata，不从旧参考默认值补缺。

根对象和 `extraBody/extra_body` 的 key 比较忽略 ASCII 大小写、`_` 与 `-`；支持 `api_key/apiKey/accessToken/kiroAccessToken`、`refresh_token/refreshToken/kiroRefreshToken`，以及注册 `authMethod/kiroAuthMethod`、`directoryRegion/authRegion/kiroAuthRegion`、`startUrl/kiroStartUrl`、`clientId/kiroClientId`、`clientSecret/kiroClientSecret`、`clientSecretExpiresAt/kiroClientSecretExpiresAt`、`scopes/kiroScopes`。多个来源同值可折叠，冲突或错误类型拒绝；不接受字符串注册 expiry、未知 scope、Social 伪装或任意 OAuth endpoint。对原始同名重复 JSON key 的处理仍属于既有文件 JSON loader，本合同不把 Value 阶段的 alias 检查冒称原始字节级重复键检测。

machine 优先保留显式值（历史 UUID/32 位 hex 按既有算法转成 64 位小写 hex），否则保留 provider 层实际消费的显式 machine，再使用原 extra RT 的既有 hash；account 外层只读取 runtime 真正接受的精确 `extra_body/extraBody`，不激活此前忽略的其他 spellings。只有根 RT 时原 headers owner 没有消费它，固定旧全零值，不因一次转换建立新设备。后续导入已有 context 时保留原 machine，不随 RT 轮换重算。无效或互相冲突的显式值拒绝。

独立 bearer-only 材料不制造注册 context；无 access 但具有 RT 和完整含 secret 的 context 可以保留为等待刷新材料，不能继承 provider 的其他 access。access expiry 只接受绝对 RFC3339，缺省保持 unknown。根 `rawSource` 归档不作为第二个认证来源并不写回；业务 extra 内嵌注册/归档副本拒绝。

选中 account 的产品身份、DirectHttp、实际 DB merge/call-flow prepare 后的业务目的地沿 P48 边界验证。folder 输出适配该 account 既有 DB field spelling，不宣称与 route YAML 无条件互换；不意味着 hosted DB 已自动获得 route-document scheduler 刷新。最终测试及剩余兼容工作以 [P49 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49.md)为准。

P49 最终 v5 主配置449个不同Rust测试（21新增）、代表性关闭35项及静态/13项Rust来源冻结通过；另19 ignored不计通过。v4 暴露的关闭模式纯材料符号缺失已以共用 `material_metadata` 修复，关闭模式继续拒绝 OAuth 网络和 packing；不把算法可读性恢复当产品线已开启。

## 7. 2026-11-04 当前官方资料增量核实

以下为公开只读联网核实，不是新增实现或真实账号验证：

- [ACP integrations](https://kiro.dev/docs/acp/) 明确支持第三方编辑器和自定义 ACP client，但要求协商能力。ACP 对 agent harness 的支持不能外推为第三方 public OIDC client 直接调用现有 HTTP 数据面的完整合同。
- [Device flow control by profile](https://kiro.dev/docs/enterprise/governance/sign-in/#device-flow-control-by-profile) 明确 profile 管理员可以禁用设备流；没有可用 profile 时登录会被拒绝，拒绝切换时保留当前 profile。当前 profile HTTP 响应里表达该策略的 exact 字段尚未取得，不自行命名或绕过它。
- [官方防火墙表](https://kiro.dev/docs/privacy-and-security/firewalls/) 同时列出两地 `q.*.amazonaws.com`、`runtime.*.kiro.dev` 与 `management.*.kiro.dev`，并将 Q 域名注明为尚需保留的 legacy。域名存在和用途说明不证明 operation path、method、headers 或 JSON；本批不据此扩大业务 allowlist。
- [API keys 治理](https://kiro.dev/docs/enterprise/governance/api-keys/) 明确存在 portal 签发、供 Kiro CLI 自动化使用的用户 Key；企业默认关闭生成，由管理员显式启用。它是需要单独处理的原生 Key 候选，不能宣称 Kiro 只有 OAuth，也不能把 Key 贴到当前 bearer/OIDC 入口或直接新增通用模型 API 模板。

### 7.1 原生 API Key：订阅型 CLI 入口已证，官方 direct HTTP 合同未公开

[当前认证文档](https://kiro.dev/docs/getting-started/authentication/#api-key-authentication-cli) 明确 API Key 入口仅列于 CLI；Pro/Pro+/Pro Max/Power 用户在 `app.kiro.dev` 的 API Keys 中创建，再通过 `KIRO_API_KEY` 供 `kiro-cli chat --no-interactive` 使用。官方示例前缀为 `ksk_`，不据示例发明完整 Key 校验规则。

同页明确 Key 消耗从该用户的 subscription credits 扣除。因此 Key 与同一订阅的浏览器登录不是已经证明独立的余额桶；仅在同一用户、同一订阅的 credits 维度记为 `shared`，不外推不同 profile、组织或其他登录身份，也不据此认定速率、模型资格和其他限额相同。[Headless 文档](https://kiro.dev/docs/cli/headless/) 明确提供 `--model`、`stream-json` 等 CLI 契约，但没有公开 Key direct HTTP 的 method/path/schema，不能用 CLI 参数冒充 Gateway 已实现相同 HTTP 请求。

官方说明已有 browser session 优先于环境 Key。未来如选择 CLI helper 方案，必须用 Gateway 自有隔离运行环境并验证精确 Key owner，不能沿用户已有 CLI 会话借号；不读取宿主 profile、不自动退出或修改既有会话。当前不安装或运行 CLI，不引入第二套服务、helper 或新模板。

`kiro-api` 是当前默认provider展示名，模板/preset采用 `kiro-api-key`（第 10–11 节），不把 CLI 订阅入口称为官方通用模型 API。固定第三方 direct HTTP 依据及许可见下节；API Key 的 ACP 支持不能从普通 headless 契约推导。当前 Builder ID verification URI 完整真实公开样本、当前 profile/subscription HTTP service-model、逐 API 最小 scope，以及其他身份和 Web 的额度关系仍未证；这些缺口不要求用户现在录制，也不以 stub 或旧客户端参数冒充完成。

### 7.2 Key direct HTTP 的固定开源线索（调查时未采用；当前采用见第 10 节）

本轮另固定两份有完整许可证的第三方实现；没有 clone、安装、执行或复制代码。主线程抽查了第一份实际发送 owner 和完整许可证，不把“官方未给 HTTP 文档”写成“不可实施”：后续可以在明确逆向兼容边界下评估采用，但不得虚构官方支持或已取得账号权益。

| 固定参考 | 许可与实际可支持的线索 | 不采用/待证之处 |
| --- | --- | --- |
| [d-kuro/kirocc @ 7a2ba670286ca16ae89fe792ea935b3751179421](https://github.com/d-kuro/kirocc/tree/7a2ba670286ca16ae89fe792ea935b3751179421)，[client.go](https://github.com/d-kuro/kirocc/blob/7a2ba670286ca16ae89fe792ea935b3751179421/internal/kiroclient/client.go)、[auth/refresh.go](https://github.com/d-kuro/kirocc/blob/7a2ba670286ca16ae89fe792ea935b3751179421/internal/auth/refresh.go) | [Apache-2.0](https://github.com/d-kuro/kirocc/blob/7a2ba670286ca16ae89fe792ea935b3751179421/LICENSE)。Key 直接作 bearer，带 `TokenType: API_KEY`，POST 到 `https://runtime.{region}.kiro.dev/`；使用 AWS JSON target 和 EventStream，不调用 CLI shell。Key 不进行 OAuth refresh | 模拟旧 CLI Rust SDK UA；其 [catalog client](https://github.com/d-kuro/kirocc/blob/7a2ba670286ca16ae89fe792ea935b3751179421/internal/kirocatalog/client.go) 明确 Key 无 profile ARN，未支持相同模型发现。不能照搬网络失败自动重提/403“刷新”静态 Key、任意日志正文或内建模型权益 |
| [Quorinex/Kiro-Go @ f8f6071c9298a4266ad3e0c7e483d4a2510cbcaf](https://github.com/Quorinex/Kiro-Go/tree/f8f6071c9298a4266ad3e0c7e483d4a2510cbcaf)，[kiro.go](https://github.com/Quorinex/Kiro-Go/blob/f8f6071c9298a4266ad3e0c7e483d4a2510cbcaf/proxy/kiro.go)、[kiro_headers.go](https://github.com/Quorinex/Kiro-Go/blob/f8f6071c9298a4266ad3e0c7e483d4a2510cbcaf/proxy/kiro_headers.go) | [MIT](https://github.com/Quorinex/Kiro-Go/blob/f8f6071c9298a4266ad3e0c7e483d4a2510cbcaf/LICENSE)。核心 Key/bearer/TokenType/runtime-root 合同相同，Key 不走 OAuth 刷新 | 使用 JS/IDE UA，与第一份不同；其无 profile 的 REST 目录与第一份限制相反。没有 live 证据裁定目录可用性，不把兼容别名模型表作为实际能力 |

这两份参考支持“独立 Key 材料/固定发送目标/最小请求与 EventStream owner”的设计和合成负例，但不证明当前所有 Region、模型、profile、字段扩展或目录都可用。第 10 节已采用第一份有限 wire，范围及完整许可保留在生产 owner；第二份的目录假设未采用。第 11 节另接正常Key模板；不提供generic Key→OAuth导入、helper或真实调用证明。

## 8. P49 后续：管理设备授权与 UI（定向离线验证通过）

[后端回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-console.md)记录每 runtime 有界 registry、public registration、显式单步 device poll、取消/TTL、
server-held token 和原 SecretPatch/CAS 保存。20 秒 accepted save 不依赖 HTTP waiter，未知保存不自动重投；
管理 token/Host/IP/Origin/Secret Grant 与上游 device/user code 分离。新账号显式 machine，首批稳定 ID 重授权只接已有 typed context，
保留旧 machine，缺新 refresh/expiry 时明确清旧值。旧无 typed route/DB 不静默构造 context 或建立新设备。

主配置去重143项通过（20新增），另1个合成前端 fixture opt-in ignored；代表性关闭5项通过，1个新名称仅在关闭配置。
原编译中断、Serde unit variant strictness 与关闭 preset 的夹具失败均保留。20项 Rust 来源核对、Rustfmt/ratchet/diff通过。

[UI 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-ui.md)记录真实 GlobalDialogs/CredentialDialog 接线、产品 API schema、显式 directory/scopes/new machine、稳定 ID 重新授权。
状态 GET 不推进授权；POST poll 和 import 分别由明确操作触发，ready 不自动保存，unknown/save_unconfirmed 锁重投，当前 Console 挂载期 Map 保留 receipt。
26文件228项前端测试（65新增）、类型、隔离 Web 构建和1280/390 CSS像素合成浏览器通过，18项前端来源核对通过。
没有新模板、跨应用重启 receipt 持久化、旧 route/DB 升级、profile/subscription 发现或真实账号/额度验证，R02/W02仍未完整完成。

## 9. P49 后续：旧 route 账号显式重新授权（定向离线验证通过）

[本批回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-legacy-reauth.md)
扩展第 8 节首批的 typed-only 重授权限制：无 context 的既有 credential 也能从原入口显式重新授权，
但必须重新 public registration，directory/scopes 仍由操作员指定，不从旧字段补注册或权限。
新授权固定真实编译后 payload 的旧机器身份，包括显式 UUID/hex、extra RT 派生与根 RT-only 的旧全零行为；
歧义设备、shared provider 认证及错误业务 Region 在网络前拒绝。仅目标账号的旧认证副本与嵌套归档秘密
通过原 SecretPatch 清理；不放宽通用 Keep 受众或清理 sibling，原 revision/CAS 和未知保存语义不变。

最终主配置去重148项通过（5新增），另1个合成 fixture ignored；代表性关闭5项通过。
归档 bootstrap 的夹具失败及修复保留，6项 Rust 来源、Rustfmt/checker/ratchet/diff通过；UI源码未变。
这是用户明确授权后的材料替换，不是旧 route/未变 source hash DB 的自动升级，也不证明当前最小 scope、
profile/subscription、客户端资格或额度。第 7 节原生 Key、第 8 节跨应用 receipt 与其他未完范围继续。

## 10. P49 后续：原生 Key 的后端 wire 与静态材料边界

后端 preset 为 `kiro-api-key`，auth mode 为 `kiro_api_key`，复用 `kiro_compatible` executor 与既有有界 EventStream decoder，
不另建客户端进程、服务或解析器。采用范围和完整 Apache-2.0 许可见
[生产 owner 参考声明](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_api_key/REFERENCE.md)。
该实现是固定开源参考的逆向兼容，不冒称官方 direct HTTP API。

- 首批仅准入 `https://runtime.us-east-1.kiro.dev/` 根 POST；固定 bearer、`TokenType: API_KEY`、AWS JSON target、CLI 2.10.0 SDK UA。
  不继承旧 IDE 的 machine、profileArn、client 注册或 refresh；不开放全部商业 Region。
- 模型必须显式给出并保留原 ID，无默认模型、别名映射或猜测目录；未知扩展、reasoning、强制工具选择和 server-side tools 明确拒绝。
  `max_tokens/max_completion_tokens` 继续兼容既有 Kiro 输入但不映射为未证的 CLI wire 控制，不宣称上游输出预算保障。
- 原 preset 在合并前锚定；拒绝 OAuth context/refresh/discovery、session、浏览器执行、外部 keepalive、替代路径、认证/自定义 header 与 body 覆盖。
  最终发送重新验证 plan 并由 Key owner 设置完整 headers，禁止重定向，不做 403 刷新或隐式重提。
- 合成请求覆盖 text、system/history、function tools、data-image 的 packing 与三类文本响应编码；不因此宣称真实模型具有视觉/工具权益。
  非 EventStream 的 HTTP 200 明确失败，不将错误 JSON 伪装成生成成功。token usage 复用旧 decoder 的估算语义，不是 subscription credits 或实际扣费证据。
- 正常管理 Secret Grant 下的显式模型测试使用精确账户和普通 sender；无显式测试不生成，模型/禁用/权限失败不换账号，返回及审计沿既有保护且 Key 不回显。

定向验证回执位于
[Key 后端回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-key-backend.md)。
主配置按名称去重250项通过（包含14新增），代表性关闭2项通过；共享packing/decoder另行补测，
Rustfmt/checker31项/ratchet/diff和33项Rust来源冻结通过，不把不同配置与重复绿测叠加。
后端子批当时尚缺的正常模板/Key编辑/Keep由下节承接；folder、公共完整pipeline与devtest仍未完成，
不将 R02/W02、P50/P60/P70 或任一真实账号/额度验收标为完成。

## 11. P49 后续：Key 正常 Console 与材料暂存

[Console回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-key-console.md)
记录模板 `kiro-api-key` / 默认展示 `kiro-api`、固定runtime空池、精确手工模型与普通Key新增/Keep/replace/clear。
不加OAuth按钮或猜测目录；API分类按精确preset先于共享executor/profile。自动测试关闭，all仅指显式声明模型。

新材料绑定稳定provider/credential ID、产品、runtime与auth mode，最终SecretPatch重新校验；metadata-only Keep
保留尚未autosave的replacement及原绑定，clear覆盖旧replacement。后端复用完整destination projection，按preset、
有效mode/runtime识别Key owner，禁止身份擦除/进入、改变目标及新增继承消费者。正常管理Grant/Origin/CAS与脱敏不变。

定向UI25项通过，扩大277 passed/3 failed/1 skipped，同3失败用写前源码overlay复现；Rust去重124 passed/2 ignored
（9新增），类型/隔离构建/1280及390 CSS像素合成浏览器、Rustfmt/ratchet及13项源码冻结通过。不是全仓绿、
真实后端UI E2E、Redis或OS重启证明。累计新增模板13，不增整卡offline_ready。
下一子批先补Key专用folder分类/归一化与错误池，再补完整公共pipeline及正常UI服务/重启闭环；旧route/DB升级、
当前最小scope/profile/订阅/ARN/业务Region、跨应用receipt、真实资格/模型/额度及原计划其他渠道继续。

## 12. P49 后续：Key folder 产品隔离与材料转换

[folder回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-p49-key-folder.md)
记录独立Key account的family/surface/material分类、严格静态材料转换、实际DB merge与最终Key/call-flow核验。
明确Key字段和单层extra声明在generic/旧OAuth入口拒绝；不根据opaque Key前缀猜产品、不扫描归档。
export逐项写前验证，跨产品credential override不重新贴标或默默转换。旧无Key claim的路径不改。

最终主配置去重303 passed/14 ignored（13新增），代表性关闭12 passed；关闭transport时import/export明确拒绝，
不是OAuth pure parser的关闭语义。Rustfmt/checker31项/ratchet及8项源码冻结通过，前批13项UI/材料与33项后端来源保持。
真实5项红测、v1绿测后独立审查发现的声明漏口及v2修复、ratchet输出路径参数失败均在回执保留。

该子批当时map只取得account inline base，object-storage发现由下节承接；跨产品混合account仍不支持。
纯材料/路径/merge测试不是实际PG upsert、cache同步、OS重启或完整UI→服务→公共调用验收。
累计模板仍13；完整公共pipeline、旧route/未变source hash DB升级、scope/profile/订阅与跨应用receipt及原计划其他渠道继续。

## 13. P49 后续：object-storage account 的 Key 产品发现

[对象发现回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-p49-key-object-discovery.md)
记录实际import在source-hash skip/选择前按稳定Kiro身份懒hydrate缺base候选，再复用原索引与排序；非相关材料/仅label不触发。
复用原对象getter及128项/64MiB cache，单次发现共享30秒async I/O/检查预算，新base累计32MiB；不另建读取器/数据库字段。
错误不当未匹配、不降级旧OAuth map，读完/map重建后拒绝过期结果；同步parse不能抢占、不是整scan硬截止。
v1审查发现legacy `kiro` surface漏触发，v2已补两段路径与payload override回归；旧alias/source-hash skip规则本身不变。

最终folder185 passed/14 ignored、既有对象读取29 passed，主配置去重214 passed/14 ignored（14新增）；Rustfmt/checker31项/ratchet/diff、
6项源码冻结及前批8项folder来源保持通过。真实PG→对象→upsert/cache同步、完整公共pipeline及正常UI服务/重启仍未执行。
不叠加旧批通过数，不增加整卡offline_ready；公共三入口JSON/SSE、旧route/DB升级、当前scope/profile/订阅与跨应用receipt及其他渠道继续。

## 14. P49 后续：Key 公共调用链与输入/终态完整性

[公共调用链回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-p49-key-public-pipeline.md)
记录真实公共router、认证、normalizer、路由/精确Key/模型映射、普通sender与SQLite finalize的Chat/Messages/Responses JSON/SSE合成验收。
Key传输/HTTP错误固定不可重试与Abort，不读任意HTTP错误正文；帧错误使用固定诊断的domain Err，避免Responses在错误后合成completed。

Key buffered/SSE复用原parser的严格模式：已知事件坏JSON/字段、空输出、工具identity变化/stop后追加、未结束或非object参数明确失败。
工具完整性状态上限4096项/64MiB retained估算；原OAuth默认parser保持宽松。发送前拒绝尾Assistant、坏历史/不配对工具、
remote图片及未证控制，历史工具必须显式声明；Messages合法base64转data URI，Responses标准input_image转ImageUrl，扩展/file_id继续Raw。
这些是本地产品合同，不是已证当前CLI全部wire、视觉/工具资格或上游输出预算保证；usage仍为旧decoder估算，不是实际credits。

主运行组合按完整名称去重200 passed/0 failed/0 ignored（16新增）；公共组10项包含在sender20项中，不叠加。
Anthropic旧测试锁定空URL的首轮失败已保留，仅修断言后58项整组通过；其他过程失败及24项来源、有效行、静态门禁见回执。
代表性Kiro关闭编译及24项来源/静态核对通过；未运行关闭组合的测试、management-ui全仓回归、真实平台或凭据调用。
当时待补的正常UI→真实管理服务→保存→公共调用/runtime重建现由第15节承接；真实PG→对象→upsert/cache、旧route/DB自动升级、当前scope/profile/订阅与跨应用receipt仍未闭合。
累计模板13，不新增整卡offline_ready；其他渠道及P50/P60/P70仍在原范围。

## 15. P49 后续：Key 正常 UI、真实本地 HTTP 与持久化重建

[本地UI回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-p49-key-local-ui.md)
记录正常main入口与透明loopback proxy连接生产`build_app_state`、Local配置事务、真实TCP router和SQLite，不使用FakeRedisBackend或保存mock。
产品目的地与headers按生产规则验证，仅最终transport沿既有`cfg(test)`接缝转到不转发的合成上游；普通EXE不增加任意目标override。
管理登录使用正常session/secret确认流程，不注入grant、不改Host/Origin/auth/body，也不读取真实用户材料。

正常页面完成空池模板、Key与精确模型保存、metadata Keep和显式清空；JSON/SSE走正常公共access key，消息余额4→3→2。
同目录、同端口两次重建分别断言Key存在/为空，revision/credential ID/模型保留；清空后400、余额仍2，上游严格只收到两次精确Key/模型/CLI wire请求。
自动TCP测试另覆盖旧revision 409、保持Host/Origin不变的旧grant 403和新grant clear；页面reload则沿现有登录流程重新确认权限。

自动TCP1项、显式ignored opt-in host1项和公共回归10项，按名称去重12 passed/0 failed/0 ignored（2新增）；不是默认跳过fixture也不是前批200项全重跑。
Rustfmt四owner、checker31项、ratchet及27项Rust来源核对通过；各owner最多473有效行。过程filter零匹配与临时验证命令失败保留，未改生产语义来测绿。
HTTP/SQLite/FreeBuff和合成上游关闭成功后才写完成receipt；浏览器、专属Node进程及两个端口均已关闭，没有新Redis容器或包。

这是同进程重新bootstrap AppState/runtime，未启动通用background scheduler；不是原生EXE/OS重启、真实PG同步、真实账号资格或credits验收。
旧route/未变source hash DB升级、当前最小scope/profile/订阅/ARN/业务Region、跨应用receipt和其他渠道继续；累计模板13，不标R02/W02、P50/P60/P70或总目标完成。

## 16. 2026-11-07：显式 profile 的 OAuth runtime 兼容子链

[runtime 子批回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-kiro-oauth-runtime.md)
记录新 `kiro-oauth` 空池模板与独立 `kiro_oauth_runtime` mode。固定 Apache-2.0 公开参考的 runtime-root/AWS target/显式 profile 不与旧 REST 路径混同；不发送 Key 专属 `TokenType`，不默认 scope、profile 或业务 Region。
注册 context 新增可选 runtime target，正常授权保存、Keep、持久刷新/重建、缓存目标与精确模型 probe 沿用原 owner；旧 context 缺字段的 wire identity 保持。

这个限定子链不补造profile/subscription发现或当前官方完整数据面合同。该批当时缺的联合公共调用/结算与SSE本地取消由下节承接；真实客户端资格/权益、最小scope、旧route/DB自动升级、完整业务/全部取消阶段、跨应用恢复与原生包仍未完成，不将R02/W02标整卡offline_ready。

## 17. 2026-11-07：正常 UI、真实公共 TCP 与 Local 重建

[本地联合回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-kiro-oauth-local.md)
记录正常管理设备流/保存、三公共协议JSON/SSE、精确credential/alias→model/profile、真实SQLite消息结算和两次同目录同端口重建；metadata Keep保三秘密，旧Grant重建失效，新Grant clear后材料不复活。
401/302/native错误均failed、退款且一次发送，三协议实际TCP SSE Drop记cancelled、body释放、健康不中伤；不是供应商撤销或退还credits。
正常main页面另完成显式profile模板/授权保存/手工模型，普通公共Key的Chat JSON/Responses SSE余额4→3→2；clear由正常管理API执行，清后重建400、余额仍2、业务上游严格两次，不能说所有操作都由UI完成。

最终Kiro库206 passed/2 ignored与显式OAuth浏览器host1 passed按名去重207 passed，旧Key host本批仍未跑；四项新增。生产lib/main隔离构建/Rustfmt/checker31/ratchet4789、八源码最大499有效行及终态来源核对通过，具体失败分类与边界见回执。
只验证同进程重新bootstrap、Local及合成OIDC/业务peer；没有通用background scheduler、Server/PG/Redis、独立原生进程或真实平台验收。完整decoder/参数/全部取消阶段、当前scope/profile发现/订阅/客户端资格、旧材料自动兼容和跨应用receipt/整包继续，模板仍17，不标整卡offline_ready或P50/P60/P70完成。

## 18. 2026-11-07：严格 runtime 事件完整性与空工具参数

[完整性回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-kiro-runtime-integrity.md)
限定 CLI Key 与 OAuth runtime 共享的严格 parser：header 进入 map 前拒绝歧义/非法 UTF-8，明确 envelope；已知 payload 直接 typed serde，拒绝重复关键字段、错误类型、null 与非 object，但保留合法续帧省略字段和未知事件。旧 REST 的 default parser 不改变。
空/全空白工具参数直到 stop 才输出 `{}`，非空参数/JSON 字符串内部空白保留；按原 delta 预算与 per-delta 扫描，不引入累计 trim 的 O(n²) 或平行状态 owner。
三协议 JSON/SSE 真实本地 TCP 回归核错误退款/无成功终态/one-send，交错工具参数核 delta/done/completed 等适用公共合同；最终测试、生产/代表性关闭编译和来源审计见回执。这里不补造当前官方 wire/资格、全部 decoder/取消阶段或真实 credits 证明；第17节未完范围与P50/P60/P70继续，不新增模板或整卡offline_ready。
