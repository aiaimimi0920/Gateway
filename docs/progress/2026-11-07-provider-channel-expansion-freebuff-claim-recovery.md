# FreeBuff：本地持久 claim 与未知购买恢复子批

日期：2026-11-07。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；包括继承的未提交改动，不把 HEAD 当作实现交付凭据。

## 1. 本批范围

已实现内部 LocalRuntime admission owner：真实 SQLite 先保存 claim，再允许调用一次 signed POST；已有记录只能恢复原 claim，不因取消、超时、到期或不符合预期的回复自动重买。恢复不要求原模型仍在当前目录可选，且只把精确原 instance / stable model 的未到期 active 回复转换成短生命周期观察。

**这是正常执行的持久化与恢复基础子链，不是 FreeBuff 公共业务已经接通。** 没有新增闲置购买按钮、管理消费接口或公共自动购买；目录模式仍在任何旧执行网络调用前返回 `409/freebuff_business_admission_required`。`MaySpendFreebucks` 仍只是内部 marker，不是当前价格/余额确认，也不是服务器金额 ceiling。

## 2. 实现与边界

- 新增 schema-1 的 `local_freebuff_session_claims` 表，旧数据库启动时增量初始化，不换版本、不重写旧 credentials/ID/余额。恢复复用原 `access-key-seal.json`，表中原 scope、UUID 和 stable model 的 receipt 使用独立 AAD 密封；不保存短效 handle/fetchId 或旧 token。
- 本地 slot 仅按 provider+credential 标识，**不随 token/origin/client 改变**。旧记录 scope 与当前 authority 不一致时阻塞并保留原记录，不能伪装成 absence。没有保留旧 authority，故不能声称自动恢复被替换 token 的旧购买；必须恢复适用原 authority 或由后续显式终态 owner 处理。
- `BEGIN IMMEDIATE` 内完成检查、容量限制和 INSERT，commit 后才能取得一次性 Send。固定 256 个 OS lock stripes，无等待式抢占、无路径输入、无 unlink；不同 slot 的 stripe 碰撞保守返回 busy。guard 覆盖发送/解码及 live observation，取消或进程退出只释放锁，不删除记录。
- 每条已有记录都保守代表 outstanding / unknown。恢复不构造 AdmissionRequest，不 POST、不自动 end/refund、renew/takeover、reuse 或轮换 UUID。`none`、`ended`、foreign、HTTP 错误、坏 body、原模型不符和观察过期都不能清除记录；本批尚无终态 closeout/再次购买许可。
- 恢复 authority 从同账号、设备和当前有效 catalog 获取 fetchId，不对原 stable model 执行 `select`，也不借另一个模型构造假 handle；原模型锁定/消失不会单独阻止 heartbeat。没有有效 catalog/设备/原 authority 时仍明确受阻，不 unsigned fallback。
- active 必须同时满足 HTTP 200、`status=active`、exact instance、原 stable model、必填类型、日期顺序、expiry 尚未到期及正 remainingMs。观察不可序列化，TTL 取 remainingMs 与 expiry-now 的较小值，再用单调时钟失效；不假设 admittedAt 与本机时钟同步，也不让 future admittedAt 延长 TTL。它不是 durable 永久执行授权、run lease 或余额结算证明。
- SQL 在读取异常 sealed/attempt 列时有界；损坏、移植密文、篡改 receipt 或丢失 seal 均失败关闭，不当作新账号。只有 session journal 的数据库也禁止静默创建替代 seal。

主要代码：[本地 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime/freebuff_sessions.rs)、[生命周期](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime/freebuff_sessions/lifecycle.rs)、[受限 receipt](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/claim_receipt.rs)、[恢复与 active 观察](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/owned_session.rs)。

## 3. 证据与验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-claim-recovery-20261107`。

真实红测编译后运行失败：隔离 LocalRuntime 的 SQLite 没有持久 session 表，`left: 0 / right: 1`，不是导入/类型错误。首轮 FreeBuff 名称筛选256 passed / 1 ignored，最终258 passed / 1 ignored；12个新增命名测试包含一个隔离child helper，其实际崩溃行为由父用例验证。最终LocalRuntime49/49，与正常FreeBuff按完整名去重298 passed / 1 ignored，不重复累加阶段。仅关闭FreeBuff164/164，扩展原关线用例证明不创建claim；正常及关线两项生产lib、Rustfmt、checker31/31和ratchet4765文件通过，14个immutable单列、governed source>700=0。

最终8阶段各4747项所选输入与最终源码完全一致且阶段内无漂移；不是全仓所有动态/脚本输入冻结。15个受审源码（9新增）最大372有效行，无501–700例外或baseline调整；UTF-8无BOM、范围diff与本地链接见`audit.json`，根effective-lines报告已按直接preimage逐字节恢复。现有编译warnings保留；唯一ignored为既有要求专用目录/浏览器的UI夹具，没有计为通过。

新测试使用真实 SQLite/seal、真实回环 HTTP 验签及实际子进程：prepare 后/HTTP body 取消后重建，原 claim 对账；一次 POST 后多个 prepare 不再 POST；进程持锁时另一 runtime 被拒绝，精确 fixture 子进程强制退出后锁释放而原身份保留；原模型 locked 仍能 GET 对账。child helper 不执行真实上游操作，不是原生 Gateway EXE 验收。

commit 故障测试分别在 INSERT 后/commit 前和 commit 后/Send 返回前取消，验证重开后只出现 absent 或原 claim recovery；**不是 SQLite 内部 fsync 中间点的故障注入证明**。公开参考只复核前批固定 commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的已下载文件哈希，没有宣称重新联网核实当前线上 API 或真实扣额。未读取真实 `.ng`/凭据、注册真实设备、购买会话、操作运行实例、创建容器或出包。

## 4. 继续开发

仍需将逐次消费确认、即时价格/余额/Grant/revision 重验、精确账号执行上下文与本 owner 接线；补 typed agent/run/lease、冻结 completion body、JSON/SSE decoder、取消/精确终态与计费/probe，再开放正常管理/公共路径。不能先放开旧 executor guard，也不能把本 owner 的原语测试当正常用户流程已可用。

终态 closeout、用户明确授权的下一次购买、长期 outstanding 容量治理、跨安装协调、Server 持久 owner、UI→真实管理 TCP 联合链、独立 Gateway 进程/原生包和真实平台扣额均未完成；`/session/reuse` 完整 wire 仍未证，不猜实现。原记录不自动过期删除的可用性代价须由后续 owner 显式解决，而不是重买兜底。

总范围仍见[总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)和[总状态](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-status.md)：模板仍累计新增 16，无新增整卡 `offline_ready`；R06/W04、其余渠道和 P50/P60/P70 都没有因本批而完成。
