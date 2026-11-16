# P47：Kiro AWS OIDC 协议底座与旧误路由保护

日期：2026-11-04（Asia/Shanghai）。状态：本批实现、主配置及代表性 feature-off 定向验证通过。R02/W02 与总目标继续 active；没有新增整卡 `offline_ready`，新增模板记录仍为 12。

## 1. 本批范围

- 新增独立 [kiro_oauth.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_oauth.rs) 与按职责拆分的 directory、registration、device、material、transport；没有修改原 Kiro assistant wire，也不借用其他应用的 clientId。
- Builder ID 使用固定历史官方参考的 directory/start URL；Identity Center 显式输入 directory Region 和组织 Start URL。34 个商业 OIDC Region 固定准入，发送标准 `oidc.{region}.amazonaws.com`，不从 profile/API Region 推断授权 Region。
- 调用方明确选择非空、不重复的 Kiro scopes，自注册 `clientName:"Neuro Gateway"`、`clientType:"public"`，仅声明 device/refresh grants。注册 client secret 有独立绝对到期时间及单调 deadline，发送前后检查，不把 access expiry 当注册期限。
- device 是可变、单步、paced capability：缺 interval 按 RFC 使用 5 秒，显式零拒绝；设备 TTL 从请求开始计、最多 600 秒且受注册期限限制。只有明确 HTTP 400 的 `authorization_pending/slow_down` 才允许下一次 poll；slow_down 增加 5 秒，不延长期限。发前等待中取消尚未发送，发后取消或不明确失败即退休同 capability，不重放设备码。
- JSON refresh 消费 owned grant，无自动重试。token 材料保持 opaque，不制造已验证用户身份；access expiry 从请求开始时刻加真实 expiresIn，缺省保持 unknown，不补 3600 秒。迟到响应不重建寿命；后续持久化接线仍须尊重原绝对时间。
- HTTP 不继承默认认证头、不跟随 redirect、不采用响应自报 endpoint；单次 send+read 共用最多 15 秒预算，响应按解码后字节限制 64 KiB。直接 typed serde 拒绝重复安全字段，错误为固定枚举，Debug 脱敏，不回显私密 body。

网络受已有 `line-kiro-official-vendor-api` 控制，关闭时拒绝注册发送，纯解析仍可用。没有新建后台轮询、scheduler、持久存储或平行授权生命周期。

## 2. 过渡 gate 与产品边界

现有通用 refresh sender 使用 form/snake_case，不能表达 AWS registered-client JSON。三个实际红测复现了 Kiro 进入 generic form、Kiro 接受外部 endpoint、AWS OIDC 伪装普通 OAuth 的旧路径。

本批在 [grant.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/token_refresh/grant.rs) 的 prepare 阶段检测 Kiro canonical adapter 或 AWS OIDC host，返回 recoverable `kiro_oauth_refresh_context_required`，在现有 scheduler 的 reserve 前阻止错误发送；不将 credential 标为永久失效。这是**临时保护，不是刷新功能完成**。

必须先把 registered secret、认证方法、directory Region/start/scopes/注册 expiry 绑定到 credential-only typed context 和 durable grant identity，再接原 prepare→reserve→execute→Result→authority→cache，替换 gate。不能只从可变 extra_body 临时拼 JSON 就绕过永久 grant 身份。

本批未提供 Console 登录/模板、持久 receipt、profile/subscription 发现或普通账号刷新接线。最小 scope、profile/ARN 与业务 Region、当前 Builder ID verification URL 真实样本、客户端使用与额度仍分别未证。登录 URL 准入是本地固定安全策略，不把 Builder ID start URL 当实际 verification URI 证明。

## 3. 红绿与失败归因

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p47`。

1. `baseline-files.json` 与 `before/` 保存六项既有 Rust/文档原始字节。`red-guard.log` 首次因测试 module 遮蔽 `transport::RefreshError` 而编译失败；只改为完整类型路径，不把该错误算行为红测。
2. `red-guard-v2.log` 编译 9m50s，真实结果 0 passed / 3 failed。`red-source-verification.json` 核对红测时 protocol 注册和 grant 生产输入与 before 原字节一致；其后才接本批生产模块和 gate。
3. 第一轮 `green-kiro.log` 编译 13m33s，74 passed / 2 failed。三项 guard 转绿；一项准确暴露 rquest 自身 deadline 抢先时被归为 Transport，现将 send/read 的底层 timeout 与总预算统一分类为 Timeout，不放宽测试。
4. 另一失败在 server 收到请求后立即取消，却要求 client 已持有响应体；原 fixture 不能证明这一阶段。现用仅 test 编译、task-local 的 client body-ownership observer 同步，再证明取消释放体；另保留发送后、headers 前取消不复用设备码的独立测试。没有用任意 sleep 掩盖时序，生产没有该 observer 或新后台任务。
5. `green-kiro-v2.log` 编译 6m20s，77 passed / 0 failed。`source-frozen-v2.json` 冻结这轮 Rust 输入，后续回归与来源审计核对相同字节。所有既有失败日志保留，不把不同缓存状态的编译时长称作性能倍数。

## 4. 验证状态

主配置合计 **272 个不同 Rust 测试通过，其中本批新增 23 项**；4 个真实 Redis opt-in 未启用。关闭组合另有 5+3 项通过，其中仅 `compiled_out_line_rejects_registration_without_network` 是主配置没有执行的测试名，其余不重复累加。`verification-summary.json` 按完整测试名去重，三项 guard 在 Kiro 和 refresh 两轮中的通过也不重复累加；红测及首轮失败不进入最终绿灯计数。

| 检查 | 实际结果与证据 |
| --- | --- |
| Kiro 协议、旧业务与误路由保护 | `green-kiro-v2.log`：77 passed，Cargo exit 0 |
| 共享刷新生命周期 | `regression-refresh.log`：112 passed / 4 ignored，exit 0；没有启动 Redis 容器或把忽略项算通过 |
| 路由 refresh 材料/交接 | `regression-routing-refresh.log`：9 passed，exit 0 |
| 相邻 Accio/Grok OAuth | `regression-accio.log`：25 passed；`regression-grok.log`：52 passed，均 exit 0 |
| 代表性 Kiro line 关闭 | `feature-disabled-material.log`：5 passed；`feature-disabled-guard.log`：3 passed，均 exit 0；仅从 runtime-defaults 去掉 Kiro line，不代表全 feature-off 组合 |
| checker | `checker.log`：31 passed，exit 0；checker 实现未变 |
| formatter / ratchet / diff | `formatter.log`、`ratchet-final.log`、`diff-check-final.log`：exit 0；ratchet 4258 files、14 immutable runtime artifacts、governed source >700=0 |
| 最终来源冻结 | `verification-summary.json`：16 项 Rust 输入与最终 green 前的 `source-frozen-v2.json` 逐项 SHA256 相同；文档补齐后的本批 UTF-8/BOM/whitespace/有效行和 scoped diff 记录在 `source-frozen-final.json` / `audit-final.log` |

后端 Cargo 均显式 `--locked --jobs 4 --no-default-features`，主配置使用 `runtime-defaults`，不触发默认 management-ui 发布/清理 hook。代表性关闭组合本次首次编译 32m44s，后续同组合 guard 准备 5.62s；期间观察到其他项目 Rust 编译进程，未停止或调整他人进程，不将此时长视为独占性能基准或提速倍数。未运行全仓回归、真实上游或正式发布门禁。

测试使用随机 loopback 和合成材料，覆盖精确 register/device/refresh JSON、默认认证头隔离、scope/start/Region、注册过期、pending/slow_down/denied/expired、发送前节奏、headers 前与 body 中取消、redirect、分块/gzip 解码预算、单一发送读取期限、重复字段和错误脱敏。源码只读复审未发现指定 wire/目标/期限/发送后重放问题；该复审不是新增运行证据。

本批既有 Rust：protocol mod 98→99、grant 128→146、refresh tests 210→211 有效行；13 项新增 Rust 最大 243 有效行，无新 501+ 例外、baseline 重写或 oversized debt 增长。

## 5. 后续接力

1. 设计并测试 credential-only typed Kiro 授权 context、完整秘密分类/SecretPatch/Keep、durable grant identity、旧格式兼容与冲突拒绝，然后接现有刷新 publication/replay/cache；不绕过原 revision/CAS 或跨实例发送证据。
2. 处理旧 kiroRefreshToken/AccessToken/AuthMethod/ClientId/ClientSecret/AuthRegion 等副本与标准字段归一化，稳定保留显式 machine ID；refresh 轮换不能继续发送旧副本或隐式更换设备身份。
3. 接有界、可取消、可恢复的普通 Console 登录/材料保存和 UI，再按证据补 profile/subscription 发现、选择及精确账号业务测试。固定旧官方 SDK 的有限 profiles/无 profileArn 证据见 [调查合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-kiro-oauth-contract.md)，不外推当前服务端权限。
4. 原有 Accio/Grok 未闭合项、Qwen/Vertex 未决认证、其他 API/Web/媒体、P50/P60/P70 继续；仍按用户要求先无凭据开发，最后集中录制和逐渠道真实验收，不删减原始范围。

没有读取 `.ng` 或历史凭据、访问真实账号/模型、消费付费额度、创建/删除容器、出包、替换运行实例、提交或推送。未撤销其他并行改动，未接管 S06 Rust/Gemini lane。
