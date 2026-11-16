# P44：Accio 正常管理 HTTP 与服务端保存

日期：2026-11-04（Asia/Shanghai）。本批服务端子链已完成定向离线验证；正常模板/UI 尚未接线，R01/W01 和总目标仍在执行，不新增整卡 `offline_ready`。

## 1. 已实施的服务端子链

新增 `/v1/internal/gateway/console/accio-auth-sessions` 的 create/status/complete/cancel/import。复用现有管理鉴权、Origin/IP/期限绑定的 Secret Grant、普通 redaction/SecretPatch/keep 保护和原 revision/CAS 配置提交；没有第二套刷新 scheduler、配置数据库或自动浏览器/listener。

- 真实保存 provider 必须为 `preset: accio`，实际 compiled adapter/base/path 精确匹配；编辑还校验稳定 credential ID 与其真实覆盖。不按 token、邮箱、显示名或数组位置合并账号。
- 显式 client、China/Global 固定授权站点与非零 callback port；只生成固定 loopback `/auth/callback`。手动完整 callback 消费一次，管理身份与 PKCE/state 分离；没有 OIDC principal/已验证账号身份的虚假声明。
- utdid 新增必须显式输入；重授权可明确省略以保留该 credential 自己已配置且无歧义的 device。拒绝空白/控制字符/超界，不生成 device，不借 provider 的共享设备替代。
- token、refresh、SG、device、code 和 verifier 留在服务端，不返回 token-bearing draft。正常 `SecretPatch` 保存 access/refresh/credential-only SG，绑定固定 refresh endpoint 与显式 client。
- 原绝对 expiry 直接 RFC3339 保存，保留纳秒，不按保存时间重授 lifetime；已过期 access 的成功材料仍完整存入，并由现有 refresh readiness 拒绝业务 dispatch。
- 新登录不证明与旧账号相同，因此新 grant 缺 SG 时明确 clear 旧 SG；这不改变 P43 refresh 缺 SG 时保留旧值的语义。目标自身已知旧认证副本明确清理，其他账号与 provider 不被清空。
- 每实例最多 16 会话、600 秒登录 TTL、一个 expiry worker；首次接纳保存后固定目标 ID，受跟踪 JoinSet 独立于 HTTP waiter，20 秒异步预算包括 publication wait；正常退出 join 已接受保存。只在真实 normal commit 成功后报告 `succeeded`，错误/timeout/drop 均 `save_unconfirmed`，重复 import 不重投。

详细接口见 [Accio 管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-accio-oauth-login.md)。存储格式没有新增字段、版本提升或启动数据重写。

## 2. 已执行的行为证据与审查修补

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p44`。

1. `red-http.log`：生产未改时实际 0 passed / 1 failed，正常管理入口为 404，而非预期 200；不是编译错误冒充行为红测。
2. `check-initial.log`：首版直接读取 private `RouteConfigInner.providers` 引发 E0616；已改用现有 pinned snapshot getter，不扩大生产可见性。
3. `green-accio-oauth.log`：首版 36 项定向 Rust 测试通过，包含实际管理 router、合成回环 JSON exchange、normal commit/归档、稳定 ID 重授权、取消/drop、原 expiry 与 SG。
4. 两个独立只读复核指出 provider 已知认证材料的继承缺口。`red-provider-inheritance.log` 实际 0 passed / 1 failed：provider Cookie 被准入，返回 200。已在 target 准入拒绝 provider 的已知认证、Cookie 派生与 device 副本，不清共享 provider，不影响其他账号；源代码窄复核已确认阻断链。
5. 后续自查确认 `empid/tenant` 来自旧 Cookie/账号上下文，也会由 generic merge 随新 token 发送，不能把它们当普通 operator metadata。首轮管理组合实际 1 passed / 2 failed，其中 provider tenant 被准入为 200，另一个是新增夹具在 drain 后申请 Grant。现已在新授权专用边界移除目标的 `empid/empId/accountId/account_id/tenant/tenantId/tenant_id`，provider 同类副本则拒绝；独立源码复核确认封住原继承链。最终重授权合同只保留真实非账号字段 `iaiTag`，并分别验证 provider 七别名，已实际转绿；没有修改旧手工配置或普通 packing 的普遍语义。
6. 新增“无 runtime / drain”夹具修正过程中，又暴露了错误的 Arc 独占假设（v2b）及对 drain 误期待 400（v3）。已亲读真实 Grant handler 与全局 drain middleware，最终直接使用正常 HTTP Grant、先授权后 drain，并断言 503 及各自精确错误码 `console_mutation_not_supported` / `gateway_draining`。没有为测试改变生产鉴权或 drain 语义；失败日志和各版源码冻结均保留。
7. v2 显式启用进程级 `CARGO_INCREMENTAL=1` 后，重编发现既有 BoringSSL 生成输入指向已不存在的临时目录，报 `os error 3`，不是业务测试失败。已用项目 `tools/prepare-cpa-bssl.py` 的固定归档/摘要校验恢复原缺失目录；未清缓存、升级依赖或改构建源码。恢复记录见 `build-input-recovery.json/.log`；后续成功过程保留环境参数，不从不同源码/机器负载的耗时推断加速倍数。

### 2.1 最终通过的检查

表内 Rust 测试命令使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，测试线程数 2、`RUST_MIN_STACK=16777216`；没有启用 `management-ui`。代表性关闭编译另从 runtime-defaults 展开的 feature 列表移除 Accio leaf，不同时打开 runtime-defaults。测试临时目录均显式落入本批 `linshi`。最终有效证据如下，不能把重叠套件相加：

| 检查 | 实际结果 / 日志 |
| --- | --- |
| Accio OAuth | 40 passed；`v2b-green-accio-oauth-final.log` |
| Accio 相关 | 110 passed；`v2b-regression-accio.log` |
| Console | 143 passed；`v2b-regression-console.log` |
| Token refresh | 111 passed / 4 ignored；`v2b-regression-refresh.log` |
| Routing config | 119 passed；`v2b-regression-routing.log` |
| 四个正常管理 integration target | 10 passed；`v4-green-management-and-regression.log` |
| 代表性关闭 Accio leaf 的库编译 | exit 0；`v4-feature-disabled-check.log`，不是完整 feature matrix |
| 有效行检查器 | 31 passed；`v2-checker.log`；checker 本身未变，未重复执行 |
| 最终 ratchet / scoped Rustfmt / diff | 见 `verify-v4-post-exits.json`、`final-audit.json` 和 scoped diff |

按 target + 测试名去重共 **461 个不同 Rust 测试通过，其中本批新增 18 个**，明细见 `test-summary.json`。四个 ignored 均为既有的显式 disposable Redis 合同，未在本批新执行，不是四项通过。v2b 之后只修正一个新增 integration 夹具；其余 **19 项本批 Rust 输入摘要未变**，因此复用其已通过库测试，管理组合由 v4 重新执行，没有无故重跑全部套件。

### 2.2 源码、存储与验证边界

本批共 20 个 Rust 文件（7 个既有、13 个新增），加 5 份文档；最终 UTF-8 无 BOM、冻结摘要和 scoped whitespace/diff 审计见 `source-frozen-v4.json` / `source-final.json` / `final-audit.json`。既有有效行变化依次为：console mod 43→44、state 186→187、runtime 367→372、management router 302→316、internal console 401→403、协议入口 110→114、transport 131→136。新增文件最高 256，既有变更文件最高 403，没有新增 501+ 文件或 grandfathered debt 增长。

工作树原始状态清单与最终状态清单分别保存在证据目录，状态差异包含并行的 AccessKey/ProviderStorage/样式工作，不能全算作本批；没有接管、清理或回滚这些差异。没有提交、推送、出包、替换在线实例或读取 `.ng` / 历史凭据。

真实 Redis Lua、本机进程外强退、真实平台/凭据、第三方 client 许可和打包交付均没有由本批证明。会话仍不跨 Gateway 进程重启，不把目标 ID 存在当保存成功；China/Global 和旧导入之间的产品/额度关系仍 unknown。正常模板/UI 及精确业务/probe/额度仍需继续，不用服务端子链或定向测试冒充整卡完成。

## 3. 后续正常模板/UI 接力（P45）

1. `accio-oauth` 新建入口继续使用兼容 `preset: accio` 和固定 gateway，不因 OAuth 获取方式另造相同受众池；空 credentials、无默认账号/模型/路由，不制造余额。China/Global 的 quota 关系仍 unknown。
2. Accio 专用 API 输入是 provider/revision/可选稳定 credential、显式 client/site/port/utdid；session view 没有 Grok `identityVerified` 或 principal。固定 API root、请求预算、provider/session/revision/target 响应绑定必须保留。
3. 复用 `CredentialDialog` 的 `renderAuthentication/renderReauthentication` 和 OAuth 独占保存接缝：后端保存成功后关闭/刷新，不再把材料交普通 Key 表单重复提交。旧账号 metadata/模型编辑保留，进入重授权前阻止未保存表单修改。
4. 窄复用 Grok 已有前端 operation/generation/有界串行轮询机制；若提取共享部分，仅抽出两产品真实相同的 HTTP/session 生命周期，不复制 OIDC 语义，也不重写 ChatGPT/Antigravity。
5. 接纳保存的非秘密定位记录按 client/management 身份隔离，并与 Grok 分离；关闭重开只继续查同一 session。revision 变化停止新 mutation，不丢已接受保存；Grant 恢复只续接原显式操作；未知保存不重提，迟到回包不得复活已消费记录或覆盖后继会话。
6. 显式参数、手动 callback、SG 缺省清旧值/旧账号上下文替换、无 listener、quota unknown 等语义如实展示。callback 密码输入、提交后清空；授权链接只允许两个固定 login origin/path，使用实际 Neuro primitives，单一主动作。
7. 新 UI 的红绿测试、真实全局 Dialog 新增/重授权接线、权限/revision/uncertain/卸载重开、普通 metadata 保存隔离、模板空池与产品隔离、typecheck/隔离 Web build/可行宽窄屏检查分别记录。不能用合成组件截图冒充真实登录。
8. 精确 Accio 业务/probe、旧导入副本归一化、真实客户端使用边界和集中账号额度仍单独推进。其他原定渠道以及 P50/P60/P70 全部保留，不因本批后端保存完成而缩小目标。
