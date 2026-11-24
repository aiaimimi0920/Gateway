# Grok Build OAuth 管理登录合同

本页描述 P35 管理 HTTP/正常保存链及 P36 手动授权 UI，不表示 Grok Build 全渠道完成或已经真实登录。实现与验证记录见 [P35 回执](progress/2026-11-03-provider-channel-expansion-p35.md) 和 [P36 回执](progress/2026-11-03-provider-channel-expansion-p36.md)。

## 产品与前置条件

Grok Build、xAI API Key 和 grok.com Cookie 是三个独立材料/受众 owner，不能互投。此流只针对已经保存的 `preset: grok-oauth` provider，业务 base 为 `https://cli-chat-proxy.grok.com/v1`；正常配置编译仍负责完整的 adapter、认证、路径与受众检查。不会新建 provider、默认账号、模型目录、模型路由或推理成功记录。

OAuth client ID 和 scopes 必须由操作者显式提供，且确实可用于此集成。代码没有内置官方 CLI client 身份；公开源码中的客户端值不等于第三方使用授权。当前第三方客户端可用性与产品使用边界仍需在集中真实验收前核实，不通过借用身份或放宽目标约束解决。

请求必须通过现有管理身份认证。创建和所有 action 都额外要求正常 `x-secret-grant`；它继续绑定管理 token fingerprint、origin、IP 与期限。会话 owner 单独绑定管理 token fingerprint、Host 和 IP，使同一管理客户端的无 Origin GET 可以轮询。OAuth state、PKCE、ID-token 或 Team hint 均不授予管理权限。

## HTTP 接口

共同前缀：`/v1/internal/gateway/console/grok-auth-sessions`。成功 JSON 均使用 `Cache-Control: no-store`，不会返回 token、receipt、secret patch 或原始 callback。

### 创建

`POST` 集合地址，示例结构：

```json
{
  "providerId": "my-grok-build",
  "expectedRevision": "the-saved-route-revision",
  "clientId": "your-permitted-client-id",
  "scopes": ["openid", "offline_access"],
  "redirectUri": "http://127.0.0.1:34567/callback"
}
```

`credentialId` 可选。提供时表示重新授权这个稳定 ID 对应的现存账号；未提供时表示新增独立账号。不按邮箱、数组位置、相同 token 或未验证 principal hint 自动合并。

可选 `principalSelection: {"kind":"Team","id":"..."}` 只是 consent 预选；独立的 `requiredPrincipal` 同形对象才是强制匹配约束。强制匹配不会把 endpoint 返回的未验签 hint 变成已验签用户身份。

client/scopes/redirect/principal 沿用协议 owner 的输入限制。redirect 仅接受带显式端口、路径为 `/callback` 的 HTTP loopback URL，不接受用户名、密码、query 或 fragment。请求不能指定 issuer、token endpoint、业务 base 或导入自造 receipt；未知字段拒绝。创建 body 上限 16 KiB。

创建检查真实 provider ID、精确 preset、固定业务 base、目标 credential ID 和原 `expectedRevision`，还需要正常可写 runtime。drain/shutdown 不接纳新登录或新保存。成功返回 `session.authorizationUrl`，不会自动启动浏览器。`authorizationFlow` 默认 `code`（手动 callback），也支持 `device` 和显式选择的 `loopback`。

`loopback` 只在运行 Gateway 的机器上绑定显式注册的数值地址 `127.0.0.1` 或 `[::1]` 及非零端口，路径仍为 `/callback`。浏览器必须在同一台机器上；远端管理不等于在浏览器机器上监听。此模式拒绝 `localhost`，避免 DNS/IPv4/IPv6 解析歧义；手动模式继续兼容原有 redirect。端口被占用直接拒绝创建，不自动换端口、不返回假授权成功。返回的 `session.automaticCallback` 表示此会话采用自动接收，旧回包缺省仍按手动方式处理。

自动回调复用共享有界 HTTP parser 与原 PKCE/state 校验，错误 state 不消费会话；合法拒绝终止等待。回调交换前重查目标与原 revision，只到 `ready`，绝不自动保存。每个 registry 最多 16 个受跟踪 listener，取消/过期/关停退出，关停等待任务回收；浏览器断开不撤销已接纳的单次交换。请求读头最长 5 秒、32 KiB，回复最长 1 秒且不回显 callback/code/token。

`device` 不接收 `redirectUri` 或 consent principal selection，返回公开 `deviceAuthorization`。用户明确点击检查时才调用 `POST /:session_id/poll`；普通 GET 只读取状态，不自动向上游轮询，且遵循上游间隔。

### 轮询、完成与取消

- `GET /:session_id`：需要同 owner 的管理身份，不额外要求 Secret Grant。
- `POST /:session_id/complete`：body 为 `{"expectedRevision":"...","callbackUrl":"http://127.0.0.1:34567/callback?code=...&state=..."}`。
- `POST /:session_id/cancel`：body 为 `{"expectedRevision":"..."}`。

action body 上限 32 KiB。每个 action 的 revision 必须等于原会话绑定；完成前还会再次检查真实 target 和当前 revision。不会因为配置已变化而重新取得最新 revision 继续交换或保存。callback 仍由协议 owner 校验原 redirect、单次 state、PKCE、拒绝/歧义参数与适用身份验证。取消、TTL 和 exchange future drop 不允许迟到交换结果复活。

## 保存与状态

`POST /:session_id/import`，body 为 `{"expectedRevision":"..."}`。此接口只在服务端保存，不把 token 下发给浏览器后再上传。

保存前先在同一原 revision 上生成正常 redacted draft，通过 `SecretPatch::keep/replace/clear` 与 `resolve_secret_patches` 解析，再做正常配置预验证。access token 落在 credential `api_key`，refresh token 落在 `refresh_token`；新的 receipt 没有 refresh token 时明确 clear 旧值。`grok_oauth` context 从真实 receipt 生成，expiry 使用 receipt 的原绝对时刻，不按保存时间重建生命周期。重新授权保留稳定 ID、enabled、操作者名称、模型和 policy 等未修改字段；其他账号与 provider 的材料沿用正常 keep 保护。

若重新授权改变 context，而目标账号还含其他受保护秘密，普通 keep 可能拒绝；不会绕过该检查、悄悄复用或清除未知辅助秘密。预验证失败不消耗 Ready receipt。

接纳保存后，实例 registry 的受跟踪任务拥有 receipt 和候选，使用原 revision 调用正常 `commit_document`。HTTP 等待者丢失、重复 import、cancel 或登录 TTL 都不会撤销已接受的 commit。shutdown 会停止新增并 join 已接纳保存；异步保存阶段采用 20 秒 deadline，不包含接纳前同步预验证，也不声称可以强制中断同步文件 I/O。

| 状态 | 含义 |
| --- | --- |
| `waiting_user` / `exchanging` | 等待用户 / 单次交换中 |
| `ready` | receipt 已接受，尚未保存 |
| `importing` | 已交给有界保存 owner；不是保存成功 |
| `succeeded` | 正常配置 commit 已确认成功，具有 `credentialId` 与 `savedRevision` |
| `save_unconfirmed` | commit 报错、超时或保存 future drop；不宣称成功，不自动再提交 |
| `failed` / `cancelled` / `expired` | 尚未接纳保存的失败、取消或过期 |

`claimed` 仍保留为底座的单次内部材料转移状态，不是 HTTP 成功保存状态。HTTP 保存使用明确的 operation owner。

`targetCredentialId` 是已接受操作的非秘密目标定位信息，`expectedRevision` 始终是原 revision。它们有助于对照普通配置和事务恢复，不能仅因目标存在就将结果改报成功。重复 import 只返回同一个已接纳操作的状态，不生成第二个账号或重新提交。未确认保存由正常 runtime journal 的恢复 owner 处置；本批没有新增完整的自动 reconciliation UI 或跨进程登录恢复协议。

会话总数最多 16，登录 TTL 600 秒，成功/未确认的非秘密回执额外有界保留 300 秒。registry 按实例拥有，只有一个 expiry worker；保存 JoinSet 按会话边界受控并回收，不建立每账号无限轮询。重启会结束登录会话；已有持久配置由正常存储 owner 恢复。

## 正常模板与手动授权 UI（P36）

新增 `grok-oauth` 正常空池模板，固定上述业务 base、空模型、无默认账号，不自动探测或添加路由。既有 `xai-api`（默认 ID `xAI-api`）和 `grok-web` 保留；不迁移或合并凭据/额度，也不将模板标记为适配验收完成。

正常账号弹窗按已保存的 preset（不是可改 provider ID）选择此 UI。先保存服务商及配置；editor lock、dirty draft、草稿不匹配 active revision 时禁止授权变更。所有 mutation 都带 Secret Grant 和原 revision；GET 只带管理身份。授权链接仅允许 `https://auth.x.ai/oauth2/authorize` 的精确 origin/path。

Client ID、scopes，以及代码授权的 loopback redirect 必须显式填写。Consent 预选与强制 principal ID 分开；可以选择手动完整 callback、自动本机接收或设备授权。自动模式隐藏手动 callback 输入，但 Ready 后仍须明确保存。重新授权是账号编辑中的明确动作，目标为原稳定 credential ID；有未保存表单修改时不切换，普通 metadata/模型编辑仍保留。

每个请求 30 秒 abort，状态读取串行，连续五次失败后暂停，操作者可明确继续检查；不自动重提 import。迟到读写分别受 management/client/provider/credential generation 和 mutation version 约束。Active revision 变化会禁止后续 complete/import，但已接纳的保存继续用原 binding 只读检查，不能因新 revision 可能来自自己的提交就丢弃结果。

在同一 Console、client 和 management 身份内，以最多 16 条非秘密保存记录保留会话定位与结果未知标志，不存 callback/token、不写 localStorage；关闭/重开弹窗仍检查同一会话，未确认结果不重新开放提交。明确接纳前拒绝（如 `grok_login_save_rejected`）与网络/解析/超时结果未知分开；前者保留 Ready receipt，后者禁止重发。旧 import 回包只能条件更新仍属于原 session 的记录，不能复活已消费记录或覆盖后续会话。

只有合法 `succeeded` 同时具有 credential ID 和 saved revision 才调用保存完成/刷新；`importing`、目标 ID 存在及 `save_unconfirmed` 均不是成功。未被接纳保存的离开清理使用原 owner API、原 grant、原 revision 做有界 best-effort cancel；已尝试保存的操作不由弹窗卸载撤销。

## 尚未覆盖

上述内存记录不跨 Console 卸载、页面刷新、management 身份替换或 Gateway 进程重启；未知结果不按年龄自动抛弃。跨进程 reconciliation UI、真实第三方客户端授权、完整 Responses 保真与集中账号验收继续后续工作。自动 listener 和 device 管理入口已实现，不代表第三方客户端权限或真实业务验收通过。P34 的跨 revision 刷新恢复、永久 Sending 可用性、其他 refresh 入口及 durability 缺口也未被本批替代。
