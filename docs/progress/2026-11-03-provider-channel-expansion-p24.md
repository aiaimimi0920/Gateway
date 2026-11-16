# 服务商分渠道扩展：第 24 批，Grok OIDC 验签与身份约束

> 2026-11-03（Asia/Shanghai）；R05 / W03 / P20。实现与85项定向离线验证已收尾，代表性产品feature关闭编译通过；不等于完整 OAuth 登录或整个渠道 offline_ready。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或接管运行实例。

## 1. 来源与范围

延续 [P23](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p23.md) 的固定 issuer、显式客户端与三类 grant，本批补有 ID token 的 code-flow end-user 身份验证，不把 transport 的 UnverifiedTokens 直接转成可信身份。

无凭据只读 HTTPS GET 再次核实 [discovery](https://auth.x.ai/.well-known/openid-configuration)、[JWKS](https://auth.x.ai/.well-known/jwks.json) 和 [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html)，均 HTTP200。固定协议参考仍为官方 Grok Build `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`，Apache-2.0；未复制第三方源码、安装或执行参考项目。精确 URL、公共响应与 SHA-256 保存在 [P24 证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p24) 的 `references.json` 与 `reference/`。

discovery 当前只声明 ES256，JWKS 两个公开 key 都是 EC/P-256。实现只支持这一已核实 profile，不把 JWT 的 alg 当动态算法协商，也不依 token 提供的 URL 或 key 另行请求。

## 2. 已实现的 owner

- `oidc.rs`：公开 `OAuthClient::verify_code_identity`，网络前比对完整 OAuthClient 授权快照，验证后才构造无 raw token 的 VerifiedIdentity；Debug 固定隐藏身份。不改变原 grant transport 的未验证返回类型。
- `oidc/jwt.rs`：compact JWT 总长32KiB、恰三段、无padding base64url、kid必需、alg精确ES256、64字节raw R||S；ring验证原始header.payload后才解释claims。拒绝crit/b64和token自带key/URL/certificate headers，header/claims重复顶层字段拒绝。
- `oidc/jwks.rs`：响应64KiB、1–32个key；保留每个JWK的raw JSON再查重复字段，重复kid拒绝。所选key必须EC/P-256；存在alg/use/key_ops时分别限定ES256/sig/单一verify，拒绝私钥d；x/y各32字节，实际曲线点由ring验签检查。
- `oidc/claims.rs`：iss精确相同、sub非空且最多255个ASCII字节、aud必须是当前client字符串或单元素数组、存在azp时必须相同；nonce constant-time比较。exp/iat必需，nbf可选，60秒clock-skew、溢出和非法类型拒绝；存在at_hash时检查access-token SHA-256左半摘要。
- `oidc/fetch.rs`：每次只从固定issuer的 `/.well-known/jwks.json` 读一次新key集，不缓存旧key、不重试、不起后台任务；禁共享默认headers、禁redirect、10秒总deadline覆盖完整body，声明及decoded累计字节均有界，drop取消其连接等待。

OIDC Core §3.1.3.7 要求额外audience必须可信；本客户端没有登记额外受众，因此不能仅因azp正确就接受任意多audience。这是明确的本地信任策略，不冒称参考客户端完全相同。网络函数也不是对任意恶意HTTP client配置的沙箱；TLS/proxy等仍由未来正常调用owner准确构造。

**不包含：**Team/device缺ID-token的grant acceptance、consent principal确认、Console管理身份/Secret Grant/一次callback/TTL会话、设备会话与退出join、正常秘密保存、自动refresh generation/CAS、独立preset/模板与业务Responses调用。VerifiedIdentity只证明所验ID-token的end-user身份，不授予管理权限，不证明业务token的principal或额度，不合并账号。

## 3. 实际red与验证

生产注册前，新身份测试实际编译失败，`red-identity-v1.log` exit101，错误为缺 `oidc` owner及 `OAuthError::InvalidIdentity`（E0432/E0599）。这是缺实现的编译红测，不冒称复现旧生产运行漏洞。red编译期间将一个多audience正例按刚核实的Core受众信任规则改为singleton；该更改不改变上述缺符号失败的归因，不将其旧源码版本混称最终来源。

合成测试使用新生成ring P-256私钥签名；包括不同私钥、重新签名的iss/aud/nonce/time等mutation、at_hash、alg/JOSE/duplicate字段、JWK约束，以及公开方法的真实loopback GET、连续新key、禁止header继承和redirect、malformed/超限body、取消与自身10秒deadline。全部业务HTTP限随机loopback，不发真实grant/refresh或推理请求。

最终 `green-identity-v1.log` 实际exit0：编译10m44s，运行15.04s，**29 pass / 0 fail / 0 ignore**，包含原18项与本批新增11项。`refresh-adjacent-v1.log` 为16 pass，`call-flow-adjacent-v1.log` 为40 pass；三过滤器不重叠，`audit-results.mjs` 实际解析合计 **85 pass / 0 fail / 0 ignore**，存为 `test-results-v1.json`，不把P23或本批red重跑累计成额外测试。

`feature-disabled-v1.log` 的 `cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib` 实际exit0，用时1m38s；只保留Gemini，用于证明新OIDC底座不依赖Grok Web产品feature，不冒称此前全feature-off Gemini缺符号或完整feature矩阵已解决。现有29条test-profile warning未越界处理，本批新owner没有对应warning；未运行全仓回归或正式release gate。

独立只读复审在本批限定范围未发现可确认的验签/绑定绕过或无界后台任务，未运行测试，不将其算动态通过。以上test-profile编译耗时不是EXE构建或打包速度。

`run-static.ps1 -Version v1` 已exit0：官方Rustfmt、checker31/31、无报告ratchet **4047文件 / 14 immutable / governed source >700=0**、tracked whitespace通过。未覆盖原有脏有效行报告或调整baseline/豁免。`source-before.json` 在写源码前保存，`source-frozen-v1.json` 在启动green前冻结10文件；最终 `audit-final.mjs v1` 复核源码hash匹配、14个源码/文档严格UTF-8无BOM、本地链接及围栏、包含未跟踪源码的独立whitespace通过，见 `source-doc-final-v1.json`。

有效行前→后：root114→118、tests171→174；新oidc66、claims88、jwt63、jwks55、fetch44、signing fixture55、identity tests273、wire tests265。两个251–500行测试文件分别围绕身份策略和真实网络资源合同，均单一职责，不需要软上限例外。

## 4. 后续与总目标

下一步在明确Team/device与未验证principal语义后，接正常Console会话、秘密保存、共享refresh generation/rotation/CAS及实际业务调用；不能将当前公用方法直接当全部grant已认证。

所有R01–R16及同平台额外API/Web分支继续保留，API Key/OAuth/Web按产品与实际额度区分，unknown不合池，Web不是兜底。Qwen OAuth上游受限约束仍待决。其余OAuth/API/Web/媒体、Qwen/Bedrock剩余保真和PG边界、既有feature/前端/expiry失败、P50包与隔离启动、集中录制及真实逐渠道验收均未由本批关闭。

未读取或试用历史凭据、真实.ng或在线数据库；没有真实授权/刷新/生成、费用、EXE包、部署或运行实例变更。后端测试显式locked/jobs4/no-default-features/runtime-defaults，TEMP/TMP/TMPDIR仅用linshi，不触发默认management-ui发布/清理hook。总goal保持active。
