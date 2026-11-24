# SAP 持久化 Responses 公共生命周期验收

## 结论

2026-11-23 已完成默认关闭、显式启用的 SAP 持久化 Responses 公共生命周期接线，
以及源码回归、新 API devtest、隔离 Hosted 双 EXE／重启联合验证。
随后补齐有明确耐久依据的无人值守本地账务恢复，源码回归与新恢复包验证均通过，见下。
本轮又完成阶段化错误／健康反馈、内部探测事实、流式实际状态和单次限流准入修正，
最终源码 **207/207**、隔离 Hosted **11/11** 通过；这批修改尚未进入新包。
随后补齐精确零发送回退的 JSON／SSE 拒绝记录、SenseNova 内容块准入与
SAP 无用量单条消息预付恢复；新增批次 **48＋12 项源码检查、1 项隔离 PG** 全部通过，
见下方“拒绝记录与消息预付恢复”。这些结果不抵扣当前源码的新包门禁。
这不是三服务商全部收口，未知提交的权威绑定和人工处置仍未完成。
启用条件与恢复矩阵见[受控功能合同](../providers/sap-stored-responses.md)。

## 本批实现

- `POST /v1/responses` 的显式 `store:true`、同主体续接，以及 Gateway 自有
  `resp_gw_*` 的 GET／cancel／DELETE；不直接接受 SAP ID，不泄漏私有模型和 metadata。
- Local SQLite schema-1 附加初始化，Hosted 独立迁移 `008`–`011`；耐久 pin
  绑定原主体、公开模型、部署、资源组、实际凭据和扣费来源。
- Create／续接实时复查原候选，并在最终发送点原子取得熔断／容量许可；
  GET／清理保留当前授权检查，不继承生成 cooldown。普通非 SAP `store:true` 不改道。
- 耐久预扣、确定未发送退款、终态迟到用量和幂等结算；取消／删除由数据库授予
  唯一发送资格。未知提交不重放，未确认 mutation 不因重启、超时或 GET 终态解锁。
- 每进程 32 个耐久任务；发送后的 HTTP 断开不丢账，正常 drain 等待完成。
  管理 pending 分页仅用于发现，拒绝普通 Key 与公开 passwordless marker。
- 普通 auto-route 预扣职责原样抽至 `stage_route/reservation.rs`：
  `stage_route.rs` 从本批抽取前 525 降至 482 有效行，新模块 47 行；普通预扣顺序不变，
  durable 路径不进入普通预扣和 finalize。
- `011` 增加与 revision 匹配的耐久未发送证明，仅 live submit owner 在执行前明确
  拒绝时写入；证明独立提交后再退款。历史未知提交不补证明，证明写失败不推断资格。
- 复用正常后台任务 owner；启动即扫、10 秒周期、每批 32 项、每个 DB 步骤 5 秒等待
  预算。跨批 keyset 游标隔离坏行，双库事务复查保证只关闭一次 hold。
  原批次仅恢复明确未发送或已有完整权威终态用量；后续增加严格限定的已完成
  单条消息预付恢复，见下。不访问上游、不发 OAuth、不解除 mutation 屏障。

## 原恢复批次的源码与静态验证

| 检查 | 实际结果 |
| --- | --- |
| Nextest，含适用隔离库及 runtime 生命周期 | **177 passed，0 failed，6315 按表达式跳过** |
| Python 包级夹具单测 | **20/20 passed**；OAuth 计数补强后关联 SAP 11/11 复验通过 |
| SAP catalog 模板组件 | **14/14 passed**，复用本轮未漂移的既有结果；不是原生窗口验收 |
| 改动 Rust 定向格式、scoped `git diff --check` | passed |
| 权威 effective-line ratchet | passed，6140 个文件，governed source >700 为 0，immutable runtime artifacts 14；最后的测试名单修正另经 fast gate |
| `cargo fmt --all --check` | **failed：下列既有／无关文件存在格式差异，未擅自全仓格式化** |

Nextest run ID：`7f5617fd-6746-42ad-ad46-c4405d7d14d9`。
测试执行 39.387 秒，测试构建 4 分 31 秒；日志为
`C:/Users/Public/nas_home/AI/GameEditor/linshi/sap-stored-recovery-20261123-r2.log`。

```powershell
cargo nextest run --locked --offline --jobs 6 --lib --no-default-features --features runtime-defaults --run-ignored all -E 'test(sap_aicore) | test(sap_stored_response) | test(/^runtime::/) | test(local_keys_responses_scope) | test(pipeline::lifecycle) | test(pipeline::stage_route)'
```

首轮 315 项中 310 passed、5 failed：宽过滤词误选四项缺独立 Redis 环境的
`browser_executor_runtime` 用例，另一个 runtime 名单断言遗漏既有 Luma worker。
已改为完整六 worker 精确名单，并用锚定过滤词完成上述回归；未改无关浏览器产品逻辑。

新增恢复证据覆盖 SQLite 旧库缺迁移、附加初始化、关库重开与双连接竞争；
SQLite／PostgreSQL 均验后台/后台和前台/后台退款竞态、明确零用量、未知用量保留、
mutation 屏障保留，以及真实写锁超时后释放连接并恰好恢复一次。
公共入口故障验证 live owner 的证明写入和后台结算；OAuth 与业务请求独立计数。
另验 34 项分页、首行坏账不阻塞后续、证明写失败不退款及 drain 后不开始新步骤。
`store/tests.rs` 为 488 有效行，新增 recovery／worker／测试分别 76／73／260 行。

全仓格式差异：`crates/gateway-core/src/error/model_pricing.rs`、
`crates/gateway-core/src/provider_identity.rs`、`crates/gateway-embedded-ui/build.rs`、
`src/console/refresh_grants/material.rs`、`src/http/routes/internal_console.rs`、
`src/http/ui.rs`、`src/protocol/sensenova/tests.rs`、
`src/provider_runtime/model_probe_audit_test_support.rs`、`src/provider_runtime.rs`、
`src/token_refresh/cache_target.rs`。

## 错误、健康与发送证据批次

- 普通 SAP 请求在实际发送前完成 planner／OAuth，使用不可复制的一次性 sender；
  真实发送才增加 attempt、归属与指标，不因错误分类开启重试或候选重放。
  保留实际 HTTP 状态和 `Retry-After`，OAuth 状态不能冒充 inference 状态。
- inference 429／5xx 和明确的协议失败可反馈到精确凭据／模型；OAuth、普通
  4xx、传输结果不明保持健康中性。HTTP 错误不是未发送证明，持久化 Responses
  的未知交付仍保留 hold，不因健康反馈退款。
- 耐久 owner 先完成必要回执／账务写入，再最多等待 5 秒写健康。只有已确认的
  生成结果恢复健康；queued、GET、cancel／delete 不清除生成故障。
  本批新增取消测试覆盖健康 mutex 等待，不冒充 SQLite／PostgreSQL 写锁取消。
- 流式成功、失败、取消及归档保留观测状态；明确未收到 HTTP 响应时不填 200，
  已收到 200 后真实连接断流仍记录 200，但标为未确认完成、健康中性，不伪造终态。
  pre-handoff 与 immediate-drop 的 202／NULL 三态测试是 finalizer seam，
  不冒充真实 SAP 202 headers barrier。
- Hosted 内部探测依赖加法迁移 `012-gateway-sap-probe-evidence.sql`；
  仅闭合终态写入脱敏 `sap_evidence`。旧行、running 和终态写失败保留 SQL NULL，
  表示未知，不是零发送证明；缺迁移则发送前拒绝。其他产品 SQL 不依赖新列。
- 修复 OAuth 前后重复 `admit()` 消耗 provider 限流窗口的问题：外层准入一次，
  OAuth 后仅 `recheck_send_eligibility()`。完整 `run_pipeline` 的 JSON／SSE
  回归证明 `max_requests=1` 时首次成功、第二次 429，只有一次 OAuth／推理，
  第二次无实际账号归属并退回本地预扣；该测试未经过 HTTP router。
- 零发送 `Next` 恢复进入候选前的状态观测，避免 SAP 的显式空观测污染下一候选。
  当批仅静态核对；后续精确双候选动态回归及其暴露的拒绝记录缺失已补齐，见下。

### 最终源码与 Hosted 回执

| 检查 | 实际结果 |
| --- | --- |
| Nextest：SAP、限流／熔断／finalize、共享 probe owner、Command Code／SenseNova probe、生命周期及 Copilot 关联回归 | **207 passed，0 failed，6306 按表达式跳过** |
| 同一新测试 EXE 的 opt-in Hosted，逐项独立进程串行执行 | **11/11 passed：SAP 5、Command Code 3、SenseNova 3** |
| Python devtest 与 package layout 合同 | **7/7 passed**；真实 packager 的合成载荷证明 `012` 字节、manifest 登记及完整性，不是新的 Gateway EXE 包 |
| line manifests | **56/56 passed** |
| 定向 Rust 格式、112 个相关文件 UTF-8 无 BOM、scoped `git diff --check` | passed |
| 本批权威 effective-line ratchet | passed，**6160 文件**，governed source >700 为 0，immutable runtime artifacts 14 |
| 全仓 `cargo fmt --all --check` | failed；仍有 9 个既有／无关文件格式差异，未擅自全仓格式化 |

最终 Nextest run ID：`02ab3d6c-8fd8-4d3c-b7d7-a77c30cb8df3`，
构建 **5 分 13 秒**（原 target、6 jobs，开始时空闲约 10.9 GiB），测试 **35.403 秒**。
日志：`linshi/gateway-packaged-provider-fixture/sap-error-health-nextest-20261123-r4.log`。
此前 206 项通过的结果由最终 207 项回归接替，不重复累计为额外覆盖。

测试 EXE：`target/debug/deps/neuro_gateway-14c6b9e619cb0fe4.exe`；
SHA-256：`492fb8f8a0a6ba338aee57323ea81cf56e4a19883a8c58c408b6e9846f0d716b`。
Hosted 回执：
`linshi/gateway-packaged-provider-fixture/sap-error-health-hosted-20261123-r4/result.json`。
先核对专用容器标签、loopback 端口及 `gateway_fixture` 库，再从 `--list`
确认 11 个精确用例；未运行要求另一未迁移数据库的 Command Code 用例。

SAP Hosted 验证包括：OAuth／inference 分阶段状态与精确健康、未领取 create 的取消、
缺 `012` 零发送、迁移重复执行与旧行 NULL、终态写失败不发布成功或重放，以及
stored HTTP 503 仅发一次、跨 AppState 精确健康和审计、余额 2→1 且 hold 保留。
迁移用例只在独立 schema 安装 `007`／`012`，不宣称其中健康表写入成功。
共享产品验证各覆盖审计准入失败、失败／取消终态和终态存储失败。
首轮 Hosted **10 过／1 失败**定位到夹具在管理 API 后错误要求唯一 `Arc`，
改为独立 writer 建立后立即注入隔离 pool；没有因此放宽生产断言。

当前全仓格式差异与前批列表相比，`src/provider_runtime.rs` 已定向修复；
剩余另外 9 个文件。完整输出：
`linshi/gateway-packaged-provider-fixture/sap-error-health-fmt-20261123-r4.log`。
本批未出包、未执行原生窗口或真实 SAP 调用；下列历史包不含本批补丁。

## 拒绝记录与消息预付恢复

本批先执行确定性红测，保留 `provider-admission-nextest-red-20261123-r2.log`：
**1 passed／3 failed**。SAP 公共 JSON 已回退成功但缺精确拒绝项，SSE 完全缺少
`rejectedCandidates`；SenseNova 的 `video_url` 实际返回 200 合成答案而非预期 400。
未通过删除断言或放宽健康／余额规则求绿。

### 修复与证据边界

- SAP OAuth 使用可释放 barrier：收到 OAuth 后经真实 route owner 禁用原 `exact`
  凭据，再释放 Token。公共 JSON／SSE 均证明 SAP 零推理、普通 fallback 恰发一次、
  唯一实际账号／凭据／模型归属、HTTP 200 和 5 tokens、只扣一条消息，
  同时保留原 SAP 的精确拒绝原因且不罚其健康。
- 共享 `stage_send/admission.rs` 仅对未发送 `Next` 恢复原三态并记拒绝；
  实际发送、成功和 `Stop` 不改写观测。外层 attempt 准入拒绝复用同一记录入口。
  预算／取消 marker 即便携带上一供应商错误码，也不记到当前候选。拒绝信息继续
  去重、最多 100 项、仅存固定错误码与引用，不复制错误正文或秘密。
- SSE snapshot 拥有拒绝列表的独立副本，成功和失败 trace 均输出；清空原 ctx 的
  单测证明快照不丢失。另有三态 stage-send seam 测试，但该 seam 不冒充完整认证／退款链。
- SenseNova 同时检查原始内容形状、canonical 内容以及最终发送计划。拒绝未确认媒体、
  未知块、非法 `text` 和非数组对象／数值／布尔 content，避免通用解析先把非法文本变空。
  公共 JSON／SSE × 四种 role × 五类非法块共 40 次请求均 400、零发送／零实际归属、
  精确拒绝原因、余额和健康不变；同一账号随后两次合法请求成功。Raw text 扩展、
  assistant 空内容、函数工具与 reasoning 历史保持。此证据只封 content，
  不等于所有顶层媒体参数被封闭，也不表示上游不支持视频。
- SAP 扫描和事务结算均限定新增恢复为 `completed + message_prepaid + reserved=1`
  且两项 tokens 均 NULL；复查原账号／模式，只关闭原 hold，不再扣费、不退款。
  SQLite 在落库后先关库再重开，双连接竞争只成功一次，再重启仍稳定；
  PostgreSQL 独立连接执行同一 exercise。failed、pending、未知提交、token 预付与
  故障注入的零 token hold 保留，未确认 cancel 的 mutation 屏障仍阻止 Delete。
- 公共 create 的 SQLite trigger 故障令结算暂挂，后台完成后、任何 GET 之前直接读回
  `completed/settled=1/open=0`、双 tokens=NULL、原余额及零额外推理／OAuth。
  随后两次授权 GET 保持幂等；这不是包内双 EXE 或真实 SAP 验收。

### 本批检查

| 检查 | 结果 |
| --- | --- |
| Nextest：新增准入／fallback／快照、SAP HTTP／恢复、SenseNova Chat | **48 passed，0 failed，6478 skipped** |
| 同一 EXE 直接关联复验 | **12 passed**；普通 receipt 6、SenseNova public 2、SAP OAuth 2、流交接 2；receipt 前缀的 2 个 Hosted opt-in 默认忽略 |
| 隔离 PG exact ignored | **1 passed**；`sap_aicore_response_receipts_postgres_independent_connections_cas`，包括新增消息恢复双连接 exercise |
| 定向 rustfmt、18 个 Rust 文件 UTF-8 无 BOM、scoped diff | passed |
| 增量有效行门禁 | passed，6165 文件；本批不是整卡收口，未重复权威 ratchet |

Nextest run ID：`af11f5dd-33e4-4f70-a887-be757de2bec8`。复用主 target、
6 jobs，测试构建 **5m49s**，执行 **10.180s**。
日志：`linshi/gateway-packaged-provider-fixture/provider-admission-nextest-green-20261123-r1.log`。
同一 EXE SHA-256：`2dbe25d879e9e9acc89f92d0c55c4a05793064462fea2c6a7240328e9b9805d7`；
PG 回执及 12 项补跑日志：
`linshi/gateway-packaged-provider-fixture/provider-admission-hosted-20261123-r1/`。
先验证专用容器标签、loopback 52850 和 `gateway_fixture`，只在测试专属 schema 执行。
前次 207／11 回执保留为原批次证据，不累计成此次单次全绿数量。

`stage_send.rs` **499→495**、admission **75→97**、balance **276→299**、
recovery **76→81**、store/tests **488→489** 有效行；新增独立测试最大 187 行。
全仓 fmt 的既有无关差异未改；本批未跑新的全仓格式、正式发布或付费上游。
旧 `gateway-devtest-provider-media-admission-20261123-194746` 不含本批或前批错误／健康补丁；
未出新包、未替换 4200、未导入真实账号。

## 原公共生命周期包与来源（不含新增恢复 worker）

- 类型：**API devtest**，不含管理网页和桌面壳，不是正式 Release。
- 包目录：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-sap-stored-20261123-1747`
- EXE SHA-256：`d20b1e605a3fa916692f5609c12ead9707ddee2ce25420e48bd94fcb4f7ac000`
- manifest SHA-256：`5c129985983afdc1d129f4b5829e5c8b23a2faa9684ea6854fb7aa0b95b1cf76`
- 构建源码指纹：`65382634ef3b8e14f66c3f6f34a4a5edb4ffb57bb890355a9d077a54fcdb4da5`
  （8346 个输入文件，dirty worktree）。
- 构建回执时间：`2026-11-23T10:00:23.5642833Z`；manifest 的 `buildTimestamp`
  沿用确定性的 Git 时间，不是实际编译时间。
- 原 `target/` 增量缓存、3 jobs，编译实测 **15 分 26 秒**。当时 Otter 同时编译且
  可用物理内存低于 6 GiB；没有冷构建对照，不据此宣称提速倍数。
- 构建前后源码指纹一致、6197 个载荷文件校验、EXE 哈希与 API artifact smoke 均通过。
  本汇总和收口文档在出包后更新，因此全树指纹会变化；没有修改已验 EXE 或补造包凭据。

## 双 EXE 隔离实测

入口：`tools/packaged-provider-fixture/run_sap_stored_lifecycle.py`。
证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/sap-stored-20261123-1802/result.json`，
`status=passed`。专用 PostgreSQL／Redis 的标签、loopback 端口均先验证，
只在隔离库幂等应用 `008`–`010`，未改线上数据库。

1. 缺 CA 负对照进程按预期 exit 1；writer、reader、重启后端三个独立 PID
   为 `32460`、`41484`、`52568`，均正常 drain、exit 0。
2. 经真实管理 API 配置合成账号、两个各 8 条消息的成员和聚合 Key。
   Create 排队后停止 writer，新 EXE 读取并结算；另一 EXE 续接精确前序 SAP ID。
3. 两 EXE 并发 cancel 仅一次成功外发；随后 GET、DELETE、墓碑 404。
   直接过期原成员后，预热的 reader 仍拒绝旧 ID，零发送，不借备用成员操作旧对象。
4. **再经管理 API 刷新聚合成员**，新建可使用存活成员。此正对照不证明未刷新缓存时
   自动回退已实现。
5. 四个回执各结算 `2/3/5` tokens，按原 `source_key_ref` 合计仅扣四条消息；
   查询、取消、删除和重启没有重复扣款。
6. 精确十次上游方法／路径与全部尝试一致；十三条公共审计均终态，
   十条发送审计归属实际账号和凭据，三条零发送拒绝无 provider 归属。
   Create 的 queued 审计不冒充最终用量；最终用量以耐久回执和余额对账验证。
7. 遍历有界 pending 分页，四个已结算对象均不在库存中；合成 TLS 不转发，
   日志未暴露合成秘密，未连接真实 SAP。

## 新恢复包验证

API devtest：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-sap-recovery-resume-20261123-185420`。
EXE SHA-256：`34666e8f581a0bd110ae2161e5a8fdbc1df62c33c24badec8c8f188c767cfd`；
manifest SHA-256：`bb3e432bff18f053d98e60dcd212d3f73e2ead35b83d763fdc63450f8bb22bd3`；
source fingerprint：`d0699c9d5df3e92513988f640605c40e8c1538ce8b0f8092f7329b54d00173df`。
实际构建时间以 provenance 的 `2026-11-23T10:45:46.7281358+00:00` 为准，
不是 manifest 的确定性 Git 时间。包来源、实物哈希与包内 `008`–`011` 均核对通过。

回执：`linshi/gateway-packaged-provider-fixture/sap-recovery-resume-20261123-185728/result.json`，
`status=passed`。复验原生命周期十次发送／十三条审计后，再从真实公共入口创建一条
权威 `2/3/5` 用量的终态回执，通过仅作用于本次成员的数据库故障令结算暂挂。
全部旧实例 graceful drain 后启动两个新 EXE，无公共 GET 介入，后台完成本地结算。
另以明确标注的 synthetic proof／unknown 消费者行验证：有证明恰退一次，
未知提交保持预扣；第二周期稳定。恢复阶段 **零推理、零 OAuth POST**；
全流程合计 **11 次上游操作、14 条公共审计**，五个业务进程 exit 0，
缺 CA 负对照 exit 1。

合成未发送证明只验证消费者，不冒充包内 live owner 生成证明；生产者由前述源码
故障测试验证。双 EXE 启动不等于已观察到同一行并发争抢，精确 CAS 另由双库源码测试证明。
该回执是正常 drain/restart，不是断电／强杀测试；未改线上实例、凭据或数据库。

## 2026-11-24：双 EXE 工具续轮与 TLS 拒绝

API devtest `gateway-devtest-sap-hosted-tools-20261124-070907` 通过包完整性检查，
`run_sap_hosted.py` 使用两个独立后端和专用 PostgreSQL／Redis 验证 **48 次成功推理／48 条完成审计、14 次 OAuth**。
新增 16 次调用覆盖 Orchestration／Bedrock 各自的纯文本工具和图文＋工具，JSON／SSE 各做两轮；
以首轮实际返回的调用构造第二轮，校验原生 wire、参数拼接、终态、精确归属、2／3／5 tokens 和逐条消息扣款。
错误 CA 对 client secret 和直连 X.509 各产生一条零发送失败审计：没有 OAuth HTTP 或 inference HTTP、
没有上游状态／用量／实际账号归属，预扣余额返还，账号和模型健康快照不变，隔离进程正常退出。
旧包的相同 TLS 请求则记录为已尝试，不能抵扣当前验收。

- 回执：`linshi/gateway-packaged-provider-fixture/sap-hosted-tools-current-20261124-072519/result.json`
- EXE SHA-256：`49ac17f0c622de1c455f2e9ac3ee7592fcbe0146f44595c79b721888c9049578`
- Manifest SHA-256：`23f463c16348b418549360822fa01cf28fd7e1d0bc0b7012aabdde5919dfe1e3`
- 构建源码指纹：`39940685fce2b0893bdbba66f68b0f428fbc765f718dae187b2fa63d2a957cdd`
- SAP Python 夹具 **13/13**，有效行 ratchet **6167 文件通过**。未切线上。

## 2026-11-24：后台无用量结算与 Hosted 管理探测

复用上节同一包。旧 EXE 正常 drain 后，双新 EXE 在无公共 GET 条件下结算两条实际公共
create 的终态。无用量记录的三项 token 保持 NULL，hold 关闭，第二周期稳定；
后台零新增 OAuth、推理或公共审计，整轮共 15 条公共审计。该证据不等同于强杀恢复、
观察到两个 worker 争抢同一行或验证包内证明生产者。

Hosted 管理 probe 校验包内 `012` 后，实测缺 grant 不发 HTTP、OAuth 429 零推理且
不罚健康、推理 503 恰发一次且只记精确模型健康；两种失败均保留 `Retry-After`，零公共审计。
缺 `012` 及终态审计写失败仍是源码级证据。

- 恢复回执：`linshi/gateway-packaged-provider-fixture/sap-stored-no-usage-20261124-073955/result.json`
- 探测回执：`linshi/gateway-packaged-provider-fixture/sap-probe-current-20261124-074421/result.json`

## 2026-11-24：发送前撤销与普通 Chat 回退

旧包 JSON 回退通过，SSE HTTP 200 但暴露普通上游模型名。修复仅对有显式模型映射的
普通 HTTP Chat SSE 投影顶层公开 `model`；复用有界 SSE 解码，保留工具参数、用量、
SSE 辅助字段和原有终态，不改变已由专用协议守卫处理的流，不增加回退或重放资格。

API devtest `gateway-devtest-sap-mapped-chat-20261124-081035` 通过包完整性与隔离联验。
JSON／SSE 两条均在 OAuth barrier 后撤销精确 SAP 凭据：2 次 OAuth、0 次 SAP 推理、
普通备用恰好各发一次，2 条精确完成审计各 `2/3/5` tokens、共扣 2 条消息。公开
JSON 和所有 Chat SSE JSON 帧仅报告请求别名；SSE 终态唯一。SAP 拒绝快照正确，
无实际发送归属、未记健康失败；隔离进程正常 drain。

- 回执：`linshi/gateway-packaged-provider-fixture/sap-fallback-fixed-20261124-081035/result.json`
- EXE SHA-256：`b454cc9725706e6b53bdcdd21243dfeff9f51d1ef62b7e47208151f5e6b90337`
- Manifest SHA-256：`74b386291e4f1a4bb906dd25e254f1349ac7303673ef7fb8e98b0a719f9db66a`
- Nextest **24/24**（`71cc5767-ce70-479b-a5b7-34d8108b0c16`），同一测试 EXE 的三服务商关联回归 **11/11**，SAP Python 夹具 **14/14**，定向格式与 6170 文件 ratchet 通过。全仓 `cargo fmt --all --check` 仍有范围外差异，未修改。
- 上节恢复／探测与 48 条矩阵仍绑定原包；本次新包回执只覆盖发送前撤销与回退，不把旧包结果记作新包复跑。

## 仍未完成

默认关闭不等于这些问题已解决：未知提交没有 SAP ID 时缺权威绑定／人工结案流程；
证明未成功落库的 `refund_pending` 不能自动退款；不确定 cancel／delete
尚无人工处置入口。有依据的自动本地恢复不等于未知提交的完整对账流程。
现金 tariff、
未列出的协议／媒体组合、本功能原生窗口交互及真实租户资格／费用也未完成。

保留旧包和现有实例；没有切换 `4200`、导入真实账号、清理容器或删除数据。
SenseNova 视频 wire、控制台 OAuth／积分池正式合同仍未解决，不能将三服务商状态
改为“只差真实账号”。
