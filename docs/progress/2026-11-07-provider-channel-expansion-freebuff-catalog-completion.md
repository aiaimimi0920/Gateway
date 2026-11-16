# FreeBuff：目录 native completion 解码、已报告 credits 与显式单步 FINISH

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。本批包含未提交实现，HEAD 不是这些源码的交付凭据。未提交、推送、出包或修改运行实例。

## 1. 本批范围

承接[原 claim/run/context](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-run-context.md)与[原 claim 显式结束](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-claim-closeout.md)。本批只增加内部 native JSON/SSE 聚合解码、响应报告的 SDK credits，以及原 run 的显式**单模型步骤** FINISH。不是完整 `base3-free-catalog` agent harness、实时公共流或公共业务解禁。

- [CatalogCompletionResponse::decode](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution/decoded.rs)继续持有原 run/active claim 的独占借用，实际读取 HTTP 200 的 JSON 或 SSE；借用贯穿 body EOF 与显式 FINISH。沿用共享 64 MiB body 上限、请求中的原总 timeout，并另限本次 decode 预算最多300秒。
- [native decoder](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/completion_decode.rs)保留多 choice、文本、reasoning/reasoning_content、reasoning_details、tool fragments/extra_content 与非空 usage。严格 UTF-8、SSE frame/完整 EOF、真实 finish reason、身份一致性及有界 tool/index；不执行或修复工具参数。新 tool ID 不与同名旧调用或稀疏显式 index 混合。
- JSON/SSE 的 error、坏结构、无真实 finish、截断、完成 choice 后追加、DONE 后新数据、非法 token/cost 或超限均不视为正常推理。`[DONE]` 可以缺省，但单独 DONE、正常 HTTP status 或裸 EOF 不是成功；观察到真实 finish/DONE 后仍须等实际 EOF。
- [Accounting](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/completion_decode/accounting.rs)分别保留最后一次**非 null 数值** `usage.cost` 与 `usage.cost_details.upstream_inference_cost`，不按 chunk 累加。完整验证新 snapshot 后才更新字段；真实 numeric 0 与两字段均未知分开。按固定 SDK 的 `Math.round((cost + upstreamEstimate) * (1 + 0.055) * 100)`换算一次；非 finite、负数/坏类型或超 JS safe integer 不生成费用事实。
- `DecodedCatalogRun::finish_single_step(self, budget)`只供明确由 host 拥有的**一次模型调用、无工具/子 agent 执行**。状态从解码结果得出，费用由已报告组件得出，不允许公共输入指定 status/费用；缺费用返回 `freebuff_business_accounting_unknown`，不伪造0。completed/failed仅指这个明确单步，不能用于给完整多步 agent 结案。
- [FINISH wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/finish_wire.rs)复用 START 的原账号/目标/run transport：一次 Bearer `POST /api/v1/agent-runs`，`action=FINISH`、原runId、`totalSteps=1`、`directCredits=totalCredits=该单步credits`、`steps=[]`。失败只发送固定安全错误码。没有设备签名、acting-user猜测、redirect或自动重试。

原 claim 不因 FINISH 释放；先前显式 session end/receipt/SQLite closeout仍是另一条流程。返回的 `finish_http_status`只证明HTTP接受，不是最终结算、退款、完整agent成功或真实账单回执。Drop/取消不自动发送FINISH，不排后台清理；已经发送而响应不明也不自动重放。尚无持久run journal、重启FINISH恢复或已证服务端幂等合同。

## 2. 联网核实与参考边界

本批匿名获取固定公开仓库 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`；URL/UTC/HTTP/字节数/SHA-256保存在证据根的`sources.json`。13条raw请求11成功、2个404，没有用失败路径猜实现。

- [limits.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/constants/limits.ts)明确`PROFIT_MARGIN=0.055`，补齐[此前调查](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md#5-2026-11-07-补充调查root-与费用不解除公共-guard)只核到re-export的缺口。SHA-256 `b782039cffc5585608d3b530c7a8b3adcfc3d1336b0a27920e059928f38fc3c3`。
- [model-provider.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/model-provider.ts) L584–645从JSON/SSE取得两组件并分别用最后值覆盖；[llm.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/llm.ts) L79–83、442–458相加换算并单次回调防重。Node实际非负舍入对照保存在`rounding-node.log`。
- [OpenAI-compatible Chat provider](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/packages/llm-providers/src/openai-compatible/chat/openai-compatible-chat-language-model.ts)是这条native JSON/SSE的参考，不借Responses fallback。客户端宽松补齐或synthetic finish不当作服务端真实终态。
- 固定commit的匿名codeload归档18,458,459 bytes，SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`；保存198份agent-runtime TypeScript只读参考，各文件来源/hash见`archive.json`。有限额读取选定成员，无`extractall`、依赖安装或第三方代码执行。
- [run-agent-step.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/packages/agent-runtime/src/run-agent-step.ts) L446–452累计step/direct/total；L2439–2456、2464–2524、2563–2599分别处理completed/cancelled/failed并保留已累计费用；`totalSteps`来自agent loop，不是token/chunk。子agent加父total、不加父direct。官方成功路径对background处理存在fire-and-forget，不描述为一律await。
- [database.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/database.ts) L475–529给出FINISH的实际Bearer transport和字段。该客户端没有证明当前服务端结算回执或幂等；本批不将这些未知点伪装成已确认合同。

取得完整Apache-2.0 LICENSE；仅按上述行为事实编写Gateway框架代码，没有把第三方实现复制或执行进仓库。固定公开客户端不是当前线上wire、真实资格或实际费用有效性的证明。SDK credits、上游推理估计、Freebucks小时价格与Gateway用户扣费分别保留，不互相冒充或擅自换算。

## 3. 验证与证据

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-catalog-completion-20261107`。写前副本、阶段hash、原日志、固定公开原文、范围diff与最终审计独立保存，不覆盖前批回执。

`red-v1`是尚缺decoder/accounting与测试类型导入的4 errors**编译红测**；`focused-v1`因测试helper漏导入返回类型而1 error编译失败，不为此扩大生产export。`focused-v2`实际运行38 passed/2 failed：夹具先默认写JSON Content-Type再追加SSE/HTML，导致测试收到重复媒体类型。修正夹具唯一Content-Type，不放宽生产content-type或成功规则；另补真实EOF悬挂、单次/累计body预算、numeric0 FINISH、零预算及稀疏tool index保护。

终态验证如下，不叠加中间42项或历史通过数：

- FreeBuff筛选 **306 passed / 0 failed / 1 ignored**；LocalRuntime **50/50**；共享`upstream_body` **12/12**。按harness与完整测试名去重 **359 passed / 0 failed / 1 ignored**，其中 **15项新增**。唯一ignored是既有专用目录/浏览器UI fixture，本批未执行；不是全仓回归或真实供应商验收。
- 新增保护覆盖native多choice/reasoning/tool/extra与usage、任意字节分片/中文、真实finish与严格EOF、坏UTF-8/JSON/错误体/截断/DONE后数据、身份及choice冲突、稀疏index/同名并行工具、单次和累计64MiB预算、last-value/未知/0/舍入/溢出；实际SQLite原claim→START→signed JSON/SSE→明确0或40credits单步FINISH、错误后26credits安全FINISH、未知不造0、零预算/redirect/500/bodytimeout/取消不重放或释放原claim，以及完整finish/DONE后EOF悬挂不假成功。
- 正常`runtime-defaults`与从该集合仅关闭FreeBuff的代表性**生产lib检查**均通过，不是完整feature矩阵。分别保留132/153项warnings，包含内部owner尚未接正常入口的dead code；没有suppression或放松生产准入。
- 18个Rust文件官方Rustfmt、checker **31/31**、ratchet **4845文件**通过；14项immutable runtime单列，`governed source >700=0`。最大271有效行；七个原Rust写前/后为196→200、265→267、148→149、82→101、271→271、208→210、177→197。原business与共享reader各为单一职责且低于500，其余本批文件均低于250，新生产文件最大192；未改baseline/exception。
- 最终8个适用阶段各 **4270项选定输入**阶段内无漂移且与终态逐项同hash；初始4259项只允许7个原Rust修改与11个新Rust，未删旧输入。15份直接preimage、23个范围文本UTF-8/无BOM、公共guard原bytes、baseline/exceptions及根报告逐字节恢复、范围diff与`git diff --check`均经[audit.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-catalog-completion-20261107/audit.json)核对。11份成功raw原文/2个404记录、归档SHA及198份选定成员与归档原bytes、本地链接路径存在性一并核对；所选输入不是所有动态输入或环境已冻结的证明。

## 4. 仍未完成与接力

公共`freebuff_business_admission_required` guard保留原bytes。正常管理/UI的逐次价格/余额确认、真实Grant/revision到public pending/一次性receipt/candidate的可信绑定、真正排队触发、完整root/harness/工具/子agent与多步费用累计/取消FINISH、实时公共三协议、持久run/远端lease、精确业务probe和完整取消仍未接。单步API不能拿来代替这些owner。

管理/UI/public end与下一次购买接线、新设备/目录/会话UI到真实TCP、Server/跨进程/原生EXE及当前平台账号/资格/费用/退款继续。没有读取真实凭据、购买会话或调用真实模型，没有Redis/Docker、运行实例、出包或Git发布操作，不复用已消费并清理的P34授权。累计模板仍17、无新增整卡`offline_ready`；P50/P60/P70与总目标保持执行中。
