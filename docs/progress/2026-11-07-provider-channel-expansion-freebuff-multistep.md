# FreeBuff：原 run 顺序模型步骤、累计费用与协作取消 FINISH

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承未提交改动，HEAD不是本批实现的交付凭据。本批定向验证及来源审计已取得终态；仅闭合下述内部子链，整体计划继续。

## 1. 本批实现与精确边界

承接[请求拥有的一次性批准](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-request-consent.md)，继续补完整执行器必需的顺序模型步骤与费用owner。**不是完整base3 root/工具/子agent执行，不开放公共producer或正常UI购买，不修改公共`freebuff_business_admission_required` guard。**测试的tool-call/result由可信host夹具提供，仅证明历史与签名连续性，不是Gateway执行文件、命令或其他工具。

- [CatalogRun](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution.rs)继续按move移交原run/claim；完整decode成功且真实EOF后，`continue_run(self)`才返还同一唯一run和本次completion。错误/取消返回原decoded owner，不允许继续。每步仍核原scope/模型/live observation，保留同一runId/clientId/claim instance，逐请求冻结并签实际bytes；不另START、不复用旧bucket/轮转租约或固定0费用的Drop FINISH。
- [累计owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution/sequence.rs)为O(1)状态，每个消费过的decode只入账一次；每步先按原SDK公式round为reported credits再相加。单步last-value observation不跨请求继承，unknown和累计overflow不能被后续已知或0修复；JSON safe-integer上限是Gateway本地数值安全政策，不声称上游server合同。
- `RunTotals`与最后一步`Accounting`分开，步数只表示本lane已decode的模型尝试，不冒充完整agent的所有loop/tool/programmatic步骤。无实际工具/子agent费用owner，因此本lane direct=total；后续真实工具和child必须按实际回调及ancestry接入，不能继续用这个相等关系代替全部费用。
- `finish_run(self)`发送原run一次显式FINISH，completed/failed/cancelled区分；失败保留先前步骤和本次已观察部分费用。旧`finish_single_step`对累计非1步拒绝，不默默截断，也不承诺误用后返还owner。`steps: []`沿固定SDK无pending记录的实际wire，不伪造逐步记录。
- [协作取消](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution/decoded.rs)覆盖decode期间与步骤之间。`decode_until`停止读取并返还原owner和已观察费用，host随后必须单独await显式FINISH；`finish_cancelled`可结束步骤之间或已确认START、尚无模型步骤的run。后者0是“未发模型步骤的reported模型credits”，不是session购买免费或真实供应商扣额为0。
- FINISH不以短TTL observation仍live为前提，避免过期后无法报告原run；这不放宽下一次completion准入。FINISH HTTP2xx仍不是结算receipt，不释放session或持久claim。Drop、超时和未知结果不自动FINISH/重放/重新购买，没有后台任务或第二套退款机制。

## 2. 固定公开参考与未验证范围

复用先前已匿名下载的Apache-2.0固定commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`：SDK database/llm、公共数据库合同、limits、run-agent-step与LICENSE，共6份文件，本批逐项核原download/archive receipt SHA-256；没有重新联网，不宣称已核当前线上合同。原本的completed/failed/cancelled与direct/total累计来源、空steps行为见[业务合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md)。没有复制/执行第三方实现或新增依赖。

本批新增的是内部借用链能力，不是公共请求取消owner。prepare/send失败仍使用既有错误接口，不交回run；直接Drop、START结果不明、FINISH失败/被丢弃和进程退出尚无durable run恢复。本地至多一次显式send不等于远端exactly-once。已观察的部分credits也不是最终账单；unknown时拒绝发送伪造数字，并不代表已完成FINISH或免除远端义务。晚到费用、完整工具/child累计、远端lease与持久义务继续。

## 3. 验证回执

证据根：[gateway-freebuff-multistep-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-multistep-20261107)。原始命令/log、阶段输入hash、写前preimage、固定来源对账、范围diff和审计均保存在该目录；临时数据库/失败现场也在linshi，不读取真实凭据或现有`.ng`。

| 检查 | 结果 |
| --- | --- |
| 正常FreeBuff、相邻生命周期与LocalRuntime | FreeBuff 339 passed/2 ignored，lifecycle 6/6，LocalRuntime 49/49；按同lib harness与完整测试名去重 **385 passed/0 failed/2 ignored**，含10项新增定义。两个旧浏览器host本批未显式执行，不叠加中间或历史选择 |
| 正常/代表性关线生产lib | 默认生产`cargo check --locked --offline --lib -j 4`通过；从`runtime-defaults`仅移除FreeBuff且不启用management-ui的代表性生产lib通过，不作为完整feature矩阵 |
| Rustfmt/checker/ratchet与来源审计 | 12份Rust官方格式通过；checker **31/31**，ratchet扫描 **4878文件**、governed source >700=0通过；8终态阶段各5192项选定输入与终态逐项同hash且阶段内无漂移。17份范围文本UTF-8无BOM、直接preimage、范围diff/`git diff --check`、6份固定原文SHA及公共guard/baseline/exceptions原bytes核验通过；根有效行报告保存在证据根后按直接preimage恢复，scripts报告原bytes未变 |

新增10项测试：同原run两步JSON/SSE及tool历史/实际bytes独立验签、每步round再累计、unknown跨步骤污染、独立last-value/已知0、safe-integer overflow不饱和、已知/未知部分费用的协作取消、步骤之间/首步之前显式cancel、失败后不继续与累计安全failed、旧one-step误用/Drop不发FINISH；原claim保持、另一runtime执行guard互斥有实际断言。取消测试复用原test-only body witness在parser观察后发通知，不依赖定时器猜已读取费用。

12份源码最大269有效行，5份新文件最大153；既有execution149→153、decoded123→111、finish_wire23→29、business267→269、completion_decode28→28、accounting60→61、claim_owner_tests210→213。没有调整baseline/exception；新责任独立在sequence模块，未扩大历史超限文件。本批生产/测试源码终态已冻结，之后仅补5份Markdown，不以文档更新重复Cargo验证。未执行完整全仓回归、前端/浏览器、EXE/devtest、Server/跨进程或真实上游验收；既有warnings保留，不执行无关`cargo fix`。

串行验证进程已exit0；收尾只读inventory未见本批证据Python或Gateway测试进程。系统当时另有3个Cargo进程，命令均明确指向`C:/Users/Public/nas_home/Otter/native/Cargo.toml`，不属于本批，未终止或修改；原始inventory在证据根，不据此宣称全机无构建进程。

## 4. 失败分类与接力

真实行为红测`red-v1`已经执行两个completion，原FINISH却仍是1步，断言left=1/right=2失败；不是缺符号的编译红测。首轮全选择编译因共享常量的re-export可见性和test-only witness的super路径产生3项Rust错误，最小修正对应可见性/路径，没有改生产费用/取消语义。临时measure helper首轮读取尚未创建的计划文件报ENOENT，改为仅统计已存在文件；未改checker/阈值/baseline/exception。失败回执/现场保留，不计为通过。

下一步继续完整root/harness、实际工具/子agent与费用回调、prepare/send及公共请求的取消/预算联合、持久run/远端lease，再接真实公共producer/正常UI逐次确认、公共三协议/closeout/下一次购买/probe。模板仍17，无新增整卡`offline_ready`，P50/P60/P70和总目标未完成。本批没有真实凭据/供应商调用、Redis/Docker、现有实例修改、出包、提交或推送，不复用历史P34容器授权。
