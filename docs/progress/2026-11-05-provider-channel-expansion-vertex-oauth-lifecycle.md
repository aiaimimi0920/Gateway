# 服务商扩展：Full Vertex credential-only 材料、持久刷新与最终发送

日期：2026-11-05。对应总计划 R11/W07，接续 [typed context/目标子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-context.md)。

## 本批范围与完成边界

本批将既有独立 Google desktop OAuth context 接入真实配置/秘密管理、既有刷新 scheduler、durable grant/publication/replay/cache，并为 typed Full Vertex 重建最终 Bearer。
继续使用现有 Gemini 编码器、DirectHttp、原 revision/CAS 和持久化 owner，不建立第二套 scheduler 或秘密存储，不修改 S06 共享 Gemini 内部实现。

**这不是正常 OAuth 管理登录、UI、公共 router/结算、完整响应保真或整卡完成。**
正常配置与秘密处理 API 可以承载新字段，但尚无专用管理 listener/session/create/status/cancel/import 和对应新模板。
累计新增模板仍14；不新增整卡 `offline_ready`，不标 R11/W07、P50 或总计划完成，不要求现在录制真实凭据。

## 实现

### 1. 材料、受众与秘密边界

- `ProviderCredentialYaml.vertex_oauth` 是 credential-only typed context；DB 合并先剥除父级同名 context，不能继承或递归拼出 grant。
- 独立 `vertex_user_oauth` opt-in 保留旧 Full Vertex aliases、旧 `auth_token`/custom-path 配置的原语义。没有显式身份的旧配置不被自动转换。
- 原声明和最终 payload 均绑定 Full Vertex 产品、明确 client/scope、project/location/quota-project、固定 token endpoint 与业务目标；禁止跨产品 preset/profile/flow、父级 access 继承、混入其他 OAuth context、legacy `auth_token`、任意 header/path/extra-body 和发现结果覆盖。
- optional client secret 接入原 nested SecretPatch、descriptor、redaction、明文 draft 拒绝与 Keep；access/RT/client secret 的 Keep 比较完整 resolved destination/consumer。模型和 label 编辑不是新授权受众。
- flattened hosted/cache ingress 验证 context 和产品，但运行 payload 不携带 client secret 或 refresh token；quota header 仅从明确 typed target 派生，不能用 caller/provider header 偷换。

### 2. 既有持久刷新链

- `RefreshMaterial`、`RefreshAttempt`、`GrantMaterial`、专用 `Prepared`、generation、有限投影和 publication/replay/cache 贯穿 `vertex_oauth`。
- client/secret、grant/requested scope、完整 target 不允许 rotation 改变；仅允许 scope-report 从未知变已报告，以及 refresh 绝对期限学习/缩短，不允许延长或抹去已知期限。
- 成功顺序仍为 **durable Result → runtime apply → 原配置 authority/revision → Published → 派生 cache**，不是跨库原子事务。结果落盘失败只重试磁盘，不重新交换 grant。
- policy wrapper 不代替授权 owner；正常持久 rotation 保留 owner，目标修改撤销迟到结果的应用，但不抹掉已经收到的成功 Result 或解除同 RT family 的 Sending fence。
- `vertexOauth=None` 在旧 JSON 中省略且不改变旧 material/family hash。family identity 不因 target/metadata 编辑改变，不能靠改 context 重发同一被消费的 RT。
- access/refresh 绝对到期保留原值与精度；已过期 refresh deadline 在 Sending reservation/网络前拒绝，不通过重启或 replay 重设寿命。
- cache patch 绑定原完整 context、产品身份、access/RT 与 raw CAS；保留 quota/未知字段，拒绝 target/client/期限或产品变化。原 Redis Lua 与五重 publication repair CAS 未修改。

### 3. 最终调用目标与认证

- typed Full Vertex 复核固定 Google publisher 的完整 URL/method/model/query，分别绑定 global、regional、`us`/`eu` multi-region 目标，不把 Express 无 project 路径借来使用。
- 最后一个产品 owner 重建 `Authorization: Bearer <当前 api_key>` 和可选 `x-goog-user-project`，移除 caller/provider Cookie、旧 Bearer、API Key、错误 quota 等覆盖；不跟随 redirect。
- buffered 与聚合 SSE 实际 loopback 测试逐一覆盖四类目标，测试 transport 只在真实产品校验之后替换地址。它验证生产路径/header/body，不代表真实 Google DNS/TLS、IAM 或模型权限。
- legacy `auth_token` 对新 typed 渠道在网络前拒绝；没有修改旧 generic Gemini header 语义，也不宣称旧任意配置的 Bearer 遮蔽问题已自动迁移修复。
- SSE 沿既有先完整聚合再桥接方式，不描述为实时 token-by-token；完整 Full Vertex decoder/工具/签名/多模态与公共结算仍需后续实现验证。

## 验证与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-lifecycle-20261105/`。

所有 Cargo 验证 `--locked`、最多4 jobs、无 management-ui、默认线程栈；未设置 `RUST_MIN_STACK` 或加栈规避。
仅使用合成授权材料、随机 loopback peer 和 linshi 下独立状态目录；没有读取 `.ng`、ADC、Antigravity 或历史凭据。

实际功能红测 `red-credential` 为 **0 passed / 5 expected failed**，验证 nested client secret 尚未接入 redaction/SecretPatch/Keep/closed schema；不是编译失败充当行为红测。
初次 `green-lifecycle` 因既有 Accio 测试调用新增 `apply` 参数时漏传 `None`，以 `E0061` 编译失败；修正测试调用，不改变 Accio 语义。
初次 `feature-off` 因新增关闭测试将 `GatewayError.code` 字段误写为 `code()`，以 `E0599` 编译失败；改为 `code.as_deref()`，保留同一错误码预期。
上述失败回执保留，不计入通过数，也不回写为绿色。

### 最终通过与来源

| 检查 | 结果 |
| --- | --- |
| 最终默认运行组合 `green-closeout` | **62 passed / 0 failed / 0 ignored**，其中20项本批新增；重新编译过修正后的最终源码 |
| 代表性关闭 `feature-off-final` | **38 passed / 0 failed / 0 ignored**；关闭官方Vertex、保留Gemini Web core，其中1项本批新增关闭专属测试；不是完整feature matrix |
| 相邻刷新/DB/适配器 | `token_refresh::` 133 passed/4 ignored；refresh 9、payload 10、Accio40、Grok75、Kiro46、Express87/1 ignored、call_flow40，各项均0 failed；过滤间有重叠 |
| 补充 Console 单元 | nested secrets5、document2通过；与主过滤存在重叠。`console::tests` 和 `console::refresh_grants::` 实际匹配0项，不能计作覆盖 |
| 真实 Local 集成合同 | `bedrock_console_secret_contract` 10、`console_document_contract` 70/1 ignored、`console_refresh_grant_contract` 7/6 ignored，合计 **87 passed / 0 failed / 7 ignored** |
| 静态 | checker31项通过；最终Rustfmt、ratchet和工作区`git diff --check`通过。ratchet4443文件、14项immutable runtime，governed source >700为0 |
| 来源与编码 | 最终2963项选定输入无漂移；本批59项Rust源码严格UTF-8无BOM，14个新源码独立whitespace通过，写前src快照无本批范围外变化 |

按完整测试名去重、并为集成target加身份前缀，**575个不同Rust测试通过，其中21个本批新增**（默认20项、关闭专属1项）。
不重复累计绿色重跑，不把ignored、0项过滤、失败run或已有测试重跑算新增。
Console文档的1项ignored需要显式导出的合成前端fixture；刷新合同的6项ignored需要独立Redis URL，本批未执行。
相邻unit另有5项ignored，同样不算通过，不代表真实Redis/浏览器验收完成。

相邻过滤使用`green-final`的EXE，期间及结束时哈希保持`07FEF04A0FB9E40395FCF0578A2F922CDE10AC6316E5EA2147A124BE1AF56527`。
其后唯一源码差异是`business_tests.rs`中仅在官方线关闭时编译的`code.as_deref()`断言修正；默认编译路径的生产代码与测试行为未变，因此没有无故重跑全部相邻过滤。
最终默认组合仍重新编译并通过62项，新EXE哈希为`97B067E2AAC9DAD379FC6BB03AACF1E58C2556CBE60365BF4B40BC919FC51C5B`。
这两个编译/执行阶段明确分开，不谎称全部相邻结果来自最后EXE。对应证据为`green-final.json`、`adjacent.json`、`green-closeout.json`、`feature-off-final.json`、`integration-final.json`、`checks-closeout.json`和`closeout-audit.json`。

库测试构建使用4 jobs；观测到MSVC库测试link约8GiB working set后，3个集成target使用1 job串行链接，避免并发大linker，资源决策见`resource-assessment.json`。
没有固定全流程单任务或调高栈。checker实现未变，31项结果复用本批`final-checks.json`；最终测试断言更改后重跑Rustfmt和ratchet。
仅官方线、不含共享Gemini Web core的旧15项缺符号失败未由本批修复或重跑，不报完整矩阵绿。

临时closeout审计首轮误将写前src/docs快照未覆盖的Cargo/crates/config条目视为新增源码，触发`Unsupported language: undefined`；修正为原src写入范围再审计。
这是linshi审计脚本的范围错误，未改生产代码、checker或baseline；原完整2963项选定输入的前后hash检查没有缩小。
补齐Accio测试的before有效行时，初次临时重建多留了一个换行，旧hash校验明确拒绝；修正后与写前hash完全一致才计数，没有将猜测前值作为证据或回写旧源码。

## 规模与安全复核

所有59项新增/修改Rust均低于500有效行；新文件14项，范围内最大既有文件494，没有新增例外或重写baseline。
新增职责拆在Vertex专属材料准入、配置、refresh prepared、最终header和定向测试模块；既有owner只接可选context与有限projection。
外部配置仍在原声明、typed反序列化/validate和最终网络边界校验；无任意OAuth origin、额外凭据发现、依赖升级、新后台task、无限重试或无界队列。
继承的HTTP预算、取消Drop、grant单飞与Result持久义务保持；本批没有增加跨上游exactly-once或新的Redis durability保证。
本地重建是同测试进程的新runtime/store，不是实际杀进程或OS重启；scheduler与最终发送分别验证，未冒称一条完整正常UI/router业务验收。

## 下一接力点

1. Full Vertex 独立正常管理 listener/session/create/status/cancel/import、显式 client/target、服务端 receipt 与普通 SecretPatch/原 revision-CAS 保存；不借 Antigravity 登录/账号目录。
2. 新建/稳定 ID 重授权、暂存受众、未确认保存恢复的正常 UI 与最终模板，再接正常 UI→真实管理 TCP/SQLite→调用和重建。
3. 公共 Chat/Messages/Responses JSON/聚合 SSE 的真实 router/Key/usage/结算、取消/refund，以及 Full Vertex 自己的响应终态/工具/签名/模态准入；不以本批 UpstreamClient loopback 代替。
4. 本批 Vertex-specific Server/Worker 的真实 Redis publication/restart/竞争、Hosted/PG、旧数据升级与独立 OS 进程故障仍须验证。本批 RESP witness 不冒充执行 Redis Lua。
5. 真实 OAuth client/账号、IAM、project/region/model、refresh 与额度在 P60/P70 集中录制验收；语法与本地成功不证明真实权益。
6. Express 未支持的多 candidate/Part/Preview 表示、signed Preview 关联和实时参数流，以及其他原定渠道、Luma Agents、Web/媒体、P50/P60/P70 不从原范围移除。

本批不新建 Redis/Docker 资源，不使用已消费的 P34 单容器授权；不出包、替换运行实例、迁移数据库、提交或推送。
