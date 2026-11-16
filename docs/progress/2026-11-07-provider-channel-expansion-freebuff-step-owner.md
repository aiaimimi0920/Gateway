# FreeBuff：模型步骤的统一预算、协作取消与原 run 收尾所有权

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承未提交改动，HEAD不是本批交付凭据。本批内部步骤owner已实施并完成下列终态验证；不是R06整卡或总计划完成。

## 1. 目标与边界

承接[顺序模型步骤与累计费用](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-multistep.md)，补后续host执行器必需的单一步骤入口：prepare、send和decode共用预算；确定未发送的错误不伪造模型步骤，可能已发出的错误/取消不把未知费用写成0；原run始终回到显式收尾owner。公共guard、模板数与凭据/数据库格式不变。

这不是完整root/harness、实际工具/child、公共producer或正常UI逐次消费确认，不以执行器内部方法代替公共auth/filter/route/send/finalize。START结果不明、直接Drop、FINISH未知与进程退出的持久run/远端lease恢复继续；本地至多一次send不是远端exactly-once。

## 2. root 合同调查

本批不重新联网，核同一公开固定commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的既有tar归档SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`。重新提取base3、CLI catalog agent、compiled model映射和Apache-2.0 LICENSE四份原文，逐项SHA见证据根`reference-provenance.json`，不执行第三方实现。

base3明确将基本harness、CLI专属工具及模型compaction/step prompt分开；canonical opening需置首，但这不证明只拼opening即完整支持。公开归档不包含注释引用的`web/src/llm-api/root-instruction-contract.ts`服务端实现；不据此猜服务器校验细节或宣称当前线上合同已验。本批先接统一步骤owner，后续仍需完成真实root/工具/上下文与公共消费者，不把这个依赖降格为永远单轮文本。

下一处具体接缝已定位：公开`freebuffCatalogRootAgent`需要同一row的`handle/displayName/compaction/legacyDigests`；当前[ModelRow/SelectedModel](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/model.rs)尚未承接后两项及其模型策略。这是后续root实现的真实输入缺口，不应从opaque handle或UI名称猜模型策略，也不能用任意默认compaction掩盖缺失。

## 3. 实现与验证

[execute_step](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution/step.rs)将一个已确认START的原run留在可取消send future之外，沿同一move owner完成prepare/send/decode并返还decoded owner。原分段run API只保留给test-only wire夹具，生产内部入口不再把run移进会丢弃它的send错误分支；底层裸BusinessRequests仍是协议原语，不宣称其等于准入/执行器。

每步预算在准备前开始，最多300秒；后续阶段只拿剩余时间，不在headers后重置。取消future原地pin并贯穿各阶段，没有新后台任务或重试。同步有界序列化/签名不能被抢占，这不是硬实时保证；这是已确认START之后的单步预算，不是整个agent、公共请求或FINISH的总预算，host还须传当前剩余时间并另行await FINISH。

已取消请求在准备前拒绝；model/metadata/大小/零预算等确定本地拒绝不增加步骤、不污染前步已知费用。进入network future后，错误/取消视为可能已发送，追加一次费用unknown的模型尝试；不会发送伪造0 FINISH或继续下一步。decode期间保留实际已观察费用；完整语义成功与真实EOF才可继续。FINISH仍显式由原owner消费，原claim不释放；未知费用阻止FINISH不是远端义务已解决。

终态证据根：[gateway-freebuff-step-owner-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-step-owner-20261107)。原始命令/log、阶段输入hash、写前preimage、原文对账、范围diff及失败回执均保留；[audit.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-step-owner-20261107/audit.json)核终态来源、去重身份与保护文件，[test-identities.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-step-owner-20261107/test-identities.json)保留完整测试名。

| 终态检查 | 实际结果 |
| --- | --- |
| `cargo test --locked --offline --lib freebuff_catalog_step_owner_ -j 4 -- --test-threads=1` | 8 passed / 0 failed / 0 ignored |
| 正常lib选择`freebuff` | 347 passed / 0 failed / 2 ignored；两个旧browser host未显式执行 |
| 正常lib选择`pipeline_lifecycle` | 6 passed / 0 failed |
| 正常lib选择`local_runtime::` | 49 passed / 0 failed |
| 同一lib harness按完整名去重 | **393 passed / 0 failed / 2 ignored**；8项本批新增，不叠加重复选择/历史/失败中间结果 |
| 默认生产lib与仅移除FreeBuff的代表性关线生产lib | 两项locked/offline/jobs4编译通过；关线不启用management-ui |
| 7份Rust官方`rustfmt --check` | 通过 |
| `npm.cmd run test:effective-lines --prefix scripts` | 31 passed / 0 failed |
| `npm.cmd run check:effective-lines --prefix scripts` | ratchet通过，4882文件；governed source >700为0，不等于strict或全仓功能通过 |

9个终态阶段各5196项选定输入before/after无漂移，且与最终源码逐项一致；7源码和5文档共12份范围文本UTF-8无BOM、写前hash、范围diff及`git diff --check`通过。公共guard、baseline、exceptions保持原bytes，根有效行报告按直接写前preimage恢复，scripts报告未变；本批生成的最终报告另存证据根，不覆盖既有报告现场。

官方lexer按真实preimage和终态复测：execution 153→165、decoded 111→122、claim测试索引213→216有效行；新增step 66、step_owner 117、step_failures 117、step_cancellation 160。7份最大216，4份新增最大160；无baseline/exception调整。安全/资源审查：复用既有body/签名大小上限和transport策略，无新后台任务/队列/重试、无raw秘密debug、无数据库格式或凭据改写；future的Send、实际body Drop与原claim不释放均有断言。同步prepare不可抢占、直接Drop与unknown义务的限制不以测试通过掩盖。

首个`red-v1`为缺少`execute_step`的E0599编译负测，不冒称行为红测。实现后`focused-v1`真实执行8项，7 passed/1 failed：Drop发生在响应头阶段，服务端handler可在构造TrackedBody前退出，等待body Drop计数不成立。修正测试为parser已观察首帧后才Drop，沿原test-only Notify确认阶段，保留计数/无后台FINISH/无重放断言；未因该失败修改生产取消或费用语义。另增加future的Send编译断言。旧失败回执保留，`focused-v2`完整重跑8项终态通过；audit核首轮失败之后仅两份测试有变化，生产源码未因夹具失败改义，不以7项中间通过作终态证明。

本批未跑前端/浏览器、全仓测试、原生EXE/包或真实平台，不以相邻lib与静态检查代替这些验收。串行verify进程正常结束；最终只读进程快照记录本批verifier为0、Cargo/rustc为0，无停止其他项目进程，见`final-process-inventory.json`。

## 4. 接力

仍需完整root/harness、实际工具/子agent及费用、START/FINISH持久义务/远端lease、公共请求预算与调用者/candidate绑定、正常UI逐次确认、三公共协议/closeout/下一次购买/probe，以及其余渠道和P50/P60/P70。模板仍17，无新增整卡`offline_ready`，总目标保持进行中。不读取真实凭据、不调用真实供应商、不操作Redis/Docker/现有实例、不出包、提交或推送，不复用已消费P34授权。
