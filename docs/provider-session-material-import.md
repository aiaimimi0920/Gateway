# 产品绑定的 Web / Session 材料导入

> 2026-11-02；本批属于[分渠道扩展计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)的无凭据开发。
> 这是已有执行器的材料录入及正常保存链，不是新增 OAuth 授权、完整登录录制或真实平台验收。
> 本文保留 P02 七产品的原合同；P36 的 Grok Build 手动 OAuth 见其独立回执，不属于这里的 Session 导入。P40 新增第八个 Session 产品 `perplexity-web`，必须提供原会话 `userAgent`，仅投影本站 session/可选 clearance；其 storageState、最早expiry及UA绑定规则见 [Perplexity Web合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-perplexity-web.md) 和 [P40回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p40.md)。

## 1. 新建与认证边界

标准模板新增 `grok-web / qwen-web / chataibot-web / lumalabs-web / suno-web / udio-web / freebuff-web`。
均创建普通独立 provider 和空凭据池；没有默认账号、默认权益、自动测试、自动补号或新增蓝色验收登记。
没有当前账号的目录证据时，模板不预填模型，用户后续录制/发现或手工选择精确模型；内置执行器的默认模型不被冒称为当前可用模型。
媒体模板采用无标准答案的媒体生成测试提示词，不使用文本 IQ/`OK` 回复判定媒体成功；实际操作/资产验收仍属于后续工作。

添加账号按已保存 `preset` 选择产品控件，不按可修改 Provider ID 判断。Grok Web 不投 xAI API；Qwen Web 不投普通 DashScope 或 Coding Plan。
`freebuff-web` 的 `-web` 表示浏览器辅助的 session 渠道；它不是广告 Key、Codebuff SDK Key，也不表示当前已完成官方 CLI 批准状态机。
这里不开放 Accio/Kiro/Grok Build 的假 OAuth 模板；其授权和刷新工作包继续独立实施。

## 2. 导入 JSON 合同

在已获得敏感信息授权的账号对话框中粘贴 JSON 后点击“读取会话材料”，或选择 JSON 文件。
JSON 必须为单个对象，UTF-8 总大小最多 128 KiB；只允许下面声明的字段。错误不回显材料原文。
读取后清空粘贴框；切服务商、关闭、编辑会话改变或授权失效时，临时明文和晚到文件结果失效。
解析成功仅表示材料形状与所选产品匹配；不能密码学判定 opaque token 的类型、有效性或真实权益，认证仍由上游执行。

所有产品要求 `product` 精确匹配模板 ID；通用可选字段为 `cookieHeader / userAgent / expiresAt`，`token` 的实际语义如下。

| product | token 的含义 / 可替代来源 | 产品必需补充材料 |
| --- | --- | --- |
| `grok-web` | SSO token 值；可从完整 Cookie 的 `sso` 提取 | 无；完整 Cookie 可保留附加会话项 |
| `qwen-web` | Web auth token，必填；完整 Cookie 不能替代 Bearer | 无；本入口不导入账号密码恢复 seed |
| `chataibot-web` | `token` Cookie 的值，或从完整 Cookie 提取 | 无；执行器用于图像，不表示通用聊天 |
| `lumalabs-web` | `wos-session` Cookie 值，或从完整 Cookie 提取 | `realmId`：uni-1 board/realm ID |
| `suno-web` | 从完整 Cookie 的 `__session` 提取；若显式填 token 必须一致 | `cookieHeader` 必填且包含 `__session`，不接受只有短期 bearer |
| `udio-web` | `sb-ssr-production-auth-token` 值，或完整 Cookie | 分片时仅提供完整 Cookie，保留连续 `.0/.1/...`，不拼成新 bearer |
| `freebuff-web` | FreeBuff authToken，必填 | `agentId`：当前执行器的 agent 标识 |

示意材料如下；尖括号内容需要后续录制得到的真实值，本轮没有读取或使用用户凭据。

```json
{
  "product": "lumalabs-web",
  "token": "<wos-session值>",
  "realmId": "<board或realm标识>"
}
```

`expiresAt` 必须为有效的未来 RFC3339 时间。若 token 包含可解析 JWT `exp`，仅作为更早的到期提示，不作为本地认证证明。
缺省时间不伪造有效期，也不证明自动刷新存在。Cookie/token 控制字符、重复 Cookie、互相矛盾的身份以及不完整 Udio 分片被拒绝。
不允许 `baseUrl / headers / extra_body / refreshToken / authSeed` 等任意导入扩展，更不能从文件配置目的地或刷新服务器。

Qwen 既有恢复 seed 账号暂不通过此入口替换 Session，避免新 token 被旧账号密码恢复覆盖；其安全导入/更新仍需独立完善。
FreeBuff 本入口只支持显式 agentId，不接受既有 provider/credential model-agent map，避免隐式覆盖绑定；映射模式是后续单独合同，不悄悄删旧映射。
FreeBuff fingerprint 登录字段当前不由推理执行器读取，本入口拒绝这类未接通字段，不以“已保存 fingerprint”冒充登录已完成。

## 3. 普通草稿、秘密补丁与目标绑定

- `providerSessionMaterial.ts` 严格解析并投影有限字段；不展开导入对象，不写 localStorage、普通 JSON 或测试日志。
- `providerSessionMaterialApply.ts` 将 token、Cookie 和认证清理转成现有 `CredentialSecretEdit`；普通文档只保存 realm/agent、UA、有效期与指定 session_auth。
- 替换时清除旧 Cookie、显式 Authorization、auth_token、refresh_token，避免旧身份/旧 OAuth 刷新覆盖新会话。存在 refresh endpoint/client/lifetime 配置时直接拒绝，不尝试转换为 OAuth。
- Cookie/Authorization 的旧大小写路径分别发 clear，再对规范路径 replace/clear；后端路径大小写敏感，不能仅丢弃旧 keep。
- 暂存秘密附非秘密产品绑定；最终构造 `SecretPatch` 时再次校验保存 preset、固定 HTTPS origin/path、执行器、账号 realm/agent、认证结构与有效期。
- 暂存后更改目的地址、产品、FreeBuff map、realm/agent、refresh 或 session_auth 会阻止提交。共享 provider 级认证也被拒绝，不将新账号秘密与旧共享池隐式混用。
- 预览阶段剔除不安全的暂存值而不崩溃；真正 `buildCommitRequest` 即使接收显式 secretPatchOverride，也执行严格复核。
- 最终仍使用原 Secret Grant、expected revision/CAS、身份重绑定和持久化 owner；不另起保存接口，不改变数据库版本或旧凭据格式。

此处的目标保护只覆盖本产品导入产生的待提交秘密；不声称重写了整个管理编辑器的出站授权模型，也不代替每个 transport 的重定向/SSRF 保护。

## 4. 尚未证明或尚未实施

本批不证明真实 SSO/账号资格、Qwen signin seed 生命周期、FreeBuff 官方 CLI 登录和 fingerprint 接线、Clerk 刷新、模型目录/额度、媒体生成、challenge、worker 可用性或取消/资产安全均已验收。
没有启动真实浏览器、发起 OAuth、调用上游、写在线数据库、接管实例或制作 EXE；集中录制仍在计划后半段。
离线证据与当前剩余项见 [P01 材料链检查点](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p02.md)。
