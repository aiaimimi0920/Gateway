# FreeBuff：原 claim 显式结束、最终回执与本地持久收尾

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。包含大量继承未提交实现，HEAD 不是本批源码交付凭据。未提交、推送、出包或操作运行实例。

## 1. 本批完成的内部合同

承接[冻结报价/持久购买准入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-purchase-quote.md)与[原 claim/run/context](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-run-context.md)。增加 crate-only 的原 claim 显式 end 与加密历史，不新增闲置购买路由、后台 end、重试或公共调用许可。目录公共 guard 继续在网络前返回 `409/freebuff_business_admission_required`，没有整卡 `offline_ready`。

- [SessionRecovery::end](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime/freebuff_sessions/closeout.rs)消费原 OS guard，先核当前 binding/attempt/原 sealed bytes 的 preimage，再对同一 CLI instance/attempt 发一次 signed `DELETE /api/v1/freebuff/session/attempt`。无 body，签名覆盖空 bytes；无 writer transaction 跨网络，不要求原模型仍可选。
- [SessionEndReceipt](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/session_end.rs)只有在 HTTP 200、业务 `ended`、金额类型有效时才返回 sanitized facts。明确 numeric refund（包括 **0**）、无 pending 且无 grace 才是 final。金额缺失、`freebucksRefundPending=true` 或原 `instanceId` 仍在 grace 都保留原 active row；不是退款成功。foreign instance、404/none、错误类型/负数、坏 JSON、redirect、超限、超时或取消不授权清除原 claim。
- final receipt 的加密归档、原 claim sealed 保存、原 active row 的 exact CAS DELETE 与最后 fence 在**同一个 SQLite writer transaction**中；CAS/SQL/fence 失败回滚，不留下只保存归档或只删义务的半份结果。guard 贯穿网络与本地收尾，失败不换 attempt 重买。
- additive `local_freebuff_session_closeouts` 使用原 schema-1，不升级版本或改写旧 claim。所有 sealed owner 的启动计数纳入历史；只剩历史、seal 丢失时不得生成新身份。历史 reader 核账号 authority、原 claim/attempt 与独立 receipt AAD，只返回原稳定模型和最终事实，不恢复 active capability、不授权 POST。

调用者的 fence 目前仍是**内部回调**，测试使用模拟撤权检查三个边界；尚未接入真实管理 Grant/revision、正常 UI、public caller/receipt/candidate，不能称为正常管理撤权全链。用户或上层将来必须显式执行 end；Drop、到期、GET 的 ended/none 不会自动释放 slot。

## 2. 退款与未知结果

end 保留已购买的小时，官方注释说明 refund 通常为 0；本批不承诺提前结束能退额，不把资金来源相加猜总退款，也不将 Freebucks 换算为 Gateway Key 余额、Token 或美元。`refund_sources` 只保留已返回的非负 finite 数值。

end 已发送但响应不明时，保留原 claim，由后续显式同 instance DELETE 对账；不自动补发。若在 commit-await 附近超时或取消，调用方不能仅凭错误断言事务一定未提交，应检查**原 claim 与 closeout 历史**。数据库原子性保证最终不会半份提交，不代表外部调用者已经知道提交结果。内部 timeout 文案不谎称“什么都没发生”。

历史没有清理/覆盖策略，本批不删除旧历史；它不是新准入或费用许可。裸 `ended`、状态轮询合成值与404均不足以证明原购买最终结算。

## 3. 联网核实与证据

匿名取得固定公开仓库 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的四份原文，URL/UTC/字节数/SHA-256 见证据根 `sources.json`：

- [types/freebuff-session.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/types/freebuff-session.ts)：L939–977 说明 optional 最终退款、幂等重取、pending 与 grace 语义。SHA-256 `1412f63d71061f39f8c91e5143f88684752077434055fa2c6981d2c539a7f572`。
- [freebuff-session-api.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-api.ts)：L294–307 的 attempt DELETE/原 identity/空 body；404 在客户端合成为 none，Gateway 不据此清除未知购买。SHA-256 `05d45150a8e58ce2d6209528929b0605066635020fba7c3a16878279f073add6`。
- [对应测试](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/__tests__/freebuff-session-api.test.ts)及完整 Apache-2.0 LICENSE 一并归档。只提取行为事实，未复制、安装或执行第三方实现；固定客户端不是当前服务端、实际购买或实际退款有效性的证明。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-claim-closeout-20261107`。直接写前副本、阶段源码 hash、原始日志、范围 diff、官方有效行数及最终审计独立保存，不覆盖前批回执。

## 4. 验证记录

首轮 red 是缺失 `session_end`/`end`/历史 reader 的**编译红测**（18 errors），不是运行时行为红绿。首版定向 `freebuff_claim` 20/20 通过、4258 项所选输入无漂移；随后补历史-only seal、密文/attempt 篡改与发送前 stale preimage 保护测试，并修正 end 专属未知结果文案。该20项属于中间证据，不与最终结果叠加。

最终验证：

- FreeBuff 名称筛选 **291 passed / 0 failed / 1 ignored**；LocalRuntime **50/50**。按 harness 与完整测试名去重 **332 passed / 0 failed / 1 ignored**，其中 **14 项新增**；不叠加中间20项或历史计数。唯一 ignored 为既有的专用目录/浏览器 UI fixture，本轮未执行。
- 新测试检查签名/原identity/空body/无自动POST、0与4.25回执及来源、pending/grace/缺金额保留、原模型locked、authority绑定、九类不确认响应、foreign/零budget网络前拒绝、redirect不携密follow、坏JSON/超限/timeout/body Drop、三个模拟fence撤销点、网络期间第二LocalRuntime互斥及绕过写入后的exact CAS、SQL失败整事务回滚、网络前preimage改变零DELETE，以及历史-only缺seal与密文/attempt篡改。它们不是独立进程end或真实管理撤权验收。
- 正常 runtime-defaults 与从该集合仅关闭 FreeBuff 的代表性**生产库检查**均通过；不是完整 feature 矩阵。保留109/130项生产 warnings，包含尚未接正常入口的内部owner dead code，没有 suppression 或依靠放宽生产限制让测试通过。
- 12个Rust文件官方Rustfmt、checker **31/31**、ratchet **4834文件**通过；14项immutable runtime单列，`governed source >700=0`。最大 **226有效行**，五个原文件写前/后为144→144、210→226、192→196、115→125、204→208，均低于250；未改baseline/exception，根有效行报告按直接preimage逐字节恢复。
- 最终7个适用阶段各 **4259项选定输入**阶段内无漂移且与终态逐项一致；初始4252项只允许5个原Rust修改与7个新Rust输入，未删旧输入。17个范围内文本均UTF-8/无BOM，四份固定公开原文hash、Markdown本地链接、公共guard原bytes、范围diff与`git diff --check`均经[audit.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-claim-closeout-20261107/audit.json)核对。所选输入不代表所有动态输入或运行环境都已冻结。

## 5. 明确未完成与接力

本批只退役**内部原 claim 收尾**缺口，不是正常管理/公共业务 closeout 已接通。仍缺真实管理身份/Grant/revision 到 UI 的逐次确认与 end 入口、可信 public pending/一次性 receipt/candidate、真正排队触发、完整 root/harness/decoder、费用累计/FINISH、持久 run/远端 lease、精确业务 probe 与完整取消。没有实现猜测的 reuse、账号级泛化 release 或 Desktop purchase identity。

Server/跨进程/原生 EXE、当前线上协议和真实账号/费用/退款，以及全范围 P50/P60/P70 继续。累计模板仍17，历史 quote 回执的16保留为当时事实，未新增整卡 `offline_ready`。本批没有读取真实凭据、购买供应商会话或调用真实模型，也没有新建 Redis/Docker 容器、修改现有服务或重复使用已消费并清理的 P34 授权；总目标保持执行中。
