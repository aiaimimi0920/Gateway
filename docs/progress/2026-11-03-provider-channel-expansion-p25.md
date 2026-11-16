# 服务商分渠道扩展：第 25 批，Grok grant 接受与实例级会话

> 2026-11-03（Asia/Shanghai）；R05 / W03 / P20。协议和会话 owner 已实现，最终 v3 合计104项定向离线验证及代表性产品feature关闭编译通过；不等于正常 Console 登录、秘密保存或整个渠道 offline_ready。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或接管运行实例。

## 1. 来源与范围

延续 [P23](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p23.md) 的固定 grant transport 和 [P24](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p24.md) 的真实 ID-token 验签。本批区分授权码 end-user、Team、device 的保证级别，并新增同 Gateway 实例拥有的有界会话，不照搬旧 OAuth 的静态会话表或直接写配置保存链。

协议参考仍为官方 Grok Build `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`，Apache-2.0。官方 code-flow 只有 Team 可缺 ID token，Organization 不豁免；device 不具有 browser nonce。principal peek 是未验签 hint，不授予管理权限或证明独立额度。参考文件见 P23 证据，不安装、执行或复制参考项目。

另以无凭据 HTTPS GET 核实 [RFC 6749](https://www.rfc-editor.org/rfc/rfc6749.txt)，HTTP200；access token 通常 opaque，不要求 JWT。精确 URL、公共响应及 SHA-256 记录在 [本批证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p25) 的 `references.json`、`reference/rfc6749.txt`。

## 2. 已实现的协议 owner

- `grants.rs`：`complete_code` 先核对完整 client 快照和 callback，再实际 exchange；只有实际 access-token hint 精确为 Team 且 ID 完整时允许缺 ID token。其他 code-flow 仍执行 P24 验签，不将 consent 预选当强制 pin。
- `poll_device_receipt`：仅消费实际 poll 的 Authorized tokens，不伪造 nonce、签名身份或用户 subject；可选 ID-token hint 不升级保证级别。
- `principal_hint.rs`：先限 32KiB，再固定段数迭代，不分配任意分段 Vec。只有可解析、带 alg 的 JWT header 才解释 payload；点号本身不证明 JWT。明确 JWT 的重复字段、principal aliases 冲突和非法类型仍拒绝。这里只识别 hint，不以 alg 动态协商签名算法或假称 access token 已验签。
- `PrincipalConstraint`：与 consent 预选择分离；ID-only 约束在缺 type 时仍检查 ID，typed 约束要求 type/ID 同时精确一致。缺 hint 时约束失败，不静默放行。
- `receipt.rs`：公开无 constructor、Deserialize 或 Clone，receipt 仅从实际 grant acceptance 构造。保留完整 client、flow、tokens、可选签名身份及 hint，Debug 固定脱敏。响应后、JWKS 等待前记录 monotonic lifetime 与 wall expiry；身份验证结束后再次拒绝已过期 access receipt。

`SignedEndUser` 只表示 P24 所验用户 ID token；`TokenEndpointOnly` 只表示固定 token endpoint 交付。两者都不是管理授权、保存成功、业务调用成功或配额证据。

## 3. 实例级会话、资源与退出

- `src/console/grok_oauth/{mod,session,registry}.rs`：Binding 精确绑定 owner/provider/revision；它本身不执行管理鉴权，也不证明已读取真实 provider/preset。
- 每实例一个 registry、最多16个活动会话、600秒 ceiling；没有静态 session map。回调在锁下单次消费，并发第二个 callback 拒绝；领取 receipt 也只允许一次，状态为 `Claimed`，不冒称导入成功。
- watch 取消与总 deadline 覆盖 exchange/JWKS；future drop 的 ExchangeGuard 把 Exchanging 关闭为 Failed，取消/过期清空 auth 和 receipt，不留永久 busy。
- 每实例只有一个幂等 spawn 的每秒清理 worker，以 Weak 引用避免环；不依赖查询触发过期。明确 shutdown/join，Drop 兜底取消并 abort worker。
- `GatewayLifecycleState` 保存 registry；`spawn_background_tasks` 接同实例 shutdown handle。HTTP 正常结束后 request shutdown，再等待 Grok registry、token refresh，之后关闭本地 SQLite。

`Registry::join` 只等待其清理 worker，并向 caller-owned exchange future 发取消；不收集外部 caller 的 task handle。未来正常 HTTP/commit owner 仍必须负责自己的请求和持久化任务，不能把本批当成全部登录 task 已 join。

## 4. 实际 red 与最终验证

证据均在本批目录，后端使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，不触发默认 management-ui 发布/清理 hook。

| 验证 | 实际结果 |
| --- | --- |
| 首次 grant red | `red-grants-v1.log` exit101，缺 complete_code/GrantAssurance；是缺实现红测，不冒称旧生产运行漏洞 |
| 首版 grant/session green | `green-grants-v1.log`：44 pass / 0 fail / 0 ignore，编译10m52s、运行15.10s；是 v1 来源，不与后续重复累计 |
| opaque token red | `red-opaque-v2.log`：0 pass / 1 fail，实际 `opaque token must remain usable: Err(InvalidResponse)`；编译9m40s，运行1.03s，cargo exit101 |
| 最终 v3 | `source-frozen-v3.json` 在编译前冻结13文件；`green-grants-v3.log` exit0：48 pass / 0 fail / 0 ignore，编译9m06s、运行15.07s |
| 相邻回归 | `refresh-adjacent-v3.log`：16 pass；`call-flow-adjacent-v3.log`：40 pass；均exit0，无fail/ignore。三个过滤器不重叠，`audit-results.mjs v3` 实际解析合计104 pass，不累计首版或red重跑 |
| 产品feature关闭编译 | `feature-disabled-v3.log` exit0，1m44s；`cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib`；仅保留Gemini，不冒称全feature-off矩阵已解决 |
| 最终静态 | `run-static.ps1 -Version v3` exit0：Rustfmt、checker31/31、无报告ratchet 4055文件/14 immutable/governed source >700=0、tracked whitespace通过；不覆盖原有脏报告或调整baseline/豁免 |

v3 实际覆盖 dotted opaque 的 device 路径及正常 signed code 路径、两条公开 acceptance 的 ambiguous principal、验签成功期间 access receipt 到期；会话覆盖精确 binding/容量、单次 callback/claim、wrong state、取消/drop、并发 callback、deadline、shutdown/join和无人查询时 TTL 清理。业务HTTP全部限随机loopback和合成材料。现有29条test-profile warning未越界处理，新owner无对应warning；未运行全仓回归或正式release gate，以上编译耗时不是EXE或包构建速度。

独立只读复审已检查 receipt/principal 和 registry/session/runtime 的首版范围，以及opaque修补与两项补强测试，未发现可确认阻断；未运行测试，不算动态通过。已主线程核对runtime/state/console模块diff只有本批接线。

`source-before.json`在本批写源码前保存；v1/v2/v3分别记录首版、opaque red与最终来源，不编译中改源码。最终`audit-final.mjs v3`核对13个源码hash以及源码/五份文档的UTF-8无BOM、本地链接/围栏及含未跟踪文件的独立whitespace，结果见`source-doc-final-v3.json`。

有效行（本批前→v3）：protocol root118→125、tests174→175；新增 grants59、principal_hint122、receipt89、grant tests315、Console mod49/session210/registry142/tests295；既有 console/mod41→42、state185→186、runtime362→367。两个较长测试文件各自围绕 grant acceptance 和会话生命周期，均小于500；未改 baseline/豁免或新增依赖。

## 5. 明确未完成及下一步

1. 正常 HTTP 管理路由、Secret Grant、创建时真实 provider/preset/expected revision 实查，不能只相信请求传入的 Binding。
2. device 会话 registry owner、自动 loopback callback listener、正常 UI 登录操作。
3. 普通 SecretPatch/CAS 保存、caller drop 后提交 ownership、refresh generation/rotation/冲突保护和重启恢复。
4. 独立 Grok OAuth preset/模板、实际 Responses 调用及完整三适配器集成；客户端使用授权边界仍需核实。
5. 继续上述正常管理链，而不是将 grant receipt 当作功能完成；本批收尾不缩小总目标。

独立P00调查还补充了 [Perplexity Web公开合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-perplexity-web-contract.md) 和12份公共参考的URL/SHA索引。这里只消除部分来源缺口，不是新增生产实现或R12完成证据。

所有 R01–R16 及同平台 API/Web 分支继续保留；Qwen 原 OAuth 停用约束未决，unknown 不合池，Web不是兜底。其他 OAuth/API/Web/媒体、Qwen/Bedrock剩余保真与PG边界、留存前端/feature/expiry问题、P50包与隔离启动、集中录制及逐渠道真实验收均未由本批关闭。总goal保持active。

未读取真实.ng、历史凭据或在线数据库；未发真实授权/刷新/推理/生成请求、产生费用、构建EXE、部署或重启在线实例。
