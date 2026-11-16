# P49 后续增量：Kiro 管理设备授权与普通保存链

本文件只记录正常 Console 管理登录后端子链，不是总计划 P50“无凭据集成完成门”，
也不代表 R02/W02、Kiro 新模板、profile/subscription 或真实产品验收完成。
本子批定向离线验证已通过；后续 UI 接线见同日 [P49 UI 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-ui.md)。

## 范围与实现

- 新增 `console::kiro_oauth`，每个 runtime 独立注册表：最多 16 个会话，原始 TTL 600 秒，
  1 秒清扫。终态凭据保存回执有独立 300 秒保留窗口；不新增逐账号常驻轮询或刷新器。
- 新增管理入口 `/v1/internal/gateway/console/kiro-auth-sessions`，以及按 ID 的 GET 和
  `poll/cancel/import` POST。create/action 分别限制 16 KiB / 1 KiB。
- create 显式提供 directory 和 scopes；Builder ID 与 IAM Identity Center 使用既有
  `Directory` 合同，不接受任意 issuer、token endpoint 或借用其他产品 client ID。
  后台自行 public registration，再取得设备 challenge，HTTP 返回后由 registry 跟踪其生命周期。
- poll 是显式单步操作。发送前 pacing、pending 和 slow_down 沿用已有协议；并发重复
  poll 只读取当前 Polling 状态，不准入第二次发送。网络取消或不明确失败后不重放 device capability。
- 所有修改重新核验管理身份、Origin 和 Secret Grant；owner 仍由管理 token 指纹、Host、
  client IP 组成。用户 code、device code 和上游 token 不提供管理权限。
- 新凭据要求操作员显式 machine ID，沿旧纯算法规范化；重授权只接已有 typed
  `kiro_oauth`，保留其固定 machine ID，不接受不相等的显式替换。
- target 检查真实编译后的 Kiro payload 和既有业务边界。新目标不允许共享 provider bearer；
  原 revision 在准入、准备保存和最终普通 CAS 各自保留。已接受的网络操作不会在 pacing 或
  registration 返回后逐次重读配置，但迟到 token 不能越过 import 的 revision 检查或覆盖新配置。
- 服务端材料走既有 redaction → SecretPatch → resolve → validate → publication lock →
  `RouteConfigRuntime::commit_document`。不返回 token-bearing UI 草稿，不写第二套数据库。
- 新授权缺 refresh token 或 access expiry 时清旧值，不把未返回的新材料解释成“沿用旧账号”。
  canonical bearer `session_auth.expires_at` 同步为新的绝对 expiry 或 unknown，避免继承旧 nested/provider expiry。
- 保存承认与 HTTP waiter 分离。成功才返回 Succeeded；冲突、超时、丢弃保存 future 为
  SaveUnconfirmed，保留目标 ID，不自动重新提交材料。取消不撤回已承认的保存。
- runtime 的原后台启动和关闭接缝负责 spawn/shutdown/join；所有 registration/poll/save
  均在 registry 的 JoinSet 内。意外取消由 capability/save guard 关闭准入并清材料。

## 验证记录

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-console-20261104`。
只用合成凭据、随机回环 HTTP、既有 Console fake Redis；没有请求真实 OAuth、真实 Redis、
浏览器或在线实例，没有使用任何历史 Redis 清理授权。

1. `red-admission.log`：真实管理 router 的两个测试均失败，分别为实际 404 对预期 401/400，
   证明原来没有 Kiro 管理入口。0 passed / 2 failed；不是编译失败。
2. `green-lib-v1.log` 是主动取消的编译，不是通过或测试失败。实际 rustc 警告揭示新 future
   只捕获 `guard.active`，可能让整个 Drop owner 提前析构；因此按已核对 PID/父子关系只停止
   本次 Cargo 和其 rustc 后代，进程/原因保存在 `cancelled-v1-*`。不因等待时间盲目重启。
   修复采用现有 Accio 的 `guard.disarm()` 方法，强制 future 捕获完整清理 owner。
3. `green-admission-v2.log`：2 passed / 1 failed。真实入口和 body limit 已通，但错误响应缺
   `Cache-Control`，不是源码编译失败。只为 Kiro 的三条管理路由补 response middleware，
   连同 extractor/body-limit 错误统一 `no-store`；没有改全局 GatewayError 或其他产品路由。
4. `green-lib-v3.log`：39 passed / 1 failed，正常注册、单步 pending/slow_down、完整保存、
   稳定 ID 重授权、IdC directory 与业务 Region 分离、取消/丢弃/超时及 CAS 均通过。
   失败为非法目录输入；`serde-directory-probe.log` 独立证实 Serde 的 internally tagged unit
   variant 会忽略额外字段，即使枚举已标 `deny_unknown_fields`。改为空 struct variant 后才拒绝。
   原 unit variant 的字段也没有用于出站 URL，不是接受攻击者 token endpoint 的证明。
   补直接解析测试、真实 router 422/no-store 测试和 case/status 诊断，不改宽业务语义。
   独立探针第一次误选历史 rustc 1.95 的 serde_json 缓存而报 E0514；保留 build 日志，换用
   现有兼容的 1.98 库后探针通过，没有清缓存、升级或重装依赖。
5. 原 v4 wrapper 以 `1073807364` 停止，`green-lib-v4.log` 只有编译警告，没有测试结果，也没有
   `regression-v4-exits.json`。恢复前查实无 Cargo/rustc 残留，20 项源码均匹配 v4 输入摘要；
   `interrupted-v4-observation.json` 保留该证据。不推断终止原因，不把中断算通过，不清缓存或升级依赖。
6. `green-lib-v4-resume.log` 为 41 passed；管理准入 4 项、Kiro refresh 13 项、相邻 Console/context
   85 项通过。关闭 Kiro 的编译成功，但运行时 5 项都在夹具装载失败：编译关闭已移除 `kiro` preset，
   原夹具不能加载。只将关闭模式夹具改为空 Console，以实测 feature gate 在 target lookup 前拒绝；
   生产准入和 preset 集合没有放宽。`green-admission-v5.log` 为 4 passed，
   `feature-off-admission-v5.log` 为 5 passed，包含独有的关闭模式拒绝测试。
7. `verification-summary-v5.json` 按 suite + test name 去重：主配置 **143 passed**，其中本批新增
   **20 passed**；另 1 ignored（需要显式导出的合成前端 session fixture），不计通过。关闭模式 5 passed，
   其中 4 个名称在主配置中也执行；跨配置新增唯一名共 21 个。不是全仓或全 feature matrix 验证。
8. `source-final-v5.json` / `audit-source-v5.log` 冻结 20 项 Rust。相对 v4 只更新了上述集成夹具，
   19 项生产/其他测试输入不变，复用已成功的 lib 和相邻回归，不重跑慢编译。全部 UTF-8 无 BOM、
   有效行不超过 500；新增最大 306，接线文件最大 405，准入集成测试 98。Rustfmt、ratchet、
   diff 和源码核对最终通过；checker 的 31 项测试复用未变化的 `checker-tests.log`。

## UI 接线前的 owner 注意事项（后续子批已承接）

`apps/desktop/src/api/console/managed-auth.ts` 只共享 transport，产品各有 schema；
接线前 action 只有 complete/cancel/import，后续 UI 子批才增加 poll 传输和设备策略准入。
`useManagedOAuth.ts` 的定时检查始终是 GET 状态，不是设备授权 POST poll。
Kiro 没有照抄 Accio callback 流程或把 GET 当授权推进。

可复用 Accio/Grok 的当前 Console 挂载期 receipt Map、稳定 credential ID、原 revision、
显式 import、未知保存只查同一会话以及关闭重开机制；它们不是跨应用重启的持久恢复。
后续 UI 子批已由主线程亲读这些 owner 后接线，不创建虚假模板，不把 opaque token 或保存回执
称为 profile/subscription 已验证。新建仍需显式 directory/scopes/machine，重授权设备沿用服务端。

create 请求形状如下；scope 只是 wire 示例，不是已经证实的当前最小权限或 UI 默认值：

```json
{
  "providerId": "已保存的 kiro preset 池 ID",
  "expectedRevision": "当前已保存 revision",
  "directory": { "authMethod": "builder_id" },
  "scopes": ["codewhisperer:conversations"],
  "machineId": "操作员显式提供的设备身份"
}
```

IdC 改为 `{"authMethod":"identity_center","directoryRegion":"eu-west-1",`
`"startUrl":"https://d-example.awsapps.com/start"}`，输入 URL 仍由现有合法 AWS Start URL 规则校验。
重授权增加 `credentialId`，machineId 可不传，传入则必须与已保存 typed context 规范化后相等。
所有 action 的 body 只有 `expectedRevision`；GET 只读状态，不会推进上游授权或自动保存。

## 尚未完成的边界

- 本子批只形成后端管理合同；UI 入口和当前 Console 挂载期 receipt 已由上述后续子批承接。
  新模板、跨进程/应用重启的管理 receipt 恢复仍未完成。
- 旧无 typed context 的 route/DB 自动升级；P49 folder ingress 不等于全量迁移。
- 当前闭源 Kiro 的最小 scope、profile 发现/选择、订阅权益 exact HTTP、当前 client/真实产品、
  API Key/CLI/ACP 渠道取舍及额度验证，继续按 Kiro 调查合同推进。
- 全计划仍保留其他服务商、媒体/Web 生命周期和 P50/P60/P70；没有据此新增整卡
  `offline_ready`、真实凭据验收或日常/正式发布包。
