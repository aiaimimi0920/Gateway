# Kiro AWS OIDC 协议来源与采用范围

2026-11-04 联网核实；Gateway 自行实现 JSON 协议，没有复制第三方源码。详细证据、固定许可来源与未证边界见 [调查合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-kiro-oauth-contract.md)。

## 官方合同

- [RegisterClient](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_RegisterClient.html)：注册自己的 public client，显式 scopes，独立 client secret expiry。
- [StartDeviceAuthorization](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_StartDeviceAuthorization.html)：JSON camelCase，明确 start URL、设备期限及用户码。
- [CreateToken](https://docs.aws.amazon.com/singlesignon/latest/OIDCAPIReference/API_CreateToken.html)：JSON device/refresh，标准错误串；不支持 idToken，不从 token 构造已验证身份。
- [AWS OIDC endpoints](https://docs.aws.amazon.com/general/latest/gr/sso.html)：商业分区 34 个 directory Region。本模块发标准 `oidc.{region}.amazonaws.com`；不据此宣称同 Region 已有 Kiro profile/模型。
- [AWS CLI SSO 配置](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-sso.html)：设备登录站点示例 `device.sso.us-west-2.amazonaws.com` 与 `device.sso.us-west-2.api.aws`。
- [RFC 8628 §3.2](https://www.rfc-editor.org/rfc/rfc8628#section-3.2)：设备 interval 缺省 5 秒；显式零不作缺省，slow_down 增加 5 秒且不延长 expiry。
- [RFC 6749 §5.1](https://www.rfc-editor.org/rfc/rfc6749#section-5.1)：access expires_in 缺省需要其他期限证据。AWS 没有已证默认值，因此保持 unknown，不补 3600 秒。

错误串另经官方 [botocore service model @ ddaf46e8bfed119924359798579bfdd343bf3739](https://github.com/boto/botocore/blob/ddaf46e8bfed119924359798579bfdd343bf3739/botocore/data/sso-oidc/2019-06-10/service-2.json) 核实：AuthorizationPendingException.error 为 authorization_pending、SlowDownException.error 为 slow_down，均 HTTP 400。不要求未证的 x-amzn-errortype，也不匹配 error_description。

## 固定历史实现的有限参考

[aws/amazon-q-developer-cli @ 15cc8f3cd18c4272925ce1c7053268eedff1ea0a](https://github.com/aws/amazon-q-developer-cli/tree/15cc8f3cd18c4272925ce1c7053268eedff1ea0a) 的 `crates/chat-cli/src/auth/consts.rs` / `builder_id.rs` 为 Builder ID start URL、directory Region、注册过期复用与 JSON SDK 调用提供行为参考。根许可 MIT OR Apache-2.0；没有采用其源代码。该仓明确当前 Kiro CLI 闭源，不能宣称这是当前产品最小 scope/profile 的完整合同。

设备 URL 准入是本地安全策略：允许绑定 directory Region 的 device.sso 两种官方格式，以及 Builder ID 固定 view.awsapps.com/start 站点；complete URL 还须与 verification URI 同 origin。对 Builder ID 站点的允许不等于已取得当前真实返回样本，既不将 startUrl 当作 verificationUri 的上游证明，也不跟随任意响应 endpoint。其他合法站点若后续有证据，必须显式扩充合同，不能静默放开任意域名。

本模块不提供 Console 登录、profile/subscription 发现、持久化 receipt 或独立 scheduler，不把 token 等同 Kiro 权益。后续接现有秘密、revision/CAS、durable grant 与共享刷新 owner。

## 2026-11-07：显式 profile 的 runtime 兼容调用

新增 `kiro_oauth_runtime` 是固定公开参考的兼容合同，不是官方通用模型 API。
原文固定至 [d-kuro/kirocc @ 7a2ba670286ca16ae89fe792ea935b3751179421](https://github.com/d-kuro/kirocc/tree/7a2ba670286ca16ae89fe792ea935b3751179421)，完整 Apache-2.0 许可及六份原文的 URL、获取时间与 SHA-256 保存在
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-oauth-runtime-20261107/public/sources.json`。
自行实现并复用 Gateway 的 CLI packer、EventStream decoder 与正常发送器；没有复制第三方源码或启动另一套转发服务。

- `internal/kiroclient/client.go` 的 runtime-root POST、AWS target 和 OAuth/Key 区分是本批采用范围：OAuth 不发送 `TokenType: API_KEY`。
- `internal/kiroproto/types.go` 的 `profileArn` 和 CLI conversation shape 作为 wire 依据；profile ARN 由操作员显式提供，不从 token/邮箱推导。
- `internal/auth/db.go` 和 `credentials_parser.go` 说明业务 Region 与 SSO Region 用途不同。本批不采用其缺省 US East、导入真实数据库或回退猜 profile。
- 不采用参考的自动重试/换 token、模型 reasoning 扩展或默认授权参数；Gateway 失败后禁止重新生成/换号回退。

当前仅允许 profile ARN 明确绑定 US East/FRA runtime 的本地安全策略；不是平台全部 Region、最小 scope、订阅权益或当前闭源 CLI 合同的证明。
旧 q/codewhisperer REST 与 CLI Key 配置不静默迁移。实际实现及验证见
[runtime 子批回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-kiro-oauth-runtime.md)。
