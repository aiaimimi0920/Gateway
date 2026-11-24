# Accio OAuth 管理登录合同

本页描述 P44 的服务端管理入口、正常保存合同和 P45 的模板/桌面 UI；不代表真实账号登录或整个 Accio 渠道已完成。协议来源、固定版本与 MIT 声明沿用 `src/protocol/accio_oauth/REFERENCE.md`，不内置参考项目的第一方 client 身份。

## 目标与权限

只针对已经保存的 `preset: accio` provider。真实编译后的 adapter 必须为 `accio_compatible`，业务根为 `https://phoenix-gw.alibaba.com`，Responses 路径为 `/api/adk/llm/generateContent`。重授权还检查显式稳定 credential ID 及其实际编译后的覆盖，不靠 provider 显示名称、数组下标、token claim 或邮箱合并账号。

所有请求使用现有管理身份认证。create、complete、cancel、import 还要求正常 `x-secret-grant`；GET 只要求同 owner 管理身份。owner 绑定管理 token fingerprint、Host 和客户端 IP；Secret Grant 仍独立校验 Origin/IP/期限。PKCE/state 不授予管理权限，也不是已验证的 Accio 账号身份。

只在正常可写 runtime 上接纳新登录与保存，drain/shutdown 拒绝新增。每次 exchange/首次 import 都核对原保存 revision，不自动取得较新版本继续操作。

## HTTP

共同前缀：`/v1/internal/gateway/console/accio-auth-sessions`。成功响应为 `{"session":...}` 并使用 `Cache-Control: no-store`；没有 token、refresh、SG、utdid、原始 callback、code、verifier 或材料 receipt 返回浏览器。授权 URL 本身包含本次授权所需的 state/challenge/client 等公开浏览器参数，应只向操作者展示。

### 创建

`POST` 集合地址，结构示例：

```json
{
  "providerId": "my-accio-pool",
  "expectedRevision": "saved-route-revision",
  "clientId": "your-permitted-client-id",
  "loginSite": "china",
  "callbackPort": 34567,
  "utdid": "explicit-existing-device-identifier"
}
```

`loginSite` 必须显式选择 `china` 或 `global`，分别使用协议 owner 已固定的 `https://www.accio-ai.com/login` 或 `https://zh.accio.com/login`；这不证明两站产品额度通用。client 由操作者显式提供；没有第三方注册许可的实证，不以开源中出现某个 client 值冒充获准使用。

`callbackPort` 为非零 `u16`，只生成 `http://127.0.0.1:<port>/auth/callback`。当前是**手动完整 callback 桥接，没有自动 listener**；需要操作者在浏览器授权后复制完整地址，即使该回调页面无法连接。请求不能指定任意 redirect URL、issuer、token/refresh endpoint 或业务 base，未知字段拒绝；创建 body 上限 16 KiB。

可选 `credentialId` 表示明确重新授权现存稳定 ID；省略表示新增独立账号。新增必须提供 `utdid`；重授权省略时，只从该 credential 自己已配置的 `utdid/x-utdid` 取值，拒绝缺失或冲突。不会读取 provider 的共享设备值，也不从 state、ttid、UUID、SG 或 token 生成设备身份。显式 utdid 限 1–1024 字节可打印非空格 ASCII，不 trim 改值。

provider 层存在可继承的账号认证/设备副本时拒绝此专用登录目标，不静默清理共享 provider 或修改其他账号。credential 自身旧认证副本的处理见下文；普通存储读取和既有旧导入不因此重写或迁移。

### 读取与动作

- `GET /:session_id`：同 owner 管理身份读取非秘密 view。
- `POST /:session_id/complete`：`{"expectedRevision":"...","callbackUrl":"完整地址"}`。
- `POST /:session_id/cancel`：`{"expectedRevision":"..."}`。
- `POST /:session_id/import`：`{"expectedRevision":"..."}`。

动作 body 上限 32 KiB。每个动作的 revision 必须匹配会话原 binding；complete/首次 import 还核对实际 active revision。callback 由协议消费式校验精确 redirect、state、重复/歧义参数与授权拒绝；即使 callback 无效，也不复用该 attempt。exchange 不自动重试，取消、TTL 或 future drop 不允许迟到材料复活。

## 普通保存与重授权

材料只留在服务端。首次 import 先通过正常 redacted draft、`SecretPatch::keep/replace/clear`、`resolve_secret_patches` 和完整配置预验证，成功准备后才消费材料并分配/固定保存目标；预验证拒绝不消耗 Ready 材料。随后 registry 内受跟踪的保存任务使用原 revision 调用正常 `commit_document`，复用既有 publication owner、事务日志与 CAS，不新建第二套配置存储或刷新 scheduler。

- access → credential `api_key`；refresh → `refresh_token`；SG → credential-only `accio_sgcookie`。
- refresh endpoint 固定为 `/api/auth/refresh_token` 的完整 Accio URL，client 保存显式值；正常共享刷新链继续使用当前 access + refresh 的专用 JSON wire。
- 原 `expiresAt` 直接格式化为 RFC3339，保留纳秒精度，不重建 `now + ttl`，不制造相对 lifetime。有效但已过期的 access 与其 refresh 仍可被完整保存，以免丢弃已返回材料；保存成功不是业务可用声明，实际 dispatch 的 refresh readiness 继续拒绝过期 access。
- **新登录不同于 refresh**：没有已验证的同账号证明。新 grant 缺 SG 时明确 clear 目标旧 SG，不沿用 refresh 的缺省保留语义；有新 SG 时 replace。
- 目标旧 `auth_token`、Cookie/Authorization 类 header、认证 body token/refresh/SG 副本和旧 device body 副本通过明确 clear/移除归一化；写入本次显式或保留的可信 credential utdid，清除旧 Cookie 派生 cna。旧账号上下文 `empid/empId/accountId/account_id` 与 `tenant/tenantId/tenant_id` 也移除：新授权没有证明它们属于新账号。provider 层存在这些副本时拒绝专用登录，而不是清空共享值；其他账号不被改动。
- 稳定 ID、enabled、操作者名称、模型、policy 与其他未修改元数据保留。未知辅助秘密仍经过正常 keep 受众检查；无法证明安全的目标不绕过校验，也不擅自清除未知秘密。

## 生命周期与结果

每实例最多 16 个会话，登录 monotonic TTL 600 秒，一个定时清理 worker；没有每账号无限后台轮询。已接受保存的任务在 registry JoinSet 内受跟踪并回收，正常 shutdown 停止新增并 join。保存异步阶段共享 20 秒预算，不包含之前同步预验证，也不声称可抢占同步磁盘 I/O。

`waiting_user → exchanging → ready` 只表示授权材料状态。只有正常 commit 返回成功才为 `succeeded`，并具有 `credentialId/savedRevision`。`importing`、`targetCredentialId` 存在或拿到 token 均不是保存成功。

commit 错误、timeout、保存 future drop 均进入 `save_unconfirmed`，保留非秘密目标定位；不会自动重投。重复 import 只返回同一已接纳操作，不能生成第二个账号或重复 commit。HTTP 等待者丢失、cancel、登录 TTL 不撤销已经接受的保存。成功/未确认回执额外保留最多 300 秒。

`failed/cancelled/expired` 是尚未接受保存的终态。会话不跨 Gateway 进程重启；已持久配置继续由现有存储与 journal 恢复。未知保存结果不能仅凭目标 ID 存在猜成成功。

## 正常模板与桌面入口（P45）

正常目录新增 `accio-oauth`，默认 provider ID 同名但可改名，兼容 preset 仍是 `accio`。新模板在最终投影处拒绝非固定 Phoenix 地址，不重写旧手工配置。默认空 credentials、`explicit_credentials_only=true`、空 supported_models 和无新增路由；不表示旧 runtime preset 没有默认模型，也不禁止用户之后显式配置模型/路由。

新增账号与稳定 ID 重授权均由真实全局 CredentialDialog 接入。client/site/port/device 没有预填默认值；utdid 原样校验和发送，新账号必填，重授权留空才省略。callback 为密码输入，限制 16384 字节，提交后清空；可执行链接只允许两个固定 HTTPS origin 的 `/login`。没有 listener、principal 或已验证身份声明。

OAuth 材料仍只在服务端保存；只有 succeeded 才关闭/refresh，不送普通账号表单重复保存。非秘密保存 receipt 按 client/管理身份和产品隔离，关闭重开只查原会话；未知响应不重投，迟到 create 使用原 owner 取消，旧 poll 不覆盖新动作。30 秒请求预算、1.5 秒串行轮询、五次连续失败暂停及最多 16 个未确认 receipt 均有定向测试。

旧手工录入通过明确的录入方式保留，字段标为 Access Token，不承诺补齐 device/refresh/SG；编辑 metadata/模型和 keep 保留。存在活动登录、待恢复权限或未确认保存时，模式切换和返回编辑被锁住，不能借手工表单绕过未知保存；仍可 Escape 关闭，重开恢复 receipt。关闭后回到仍存在的 opener，不抢其他 dialog 焦点。

P45 证据、扩大回归的三项基线失败及合成 UI 边界见 [本批回执](progress/2026-11-04-provider-channel-expansion-p45.md)。不将模拟 API 的浏览器检查当真实平台登录或管理后端 E2E。

## 业务调用与精确账号测试（P46）

显式 prompt/model 或保存的测试计划经正常 provider probe 进入专用 Accio 测试，复用精确账号的普通三适配器准备、generateContent planner/header/sender，不使用标准 Chat JSON sender，不换号、跨渠道回退或隐式重试。没有显式测试请求不发送 discovery/generation；管理身份、Secret Grant、enabled 和模型准入仍由原入口负责。只有完整、非空、stop、无工具的文本答案才 Passed；回复最多 8192 字符，所选账号材料先脱敏再进入 report/assessment/audit，报告 total/cache 用量保留。

buffered 与公共 SSE 共用专用 admission decoder：空响应、usage-only、未知/截断、孤立工具参数及业务错误不能成为成功；成功 Finish/DONE 保留至 EOF 校验，防止后到错误被吞。单帧 256 KiB、总 wire 64 MiB、最多 128 次工具创建和累计 1 MiB 参数。stream 不新建后台生成任务，drop 释放原上游；共享 Anthropic decoder 不施加这套 Accio 终态规则。

这只是现有可识别事件的本地合成合同收紧。固定参考仅核实 OAuth，没有固定 Accio generateContent 的成功终态来源；不能将 outer turn_complete、AWS metadata 或旧 binary envelope 兼容单独宣称为已证实时 wire。具体红绿、验证和未覆盖边界见 [P46 回执](progress/2026-11-04-provider-channel-expansion-p46.md)。

## 旧材料的新导入（2026-11-09）

folder normalizer 现保留新导入的 access/refresh/SG/显式client和精确绝对expiry；root与两种extra命名中的同义材料不一致或类型无效时拒绝，不制造client身份或相对TTL。业务body不再保存token/accessToken/utdid副本，普通planner仍绑定canonical access/device，refresh/SG/client/rawSource不进入业务body；SG与归档秘密继续走正常脱敏。显式Cookie/cna和原identity逻辑未因此重写，不声称已清理Cookie内未知SG。

单独更新normalizer不是旧数据库自动迁移：原文件source hash相同在normalize前即跳过。新导入的真实红绿、普通planner与脱敏回归见[新导入回执](progress/2026-11-08-provider-channel-expansion-accio-import.md)；已有行的限定读取兼容见下节。后续独立DB Accio steward及共享Gateway Key下的正常server/管理/public子闭环已接入并验证，见[联合回执](progress/2026-11-09-provider-channel-expansion-accio-db-server.md)。正常route-config OAuth仍由上述owner负责；DB刷新不写另一hosted身份的gw:cred，不以cache repair字段读取冒充DB scheduler。

## 已知旧 DB 形状的普通读取兼容（2026-11-09）

普通inline/object hydrated reader对`folder_sync_import`/`folder_sync`、Accio surface、固定Phoenix受众、原rawSource和已知旧body/device副本精确认形。原access/refresh和现存上下文仍匹配原材料时，读取view补SG/显式client/endpoint/原绝对expiry并移除旧body副本；轮换、partial clear、null或不同上下文不恢复历史字段，canonical access空/null也移除已证旧body认证副本，不能回退使用旧token。手工/未知形状不泛化改写，关线不投影。

这不修改原DB/object bytes、source hash/path、ID、状态、诊断、时间戳或xmin，不增加storage version，也不强制重导；仍保留DB行时不依赖源文件继续在场。后续显式保存仍由正常writer/snapshot CAS决定，迟到view不能覆盖后来rotation/clear。共享parser只迁入协议owner，材料算法不复制。8项reader与真实PG/Redis3项通过，正常列表脱敏、同hash跳过、object原bytes/mtime和普通planner已有证据；相邻刷新回归有1项并行Nous失败，不能称全仓绿。详见[本批范围与来源回执](progress/2026-11-09-provider-channel-expansion-accio-db-reader.md)。

## 仍需继续

真实第三方client可用性、真实业务终态/模型/额度、完整DB身份/结算和各运行模式、OS独立进程与全部后台join、Accio专属完整取消audit和集中凭据录制仍需后续批次完成；DB worker与共享Key server子链不再列为未接入。未知/手工旧形状不自动重写，会话不跨Gateway进程重启。本页不宣称完整渠道 `offline_ready`、真实登录、Redis Lua新验收或包交付。
