# FreeBuff：请求拥有的一次性批准与真实管理权限

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承大量未提交修改，HEAD 不是本批实现的交付凭据；未提交、推送、出包或操作现有实例。

## 1. 本批闭合的边界

承接[原claim正常浏览器/真实Local TCP](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-claim-local-ui.md)，新增内部请求拥有的 pending/一次性批准 owner，并接正常管理路由的精确账号 list/decision。**本批没有真实公共请求 producer、正常UI逐次确认或完整 catalog executor；目录公共 guard 仍返回 `409/freebuff_business_admission_required`，不开放闲置购买。**

实际验证链是：生产管理 router / token / SecretGrant / revision → 内部活 waiter → 原 LocalRuntime sealed claim / fresh signed inspect → 唯一合成 admission POST。管理侧使用生产 router 的 in-process HTTP，不是本批新做的管理 TCP/浏览器；报价与 admission 使用真正随机回环 TCP peer，调用者是可信 typed fixture，不是公共 AuthAdapter 或公共生成请求。不能将这条内部组合称作公共调用已通。

## 2. 实现与生命周期

- [registry](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/freebuff_purchase.rs)：每个原请求最多一个 pending，最多16项未过期 slot；TTL取原 quote 剩余60秒与传入请求剩余预算较小者，不因显示/确认续期。随机 pendingId/quoteId 只定位原条目，DTO不可还原 quote 或购买许可。
- [binding](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/freebuff_purchase/binding.rs)：从 `PipelineContext` 的服务端 req_id 和可信 AuthenticatedSession 捕获身份；排除 body user、任意 metadata/header、客户端 x-request-id 和原 bearer。绑定 tenant/project、具体调用key、requesting/source key、入口/stream、精确 provider/payload credential、quote-time revision、模型及冻结请求内容 SHA-256。仅 synthetic dev/shared key、缺精确账号、DB credential candidate，或candidate与当前lookup的payload/token不一致时不构造binding；若二者同时更新，原quote仍在消费时核token/origin/device scope。这不独立证明运行模式或全部模型权限。
- 精确账号校验复用 `select_credential_probe_target`，不重新调度或推进 round-robin。模型 eligibility 仍归真实 `stage_send` owner；不能用 provider ID 替代本地池 `payload.credential_id`。
- [wait/consume](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/freebuff_purchase/waiting.rs)：oneshot只交付一次决定，原请求保留不可Clone/Deserialize的 quote 与 Lease。批准后 entry 不提前释放；原请求 Drop、取消、超时或 drain 关闭后不购买、不重放。已批准对象也要再次核 lease、原TTL和动态权限，随后把同一个 fence 贯穿已有 admission 的 fresh inspect、commit前后及POST前后。
- 不新增后台任务/定时扫描/数据库或第二套退款机制。清理为有界惰性清理和请求-owned timeout/Drop；slot不是所有入站请求、已过期但仍被内部调用者持有的对象或全部网络并发的全局上限。将来 producer 必须保留既有请求取消/预算 owner。
- [管理路由](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_pending.rs)：`POST .../freebuff-pending-purchases` 只读已有条目；`POST .../freebuff-pending-purchases/decision` 要求原 provider/credential/revision、pendingId、quoteId及显式 approve/cancel，只返回 `delivered:true`，不生成报价、购买、启动模型或复活已取消请求。严格DTO/16KiB/no-store包括错误边界。
- 动态 fence 每次重新认证**当前 management token**，再验证 SecretGrant/revision及原已保存账号；不是保存 actor 或 `approved:true`。本批不扩大其他旧管理操作的权限合同。队列锁内无外部鉴权回调/await，批准 fence 不存入 registry，未形成 registry→AppState→registry 的 Arc cycle。

已发POST后发生撤权/超时不能撤销远端操作；原 owner 保留已持久化 claim/未知义务，不承诺回滚、锁价、服务端金额 ceiling 或自动退款。已有 durable claim 的原身份恢复继续由既有 owner负责，不能把“恢复成功”冒充一次新购买。

## 3. 后续公共 producer 的精确合同

`capture` 的 `final_body` 在本批指 **harness提供的冻结请求意图**，而不是尚未准入就能知道的最终签名 wire。`run_id`/原 session/client metadata 要到原 admission/START 后才由 `CatalogRun::prepare_completion` 的 host owner追加；不能为拿最终wire摘要而先购买，也不能拿客户端回显摘要作许可。后续 executor 必须持有同一冻结意图，用它重新capture并形成实际completion，仅允许该host owner追加运行身份；最终实际bytes仍由已有签名owner冻结/签名/发送。当前没有这条公共消费者，因此本批不声称验证了意图到最终wire的一致性。

接线时必须从产生quote时的同一当前snapshot、真实已验证candidate、原活请求捕获；不能误用较早cash route snapshot或让pending跟随refresh自动换账号。等待留在原请求future、并发permit取得之前，不包装/取消额度reservation事务，不增加outbound attempt，不spawn detached admission。确认前后变化应失败重报，不能以显示过DTO或历史确认绕过。

## 4. 可复核验证

证据根：[gateway-freebuff-request-consent-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-request-consent-20261107)。原始命令/log、每阶段选定输入hash、preimage、范围diff和终态审计保存在该目录。无真实凭据/平台、Redis/Docker、现有服务、发布或提交操作；P34单容器旧授权不复用。

| 检查 | 本批结果 |
| --- | --- |
| 正常默认配置FreeBuff库选择 | `freebuff-v3`：329 passed / 0 failed / 2 ignored，包含12项新增；两个旧浏览器host本批未显式执行 |
| 相邻请求生命周期 | `lifecycle-v3`：6 passed；取消/预扣/退款owner未替换 |
| LocalRuntime选择 | `local-runtime-v3`：49 passed，与FreeBuff选择有交集，不直接相加 |
| 正常生产lib | `prod-v3`：通过；保留warnings，包括尚未接公共producer的dead code，不用suppression隐藏 |
| 代表性关线 | `disabled-v3`：1 passed / 0 failed / 0 ignored，两个管理操作均在解析pending身份前拒绝；`disabled-prod-v3`生产lib检查通过。从runtime-defaults显式移除FreeBuff且不启用management-ui，不是完整feature矩阵 |
| 静态与来源 | 19项Rust官方格式、checker31/31、ratchet4873文件通过；governed source >700=0，14项immutable runtime资产单列。源码最大434、新增最大226有效行；baseline/exception不改 |
| 终态去重/审计 | 按feature harness与完整测试名去重376 passed / 0 failed / 2 ignored（正常配置375、关线配置1；含12项正常新增及1项关线新增）；8个终态阶段各5187项选定输入与当前源码一致且阶段内无漂移。checker实际scripts输入未变，复用31/31回执。24份范围文本UTF-8无BOM/范围diff/`git diff --check`通过，公共guard/baseline/exceptions保持原bytes；本批有效行报告独立保存，根报告按直接preimage恢复，不以旧报告冒充本批结果 |

12项正常新增覆盖：纯读零购买、完整冻结报价显示/不泄漏材料、显式取消/Drop/TTL/迟到确认、批准后仍占slot、容量16、drain、单原请求去重、错误Grant/origin/revision/quote/foreign账号与注入字段、管理key撤销和Grant撤销、真实caller/tenant/source/req_id/内容/候选/模型/stream变更网络前拒绝；明确批准时管理route零POST/零row，随后原内部请求一次POST/一条sealed claim；价格变更及消费前撤权/Drop/drain均零新义务。旧quote测试另补原签发TTL与模型检查，不增加测试个数。

## 5. 失败记录与未完成

第一条行为红测为新增管理读取原先404而预期200，不是仅缺符号的编译红测。随后保留：UUID未启serde的6项编译错误（改为边界String/显式UUID解析，不改依赖）；首批运行1 passed/11 failed暴露重新调度推进RR，已改精确lookup并补20次capture不推进游标；该批构建期间补充源码有漂移，不算最终证据。第二批9 passed/3 failed均在既有清理helper拒绝新fixture目录前缀，业务断言已执行；修正fixture为要求的 `gateway-local-`，未放宽清理guard。失败生成的本批linshi现场保留。一次apply_patch因格式后上下文不匹配未改文件，后按真实原文精确替换；所有失败不计入最终通过。

本批只退役内部批准传输/真实管理权限与持久admission组合缺口。完整root/harness/工具与子agent、多步与取消费用FINISH、持久run/远端lease、实际公共producer/请求取消/预扣联合、正常UI逐次确认、公共实时三协议/closeout/下一次购买/probe、新设备和目录应用的UI→TCP联合、Server/跨页面/独立进程/原生包及真实平台费用均继续。累计新增模板仍17，无新增整卡 `offline_ready`；P50、P60、P70与总目标未完成。
