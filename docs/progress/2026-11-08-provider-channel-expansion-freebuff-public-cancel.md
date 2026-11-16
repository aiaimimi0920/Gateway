# FreeBuff：公共付费阶段协作取消与原 run 费用收尾

日期：2026-11-08（Asia/Shanghai）。承接[正常网页与活公共请求联合](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-ui.md)。
工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。
继承大量 dirty/untracked 实现；HEAD 不是本批来源凭据。本批只推进目录 CLI / Local 非流式 Chat，不是整渠道完成。

## 1. 本批实现

- 删除付费执行外层会直接丢弃 run 的 cancellation select；仍保留报价等待、并发等待原有可取消 owner。
- 原付费 owner 在 admission POST 前、返回后、START 后和模型步骤内观察 caller watch、watch 关闭、shutdown 和原请求 deadline。已经发送的购买/START 不重买、不换身份，不凭取消猜远端结果。
- 确认 START 后才持有 deadline closeout guard：原请求到期仍立刻失去生成和新 attempt 资格，但原 dispatch 最多继续等待 20 秒完成费用收尾，复用 run transport 的既有上限。不 spawn generation/补偿任务，不修改全局正常请求期限。
- 已确认 START 但模型尚未发送时，显式原 run cancelled FINISH 的步数/credits 为真实已知的 0；可能已发送却无费用报告时拒绝 FINISH，不填 0。
- 模型步骤继续使用原 strict decoder 和累计 ledger。同一个取消 future 贯穿 prepare/headers/body；已观察到的 reported SDK credits 在取消后仍交给原 run 的显式 FINISH。
- FINISH 已在途时继续等待同一次 acknowledgement；若模型已完成，原 FINISH 保持 completed，不因晚到取消重发 cancelled FINISH。HTTP 接受不是结算回执；FINISH 从不 end/释放 session。
- 安全业务错误代码与原 request stop cause 同时保留。未知费用仍可显示 `freebuff_business_accounting_unknown`，但不会把 caller/shutdown/deadline 归成凭据永久失败。

同时修正相邻实际缺陷：函数工具请求只有一个 attempt slot，admission 已消费该 slot 后，旧 START/模型前检查错误地再次要求剩余 slot。
现区分新 send 准入与已准入操作的 lifetime/健康复核；后者仍检查 stop/deadline/handoff 和 mutable eligibility，但不多占 slot，也不能恢复任何已停止的 budget。
没有改变既有新 attempt 的次数、最大重试、健康判定、Grant/revision 或报价围栏。

## 2. 已执行行为证据

新付费阶段 9 个测试均通过，包含：

| 场景 | 原 run / 实际出站及 Local 事实 |
| --- | --- |
| 解析器已观察 cost 后 caller、closed watch、shutdown、deadline | 唯一原 run cancelled FINISH；steps 1、direct/total credits 13；不 DELETE，claim 保留，attempt 1，Key 次数归还 |
| START 在途取消 | 等原 START 返回真实 ID 后 cancelled FINISH；steps/credits 0；零模型调用，原 claim 保留 |
| admission 在途取消 | 等原购买结果，不再 START、不猜 run ID、无 FINISH/新购买 |
| 模型 headers 或没有费用的 body 取消 | 保留 accounting unknown；零 FINISH，不把未知当 0 |
| FINISH 在途取消 | 等唯一原 FINISH acknowledgement；不重发、不 end，公共调用不返回成功 |
| 正常公共 HTTP shutdown | 实际公共 TCP、Local auth/管理批准、合成 supplier TCP；解析器观测费用后 shutdown，FINISH 13 后公共 HTTP 503 |
| 真正 abort/Drop | 先丢弃真实模型读取；Local cancelled/refund/permit 收尾，无 detached FINISH、end 或 replay |
| START 无可用 ID / FINISH HTTP 拒绝 | 固定安全错误、原 claim 保留，无替代购买或 POST retry |
| 函数工具请求 | caller tools 原样到最终 wire；一次 admission/model/FINISH、attempt 1、完整 native JSON，Key 10→9，不执行 caller tools |

上述 caller/closed 信号通过真实生产 pipeline 的 watch 入口注入，不能据此宣称当前 Chat HTTP 已支持 WebSocket，或 HTTP future Drop 自动变为 cooperative FINISH。
真实 HTTP shutdown 另有正常路由测试。deadline 使用同一完整 pipeline 和其真实 budget owner，不冒称本批执行了 HTTP 配置期限的浏览器验收。

费用为合成 peer 的 reported SDK credits，既不是 Freebucks 小时报价/退款，也不是 Gateway Key 次数。
审计严格遵循既有 finalizer：caller/closed/shutdown 为 cancelled，deadline/真实上游失败为 failed；只有 caller/closed 标记 client disconnected。
断言实际 SQLite route attempt、余额、permit、terminationReason 和取消的空 failureClass，而不只看 HTTP 状态。

## 3. 验证结果、失败与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-cancel-20261108`。
Cargo 全部 locked/offline、关闭 management-ui、jobs 4；FreeBuff 和相邻测试 threads 1。

| 检查 | 结果 |
| --- | --- |
| 新公共付费阶段 | 9 passed，已包含下项，不相加 |
| FreeBuff 完整相邻 lib 选择 | 450 passed / 3 ignored；先前较窄 `freebuff_` 302 项包含在内，不相加 |
| request budget / rate gate / lifecycle / stage-send lib | 10 / 15 / 6 / 46 passed；其中 5 个新增 budget 测试全过 |
| 正常生产 lib / 代表性仅关闭 FreeBuff 的生产 lib | passed / passed |
| 共享预算既有 integration contracts | 12 / 8 / 2 passed，三个 harness 共 22 项 |
| 既有 pipeline send 的 budget / deadline integration 选择 | 5 / 7 passed；重叠 1 项，去重 11，不相加成 12 |
| 本批 Rust 最终合并 | 按 harness 与完整测试名去重 **560 passed / 0 failed / 3 ignored**；其中 lib 527、integration 33；14 个新增全部通过 |
| 14 Rust 官方 rustfmt / checker tests / ratchet | passed / 31 passed / passed，4,943 文件，governed source >700 为 0 |

保留真实失败，未把失败次数加入通过数：

1. 新夹具初次编译因既有 `TEST_BODY_STARTED` 为 private 报 E0603；只把这个既有 cfg(test) witness 的可见性改为 crate 内。模型前先排空 session-wire 通知，再由解析器通知证明真的观察到模型费用。
2. 真正行为红测在旧生产实现下得到 FINISH 0，预期 1，准确复现已知费用取消遗漏。
3. 新 Drop 测试使用 `unwrap_err` 不必要地要求 `PipelineOutput: Debug`，报 E0277；改为显式匹配 JoinError，未给生产响应增加 Debug 或放宽断言。
4. 首轮执行 3 passed / 6 failed，全部停在夹具把 cancelled 预期成 failed 的断言。核对原 `audit_persistence` / `StreamFailureCause` 后按精确原因修正夹具，并增加断连/terminationReason/健康中性断言；未修改生产 audit 分类。最终 9 项全部重跑通过。

上一批宽目标编译的 `tests/console_provider_toggle_health_contract.rs:23` / E0560 仍为已知继承问题，本批没有修改或重新声称全目标通过。
不重复未变化的前端/浏览器检查，不出包，不以生产 lib check 代替原生整包验收。

14 个 Rust 文件均小于 500 有效行，最大 269；7 个新文件最大 180。原预算 260→269、gate 216→223、公共 send 182→117，未增长任何 >700 源码。
本批源码/配置快照、各阶段命令、完整日志、direct preimage/new-file diff、UTF-8 无 BOM、保护文件和最终去重核验见该证据根。
选定输入快照不是 node_modules、外部路径依赖、运行时资产或完整宿主的 hermetic 声明。
3 项 ignored 都是既有 opt-in 浏览器宿主，本批不重新执行未变化的浏览器流程；上一批的显式正常网页验证仍是上一批证据，不加入本批通过数。
最终 14 个编译/测试/格式/ratchet 阶段各 4,408 项选定输入 before/after 与最终源码逐项相同；`final-audit.json` 已通过，18 份范围文本及 direct preimage/new-file diff 已留存。
`actual-ratchet-report.json` 保存本轮真正根报告，随后按本批 preimage 原 bytes 精确恢复 `artifacts/effective-code-lines.json`；保护的旧 guard、baseline、exceptions SHA-256 不变。
所有本批命令均已终态，成功 TCP host/peer 的正常 join 是测试断言的一部分；没有本批待收尾的浏览器、静态 helper 或容器。失败轮合成现场保留，不重放其购买/FINISH/end。

## 4. 未完成范围与下一棒

本批不承诺 HTTP 任务真正 Drop、进程退出、START/FINISH 结果未知后的自动恢复或持久 run/费用 journal。现有 durable claim 保留原购买义务，但不等于完整 run/lease 恢复。
没有后台 detached 补 FINISH、自动 end/退款、认领旧 claim、重试不确定 POST 或绕过逐次批准；真正 Drop 仍明确不发送 FINISH。

继续：原 run/START/FINISH 未知与持久费用/lease 恢复、其余公共协议/实时 SSE/模态/参数、完整 CLI host 实际工具/child 及费用、精确 probe、新设备/目录应用 UI 联合、Server/PG/Hosted、跨进程/原生包，以及真实平台资格/模型/额度验证。
正常 UI 原 end→新购买联合证据保留，不重放上一批失败现场。其余平台和 P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道真实验收继续。
模板仍 17，无新增整卡 `offline_ready`。本批无真实凭据/供应商、Redis/Docker、现有实例、出包、提交或推送操作，不复用已消费 P34 授权。
