# FreeBuff 浏览器批准管理接口

此入口属于 `freebuff-web` 的 CLI/browser-assisted session，不是标准 OAuth，也不是平台推理 API Key。它复用已有 `preset: freebuff`，不会因为登录方式新增同义额度池。

本文描述当前接口与边界；对应定向验证结果单独记录在进度回执。[正常前端登录/重授权 UI](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-ui.md) 已接线；后续 [本地管理子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-local-ui.md) 已用正常 main→真实管理 TCP、Local runtime/持久配置/SQLite与合成认证 peer 验证新增、metadata Keep、稳定账号重授权和两次同目录重建。真实平台登录、独立进程重启和完整业务不能由此推定完成。

## 1. 管理权限与目标

接口根路径为 `/v1/internal/gateway/console/freebuff-auth-sessions`。

- 所有请求要求正常管理身份。会话 owner 绑定管理 token fingerprint、Host 和客户端 IP，不把浏览器批准链接或上游材料当管理权限。
- create、poll、import、cancel 均要求正常 Secret Grant，并沿用其 Origin 校验。状态 GET 不要求重新提交 Secret Grant，但仍须同 owner。
- create 的 JSON 上限为 16 KiB，action 为 1 KiB；未知字段拒绝。成功、业务错误和提取器错误均返回 `Cache-Control: no-store`。
- 必须存在可写配置 coordinator；在创建、poll 和准备保存时核实实际 provider、原 revision 和产品目标。最终保存仍由正常 revision CAS 决定，不以先前检查代替提交仲裁。

新批准入口只接受固定 FreeBuff 产品：`preset: freebuff`、`https://freebuff.com` 根地址及既有 FreeBuff adapter。拒绝共享 provider Key/Cookie、自定义业务 origin/path、认证 header、其他 OAuth/refresh owner、browser worker 或其他冲突执行设置。该限制只针对新 typed receipt/登录，不把旧 generic adapter 的 Codebuff 或自定义路径配置重写成新产品。

## 2. 请求与公开会话

创建：`POST <root>`。

```json
{
  "providerId": "freebuff-pool",
  "expectedRevision": "current-saved-revision",
  "credentialId": "existing-account-id",
  "fingerprintId": "synthetic-device-id"
}
```

`credentialId`、`fingerprintId` 可省略。新增账号省略 fingerprint 时服务端生成随机设备 ID；对已有 typed credential 重新授权时，省略会复用该账号自己的设备，显式提供不同设备则拒绝。未配置设备的旧账号可以在显式重新授权时获得新设备，不从其他账号或 provider 继承。

读取：`GET <root>/:session_id`。

操作：`POST <root>/:session_id/:action`，action 仅 `poll`、`import`、`cancel`，JSON 为：

```json
{ "expectedRevision": "same-revision-as-create" }
```

所有成功响应为 `{ "session": ... }`。公开字段包括 `id`、`providerId`、`expectedRevision`、`status`，以及适用时的 `loginUrl`、`pollIntervalSecs`、`expiresInSecs`、`targetCredentialId`、`credentialId`、`savedRevision`、固定 `errorCode`。不返回 authToken、fingerprint ID/hash、用户邮箱或原始上游响应。

主要状态为 `starting → waiting_user → polling → ready → importing → succeeded`；终态或异常还包括 `cancelled`、`expired`、`failed`、`save_unconfirmed`。`polling` 时重复 poll 只读取现有状态，不创建并行 GET。合法 Pending 或可重试 GET 的 transport/timeout 返回 `waiting_user`，继续使用同 challenge、原 deadline 和五秒间隔；不是重发 code POST。

批准链接只用于用户在 FreeBuff 浏览器页面确认。正常账号弹窗默认浏览器批准，另保留现有 token+agentId 材料导入；显式检查授权和保存分别执行，定时器只 GET 管理状态。UI 和原生 launcher 都精确限制 FreeBuff HTTPS origin，不自动登录、监听 callback 或录制凭据。

## 3. 会话和任务所有权

每个 Gateway runtime 最多保留 16 个会话；登录 TTL 为 600 秒，并受上游 challenge 更短期限约束。没有每账号无限轮询 task，每次管理 poll 只接纳一个有预算的 GET。

初始化、poll 和已接纳保存由 registry 的受跟踪 task 持有，不依赖 HTTP waiter 存活。取消或过期会销毁未保存 challenge/material；过期清理与正常 shutdown 使用同 lifecycle owner。shutdown 取消未保存登录并 join 已接纳任务，而不是遗留 detached task。

import 一旦接纳，cancel 不冒充事务回滚。保存预算为 20 秒，包含 publication lock 等待和 commit；正常退出会等待保存终结。完成回执额外保留五分钟。此会话和回执是进程内状态，不承诺跨进程恢复；配置提交本身使用既有持久 revision owner。

## 4. 服务端秘密保存与重新授权

只有服务端领取的 typed Credentials 可以进入这条 import 链。保存先按普通 owner redaction，再使用 SecretPatch Replace/Clear/Keep，最终走正常配置 commit/CAS：

- authToken 存入 credential `api_key` 兼容槽，仅代表 FreeBuff session Bearer，不将其命名或解释为官方推理 Key。
- 私有设备 receipt 存入可选 credential-only `freebuff_session` 的 `fingerprint_id`、`fingerprint_hash`，独立脱敏、Secret Grant 和 SecretPatch；不放进业务 header、extra_body 或公开 document。
- 不继承或递归拼接 provider 的 receipt；不从批准期限推导 token expiry/refresh，不创造 agentId。
- 新账号生成独立 credential ID；重新授权保留原 ID、模型/agent metadata、同池其他账号和无关 provider 秘密。
- Keep 比较完整 resolved 目标及 receipt；改产品、设备、业务 origin/path 或删除 receipt container 不能借 Keep 搬走旧 authToken。
- folder 原始 CLI receipt 暂不支持归一化，明确拒绝，不能静默当 legacy payload 导入或跨产品导入。旧无 typed receipt 的 adapter 材料仍沿既有行为。

仅正常 commit 成功可报告 `succeeded`。CAS 冲突、超时或保存 task 中断一律不猜成功，记录 `save_unconfirmed` 和目标 ID；重复 import 不重投已接纳保存。后续需要按实际持久 revision 核对结果，当前尚无跨页面/进程的完整 reconciliation UI。

## 5. 已保存账号的业务目录

正常账号编辑弹窗提供 FreeBuff 专属目录工具。打开不联网；未保存过业务目标时，必须显式选择业务站点和 CLI/Desktop，再点击“读取目录”；已有目标仅恢复选项，不自动发送。本地表单有修改、路由草稿未保存、自动保存中、管理身份或 revision 不匹配时，不允许借用已保存账号读取；关闭、取消、身份或目标变化会取消客户端请求并丢弃迟到结果。读取本身不会写入手动模型、`supported_models` 或 `model_routes`；可选的显式应用另见第 5.3 节。

接口：`POST /v1/internal/gateway/console/freebuff-model-catalog`，要求正常管理 token 与 `x-secret-grant`：

```json
{
  "providerId": "freebuff-pool",
  "credentialId": "existing-account-id",
  "expectedRevision": "current-saved-revision",
  "businessOrigin": "https://codebuff.com",
  "client": "cli"
}
```

`businessOrigin` 仅允许 `https://codebuff.com`、`https://www.codebuff.com`，`client` 仅允许 `cli`、`desktop`；没有隐含默认目标，未知字段拒绝，不接受调用者 Key 或任意 URL。这是**对此次读取显式选择跨业务 host 携带该账号令牌**，不是将登录目标改成业务 host；此读取接口本身亦不保存选择。

服务端从同一 snapshot 核对精确 provider/credential 与最终编译 payload，继续执行原 FreeBuff 登录受众约束；显式 disabled 账号也可由管理员读取。调用前后核 Grant/revision，复用有界 discovery slot；没有 ready registration 时执行一次 unsigned Bearer catalog GET，有本地精确 ready receipt 时恢复并签名 GET。损坏 ready/设备拒绝不会静默 unsigned fallback 或自动修复。无重试、redirect、设备生成/注册或 admission，网络与解码 body 总预算最多十秒、目录最多 1 MiB。

成功返回 `Cache-Control: no-store`，包含 `revision`、`providerId`、`credentialId`、`businessOrigin`、`client` 及 `models`。model 仅含稳定 `key`、名称/短文案、warning、badges、access/dataUse/premium/multimodal；不导出短效 handle/fetchId、plans URL、token 或 fingerprint 字段。locked 可展示，未开放行按服务器 issuedAt 排除；目录可见不证明调用权益或额度。上游错误仅返回固定安全错误/code，不回显原始 body。

目录接口仍是只读展示，不是配置发布或公共推理接线。原只读 UI 的定向证据见 [只读目录管理子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-management-catalog.md)；设备注册工具独立显式操作，见下一节。

### 5.1 本地安装设备的显式注册 API

`POST /v1/internal/gateway/console/freebuff-device-registration` 使用第 5 节相同五字段、正常管理 token 与 Secret Grant，以及完全相同的目标/revision 约束。它是有副作用的显式注册，不由只读目录、打开弹窗或服务启动触发。正常账号编辑已接“检查设备”和随后按状态显示的注册/再次注册/重新注册按钮；所有发送都经应用内风险确认。Server 无 LocalRuntime 秘密 owner 时返回 `409/freebuff_device_storage_unsupported`，不会另建本地私钥文件。

首次有效 bootstrap 目录后，服务端在本地 SQLite/现有 AES seal 下持久唯一安装 Ed25519 key，提交 exact-account unconfirmed marker 后才 POST 公钥；网络期间不持有 SQLite writer transaction。安装级 OS try-lock 覆盖 prepare 到保存，目录/账号 token 改变不复用旧 registration。成功保存返回相同身份字段、`registration:"stored"` 和非秘密本地 `recordId`，不返回上游 key ID、私钥、token 或短效模型能力；已有 ready 普通重复请求直接核本地 receipt，不发新 GET/POST。这只证明本地已保存，不证明上游当前有效或有模型权益。

POST 结果未知、取消或保存前 fence 失败时保留 marker，普通重复请求**在任何上游 GET 前**返回 `registration:"unconfirmed"` 与 `unconfirmedAttemptId`，不重 POST。若管理员明确决定承担再次注册的未知影响，另一次请求可以携带当前精确 `retryUnconfirmedAttemptId`；服务端先轮换 attempt ID 再发送，同一旧 ID 不能再次重放。这里**没有服务端幂等、稳定 key ID 或自动安全重试的保证**。损坏 ready 不可借确认参数覆盖。保存后最后响应 fence 失败返回错误但保留已验证 receipt，下一次精确读取可核实，不把响应失败误当事务回滚。

### 5.2 纯本地状态与已有记录的显式重新注册

`POST /v1/internal/gateway/console/freebuff-device-registration/status` 只接受第 5 节五字段，同样核管理身份、Secret Grant/Origin、精确目标与前后 revision。它不获取 discovery slot、不发上游 GET/POST、不生成 key。返回绑定字段及以下互斥形态，所有响应 `no-store`：

```json
{ "registration": "absent" }
{ "registration": "stored", "recordId": "current-local-record-uuid" }
{ "registration": "unconfirmed", "unconfirmedAttemptId": "current-local-attempt-uuid" }
```

需要更新上游已拒绝但本地仍合法的 ready 时，管理员先检查，再显式确认提交 `replaceStoredRecordId`。它与 `retryUnconfirmedAttemptId` 互斥，必须精确匹配当前状态；陈旧确认在 GET 前拒绝，安装锁/SQLite writer transaction 内再核一次。GET 失败不消费确认；一旦准备发送，先写新的 replacement attempt。旧 receipt 原样保留在主表，overlay 最多每个既有绑定一条、受4096绑定上限约束。新 receipt 验证后，同一事务更新主记录并删除精确 overlay；任一 SQL 失败都保留旧 receipt 与 marker。

replacement 未确认期间，正常目录调用返回 `409/freebuff_device_registration_unconfirmed`，不使用保留的旧签名，也不 unsigned fallback。只能重新本地检查，再明确确认当前 attempt 的重投；不自动回滚或重注册。正常 UI 的确认绑定账号、站点、client、管理身份/Grant、revision 和当前记录对象；变更、dirty/pending、取消或卸载都隔离迟到结果。写请求未取得确认后只保留“检查设备”入口，关闭重开不自动 POST；停止等待不是撤销远端注册。

本次 UI 与后端分别使用合成 transport、production router/真实 SQLite/seal 验证；不是浏览器到真实管理 TCP 的联合验收，也不证明真实平台有效。详细范围见[状态/重新注册与 UI 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-device-ui.md)。

安装 key 与登录 fingerprint 分离，旧 access-key seal 文件/AAD/格式兼容，不 bump 本地 schema version。丢 seal 或坏设备 key fail closed，不自动换 key。它是同数据目录 AES seal，不是 OS vault；完整目录的读取者仍可能取得秘密。实现、证据与未验范围见[本地设备子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-device-runtime.md)。

### 5.3 稳定目录选择与普通账号保存

目录结果现在可勾选开放且符合普通模型 ID 规则的稳定 key，再经“确认目录模式”暂存到原账号表单。locked、未知或无效 key 不可应用。确认明确说明尚未接通公共目录模式、不购买会话；应用本身不提交配置，仍须保存账号。当前手动模型保留，目录来源的 fetched 集合由本次显式选择替换；后续 `supported_models`、provider 汇总与路由归属沿现有模型选择 owner 对账，不重写其他账号、其他 provider ownership、disabled route 或 aliases。

普通配置保存将非秘密目标写入 credential-only `extra_body.freebuffBusiness`：

```json
{ "origin": "https://codebuff.com", "client": "cli" }
```

只允许精确两个字段、已证 origin 与 CLI/Desktop；不接受 provider 继承、拼写别名、短效 handle/fetchId 或任意 URL。原登录 `base_url` 保持 `https://freebuff.com`，token/私有 receipt 继续由原 SecretPatch Keep 保存，custom metadata 不丢失。新增、改变或移除此账号业务目标时，正常配置 validate/commit 都要求有效的 Secret Grant；commit 在 publication lock 下对照当时 active revision 的目标，再走原 CAS。目标未变的模型/metadata Keep 不额外要求 Grant；数组重排按稳定 provider/credential ID 对账，删除整个账号不被当作授权转移。

保存该配置不等于会话消费授权。带此标记的普通 request plan 与旧 executor 在任何网络前返回 `409/freebuff_business_admission_required`，不会把目录 key 当 agentId、退回旧自动 session POST 或 unsigned completion。调用者请求体中的同名 runtime 字段被过滤，不能改写配置目标。

### 5.4 已保存模型的 signed 会话与余额检查

正常账号编辑现在提供“会话与余额”工具。仅列出原账号自己已保存的 `supported_models`，不借用 provider/其他账号模型，不用未保存目录勾选。管理员必须明确选模型并点击检查；打开、恢复 Grant 或重开弹窗都不发送。未保存修改、业务站点/client 与保存值不一致、权限或 revision 变化、取消和卸载会取消客户端等待并隔离迟到结果。

`POST /v1/internal/gateway/console/freebuff-session-inspection`：

```json
{
  "providerId": "freebuff-pool",
  "credentialId": "existing-account-id",
  "expectedRevision": "current-saved-revision",
  "modelKey": "explicit-saved-stable-key"
}
```

请求上限 16 KiB，未知字段拒绝。所有响应（含提取器/业务错误）`no-store`。不接受调用者 token、origin、client 或短效 handle；正常管理身份/Secret Grant/Origin 和原 revision 先校验，再从同一个不可变 snapshot 解析精确账号、原登录受众、credential-only 业务目标及账号模型。共享原授权 owner，不另建权限体系；返回前再次核 Grant/revision。

仅支持已有 LocalRuntime 精确 ready registration：无 Local owner 返回 `freebuff_device_storage_unsupported`，未注册返回 `freebuff_device_registration_required`，均不生成 key、不 bootstrap GET、不自动注册或 unsigned fallback。已有 unknown/replacement 状态沿原设备 owner 拒绝。成功路径受 discovery slot 和总计 20 秒预算限制，依次执行 signed catalog GET、围栏检查、绑定同一目录选择的 signed session inspect GET；不发 admission、heartbeat、end、completion 或后台轮询，取消/drop 释放 future/响应及 slot。

成功 DTO 返回精确身份、`modelKey`、本地 `observedAt`，并只导出以下展示快照：

- `session.status = none | active | ended`。active 校验完整必填字段、非负有限 remaining、合法日期及日期顺序，仅导出稳定模型和时间；instance/accessTier 经校验但不导出。ended 仅导出服务器是否携带 grace instance 的 `inGrace`。UI 明确标记活动/收尾会话“未认领”，不把它当本进程持有的 lease、新 prompt 权限或退款证明；不依据本地时钟推导服务器时钟容差或 claim 成功。
- `meter.state = absent | unavailable | available`，分别保留字段缺失、显式 null、合法余额块。available 仅解析使用的必要余额/价格字段，不复制整个权益 block；保留 `balance`、每日剩余、钱包余额、可选 claimable/discount/免问标记，缺少或格式错误的必要字段不会填零。
- `sessionPrice: null` 表示此模型未提供 Freebucks 价格，数值 `0` 才表示明确零 session price；二者都不是完整资格判断。按一次 `observedAt` 依时序应用已到时间的 `priceChanges`，再应用权威 recurring `offPeak`（UTC 日窗口、跨午夜、北京时间周末）；只修改原价格表存在的 key，不修改余额或已购买会话收费，不把 `soloDiscount` 预先减成实际扣款。列表价、小时和美元 cost 不混用。

显示的是**当次新 session 参考价，不是可消费 quote、锁价或 debit receipt**。不计算完整 legacy quota/intent，不认领其他进程会话，也不实现费用确认、持久 pending/unknown claim 或购买。原公共目录模式的网络前 guard 不变。当前验证分别覆盖生产管理 router→真实回环上游/SQLite 和正常编辑宿主→合成 API，不冒充浏览器到真实管理 TCP 的联合验收；详见[会话检查子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-inspection.md)。

## 6. 尚未闭合的范围

正常 UI 的新增/稳定账号重授权、同 Console 管理身份内的未知保存关闭重开、权限/原 revision、取消及材料兼容入口已做定向验证；新鲜真实管理 TCP/Local 持久保存与同目录 runtime 重建已有独立证据。未知保存状态仍只在存活组件的独立 Map 中，不跨页面/进程恢复；独立进程退出/重启、FreeBuff-specific Server/Worker Redis 与 PostgreSQL 边界仍需单独验证。Local runtime/SQLite运行和配置材料重建不等于SQLite凭据表级核验，也不能代替上述验收。

后续[业务 ownership 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-business-ownership.md) 已修同 ID 换 token/target 的 session/run namespace 串用、caller runtime 字段与伪造 instance 残留；普通 custom metadata 保留。两个真实回环场景仍只证明旧 wire 的修复，不代表 typed 登录到当前产品公共调用已经完成，也不声称重授权立即撤回在途调用。

新公开业务参考明确登录与业务origin可分离，独立[catalog/device子链](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-catalog-device.md)已实现显式固定业务目标/client、目录GET、有效目录后显式设备注册和signed GET；第5节接正常管理目录，第5.1/5.2节接安装key/registration持久owner、正常注册/状态/重注册与UI，第5.3节接稳定选择与业务目标的正常保存。最新独立[signed business wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md)已实现最终session/completion签名发送，其中 signed inspect 已由第5.4节正常管理调用，其他有副作用业务和公共执行尚未接通；没有将token自动跨host发送或改变原登录受众，保存目标不能解释成购买授权。

本批transport的`MaySpendFreebucks`只是显式marker，不是管理UI逐次费用确认、当前价格余额重验、持久pending/unknown claim或active业务准入；这些owner仍未实现。dedicated reuse缺完整请求合同不猜，旧未opt-in执行器的自动POST并未升级，目录模式仍在网络前拒绝而非回退。完整agent/run/lease/取消、正常Console精确probe、公共推理及模型/额度/权益继续，不用默认agent、零credits、Codebuff Key或广告Key凑完成。R06/W04、P50无凭据整包、P60集中录制、P70真实逐渠道验收保留。
