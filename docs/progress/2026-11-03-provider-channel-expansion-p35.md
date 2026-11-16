# 服务商分渠道扩展：P35 Grok 正常管理登录与 SecretPatch 保存

> 2026-11-03；本批后端实现与定向验证通过；扩大的旧 loader 合同另有一项真实失败，见下文。不表示 R05 或总计划完成。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保护既有脏改，未提交、推送、出包或替换运行实例。

## 1. 本批实际范围

本批从 P34 刷新存储子链转回正常 Grok 登录。新增真实管理路由、实际 provider/preset/revision 实查、服务端 receipt 保存，以及普通 SecretPatch/CAS 桥接，不再以手工构造 Binding 代替管理集成。

- 管理 API：`create/status/complete/cancel/import`。create 与所有 mutation 均要求正常管理身份和 Secret Grant；status 要求同 owner 管理身份。owner 仍绑定 token fingerprint、Host、IP。
- client/scopes 显式输入；consent principal 预选和 required principal 约束分开。issuer/token endpoint/业务受众不可由请求指定，未知字段拒绝；创建 16 KiB、action 32 KiB body 上限。
- 只接受已保存的精确 `grok-oauth` provider 和固定业务 base；启动、callback 发送前及保存前实查原 revision。指定 credential ID 才表示重新授权，不用未验证 hint/邮箱自动合并账号。可写 runtime、drain/shutdown 同样属于准入边界。
- receipt 从服务端进入正常 redacted draft 与 `SecretPatch::keep/replace/clear`。access/refresh/context/原绝对 expiry 作为同一 credential 材料提交；缺新 refresh 时 clear 旧值，不继承旧授权的 refresh。
- 原 revision 传入正常 `commit_document`；不取最新 revision 自动重基，不建立旁路 CAS、另一个 secret owner 或新数据库格式。重新授权保留稳定 ID、disabled/操作者字段/模型与 policy；未修改账号和其他 provider 沿用正常 keep 保护。

接口与状态契约见 [管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-grok-oauth-login.md)。

## 2. 保存 ownership 与结果语义

预验证失败不消耗 Ready receipt。保存接纳后，registry 内受跟踪的 JoinSet 任务持有唯一 receipt 与候选，HTTP 断连、重复 import、cancel 或登录 TTL 不撤销已接纳的 commit。shutdown 停止新增并 join 保存任务；异步保存阶段使用 20 秒 deadline，不包括接纳前同步预验证，不声称可抢占同步文件 I/O。

只有正常 commit 返回成功才记 `succeeded`、`credentialId` 与 `savedRevision`。commit 报错、超时、panic/drop 则为 `save_unconfirmed`，不自动重发或插入第二账号；保留非秘密 `targetCredentialId` 和原 revision 定位结果。普通 runtime journal 的恢复职责未被复制；本批没有新增完整自动 reconciliation UI 或跨进程登录恢复协议。

原实例 registry 的 16 会话、600 秒登录 TTL、单 expiry worker 继续使用；成功/未确认的非秘密结果保留 300 秒，仍受容量限制。保存 future 在第一次 poll 前就持有完整 Drop guard，防止“已接纳但任务未开始”丢失状态归属。

## 3. 红测与失败归因

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p35`。

1. 修改生产代码前，`red-router-v1.log` 实际运行 1 项失败：新 Grok 路由返回 404，期望 200。这是正常 router 行为红测，不是编译失败。
2. 初版 `management-v1.log` 编译成功，实际运行 6 项，3 passed / 3 failed。保存 guard 的 async closure 仅捕获标志字段，整个 guard 在 future 执行前就析构，导致刚接纳的保存被误报为 `save_unconfirmed`；提交失败合同甚至未到实际 activation。
3. 修补使用 guard 方法强制捕获整个 Drop guard，并补未首次 poll 就 drop 的精确回归。另加等待 publication 时过期、20 秒虚拟时间预算、shutdown 拒绝新保存、稳定 ID 重授权、缺 refresh clear 与精确原 expiry 合同。
4. 静态交叉审查发现 external router 负测用 builder 追加了第二个 management token header，而非替换第一个有效 token；已修夹具使用 `headers_mut().insert`，未放宽生产鉴权。该错误未被伪报为错误 token 可通过认证。
5. `grok-v2.log` 最终 71 passed / 0 failed。真实 create 固定 issuer 的合同与 cfg(test) 合成 issuer 的 router callback/import 合同分别执行；没有给生产 API 增加任意 issuer 注入能力。

## 4. 验证记录

后端统一使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，不触发 `management-ui` 的工作区发布/清理 hook。TEMP/TMP/TMPDIR 指向本批 `runtime-temp`；16 MiB 最小线程栈仅属于测试进程。

| 范围 | 当前结果 | 日志 |
| --- | --- | --- |
| `cargo test ... --lib grok_oauth:: -- --test-threads=1` | 71 passed | `grok-v2.log` |
| 相邻 Console 单测 | 128 passed | `console-v2.log` |
| Grok/Antigravity 管理 router、正常事务 integration | 2 + 1 + 12 passed | `router-transaction-v3.log` |
| 扩大的正常文档合同 | 64 passed / 1 failed / 1 ignored；不是全绿 | `router-transaction-v2.log` |
| `cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib` | exit 0；代表性 feature-disabled，不是全矩阵 | `feature-disabled-v2.log` |
| Rustfmt | exit 0 | `rustfmt-v2.log` |
| checker tests / ratchet | 31/31 passed / exit 0 | `checker-tests-v2.log` / `ratchet-v2.log` |
| `git diff --check` | 文档收尾后 exit 0 | `diff-check-final.log` |

第一次组合 integration 命令在文档 binary 失败后停止，没有执行排在其后的 Grok 和事务 binary；后者已在 v3 单独实际执行成功，不以编译成功替代运行。文档失败项为 `resource_limits::legacy_compiler_rejects_extreme_token_expiry_without_panicking`：实际 `load_from_yaml` 经 unchecked loader 和 legacy refresh owner 截断超大 lifetime，测试要求返回错误。相邻的严格文档 expiry 拒绝用例实际通过。本批 13 项写入范围没有修改 legacy loader/refresh_material/refresh_state，该不一致作为未关闭问题保留，不改测试、加 skip 或宣称全仓绿。

按完整测试名去重，本批实际 **259 个不同 Rust 测试通过，另有上述 1 项失败、1 项 ignored**。259 包括扩大文档合同中通过的 64 项，不将同名重复执行或 71/128 套件重叠重复计数。正常定向绿色日志与扩大失败日志分别保留。

本批源码冻结记录 `source-frozen-v2.json` 含 13 项。官方 lexer 的主要前后值为：会话 210→233，registry 142→190，模块入口 49→66，HTTP wire 399→401，router 288→302；新 persistence 138、save 137、HTTP 148。冻结源码最大 401；新增测试最大 257，均为单一管理保存/失败合同职责，无 501–700 例外，未改 baseline 或 exception。其余未先测前值的既有测试文件不伪报从零新增。

文档收尾后核对 13 项源码 SHA-256、18 个源码/文档的严格 UTF-8 无 BOM/尾随空白与本地绝对链接，见 `audit-final.json`；其 `failures` 仅表示来源和文本完整性，已知测试失败另列在 `knownTestFailures`，不是总回归成功标志。scoped tracked diff 和 scoped status 分别保存；未跟踪源码由上述 hash 清单覆盖。

## 5. 尚未完成与下一步

- 前端仍是之前的九个新增模板，`grok-oauth` 模板及正常登录 UI 尚未开放；自动 loopback listener、device 管理 orchestration 继续下一批，不把手动 callback API 当完整用户流程。
- 未确认保存的完整恢复交互、真实第三方 client 授权/产品使用边界、完整 Responses 保真与逐渠道真实验收仍缺；普通事务错误不得被状态轮询猜成成功。
- 扩大验证发现的 legacy YAML loader 极端 expiry 拒绝合同仍失败，须在后续小范围修复并复验；正常新管理保存使用 strict validation，不能用这条通过路径冒充旧 loader 合同已关闭。
- P34 所列跨 revision 可信刷新恢复、永久 Sending 可用性、其他 refresh 入口、真正多 OS 进程强退/断电与 Redis durability/failover 仍保留。广 `cargo check --lib --tests` 的历史 depth-limit 失败没有被本批宣称清除。
- Accio/Kiro/Vertex OAuth、Perplexity Web、其他 API/媒体及 P50/P60/P70 全部继续；本批不增加完整完成的 W 卡，也不标总目标完成。

没有读取历史凭据、真实 OAuth/推理/费用调用、录制、创建新 Redis 容器、出包、在线替换、提交或推送。P34 已获批准的新容器早已完成精确清理，其授权没有被复用。
