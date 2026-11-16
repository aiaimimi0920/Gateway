# FreeBuff：root native history 准备进入原签名 run

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留继承 dirty edits，不提交或推送；HEAD 不是本批来源凭据。这是 R06 内部请求准备子链，不是整卡、公共业务或总计划完成。

## 1. 入口与本批改动

承接[默认 root builder](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-builder.md)：`BusinessRequests::build_root_step(step, limit)` 消费受信 host 的 bare native snapshot，先准备 history，再生成默认 system/逐 user instructions/当前 step reminder。`CatalogRun::execute_root_step` 传入原 limit，继续共用原单步预算、协作取消、run/claim/fetchId、最终 bytes 签名、decoder、reported credits 和显式 FINISH。不新建购买、START、重试或平行执行 owner。

新增 `root_builder/history.rs` 和 `merge.rs`。反向扫描借用原 tool ID，在同一真实 user 区段内，仅允许**位于 call 之后**的 tool result 解除该 ID 之前的全部 pending calls；下一条 user 是边界，assistant 不是。未获回答的调用从 native `tool_calls` 移除，其他已回答调用保留。迟到 result 不能跨 user 边界补答，但 freestanding/duplicate tool result 本身原样保留。本实现不是公开归档未包含的 web wire provider-specific orphan/duplicate sanitizer。

只在调用被删除、剩余 calls 已空且没有其他 payload 时移除 assistant；text/part-array、reasoning、refusal、audio 或其他 native 扩展保留。未知扩展可能携带原生状态，宁可保留，不把 metadata-only assistant 擅自当空消息丢弃；这是已经-native 输入与来源 internal part 模型的适配差异。修复后若整个 snapshot 为空，本地返回 `InvalidInput`，不发送只有 system 的 root 请求。

连续 assistant 合并：同形 content 字符串连接或 part-array 追加；`reasoning`、`reasoning_content` 字符串连接，`reasoning_details` 和 `tool_calls` 追加。空/null 内容不擦掉已有数据，user/tool 消息保持边界。已是 native 的 reasoning/details 不重新应用 internal→wire 的纯文本抑制规则，不猜 signature/model provenance。其他 metadata 遵循固定 converter 的 shallow **later-wins**，同名 opaque object 不做深合并，也不承诺每个早期冲突值同时存活。非空 string 与非空 part-array 不能按该固定来源无损合并时，本地明确拒绝，而不是 flatten 媒体或编造 part schema；这项拒绝是 Gateway 的 native 适配政策。

原生 JSON 的 `providerExecuted:true` 不是 SDK internal capability，不允许用它跳过本地 function call 配对。新增校验仅覆盖准备过程实际消费的 role、tool IDs、assistant content/reasoning/array 字段，不宣称实现完整供应商 schema 验证；可信 host 仍负责工具合同和 same-model native state。

## 2. 资源与所有权

沿用调用者 `1..=50 MiB`。在 pending/merge 分配之前，用 counting writer 对**原 bare history**计数，超过 limit 即拒绝；不保留第二份 JSON byte buffer，不以“稍后会删除巨大 call”为由绕过本地资源政策。此预检比最终 envelope 上限更保守：原 history 过大，即使清理后会缩小也拒绝。最终原 serializer 仍独立限制包含 system/tools/run metadata 的最终 body，并只冻结、签名这一份实际发送 bytes。预检不是 tokenizer、实际模型 context 上限或严格全进程 heap 上限。

最多 4096 条 history/1 MiB host context 的原政策不变。ID set 借用输入；owned native Values 移动，字符串/数组保留容量并 amortized-linear 追加，不逐次 clone 累积前缀。没有新后台任务、队列、文件/凭据/store/迁移、工具进程或远端副作用；校验/预检/merge 失败仍由原步骤 owner 返还原 run、零模型尝试。同步准备不可抢占，耗时计入原步骤预算，不承诺硬实时。

仅改变当前消费的 request snapshot。**没有写回 host canonical history、持久修复、自动摘要、token 计量或实际工具/child 执行。** schemas、合成 tool results 和提示中的权限不能授权 Gateway 访问调用者文件、终端、网页、UI 或安装技能；不搬入 Loom 实现。

## 3. 固定来源与不应误加的行为

没有重新联网；复核既有固定归档 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`，SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`。`common/src/util/messages.ts` 与其测试、runtime `run-agent-step.ts`、native chat converter、model-config 及 LICENSE/NOTICE 等 13 份原文逐项 SHA 与归档核对，见证据根 `reference-provenance.json`。未执行第三方 TS/SDK，不声称当前线上合同已验；继续保留原 Apache-2.0 LICENSE/NOTICE 和本目录修改说明，P50 仍须验证 notices 随 binary 分发。

来源 `supportsAssistantPrefill` 的消费者使用 `agentTemplate.model`，而不是 catalog compiled identity。opaque `fbm1.*` 不命中 Claude 版本正则，本批**不擅自追加 Claude continuation**；新增回归保护 trailing assistant 不被 compiled 身份偷改。来源 `withSystemTags` 确认为 runtime util/messages 中的 `<system>…</system>`；先前尝试的 `common/src/util/system-tags.ts` 不存在，保留 missing 调查记录。

来源 runtime 的 `contextPrunerBudgetForModel` **确有默认 400000**，明文 Kimi K2.7 Code 为 250000，明文 Flash/fast 为 312500；opaque handle 未命中特例时用默认。它是固定客户端摘要策略，不是已核实的模型最大 context，更不是当前服务端能力。row 缺 `maxContextLength` 不代表来源 runtime 没有默认；本批尚未实现真实 tokenizer/预算消费/摘要，不把 cache expiry/floor、lean 文本或近似 token 数冒充这些能力。

## 4. 终态验证与范围审计

初始真实行为红测 `red-v1`：**4 passed / 8 failed**，源码快照稳定。失败覆盖未回答调用清理、user 边界/重复 ID/原生标记与连续 assistant 合并，不是编译负测，也不声称它们是旧公共业务回归。

首轮修复后 `focused-v1` 为 **19 passed / 1 failed**，3项真实SQLite＋回环TCP均通过。唯一失败是4096条assistant的原始夹具143361字节超过通用helper的32768字节上限，实际返回`RequestTooLarge`符合新增预检。只将该长历史夹具改为显式262144字节预算，未放宽生产限制或改变生产源码；两个source快照逐项差异仅为`merge_tests.rs`。完整20项在`focused-v2`重跑通过，初始失败日志和`fixture-correction.json`保留，不把失败或重复通过数叠加。

| 实际终态检查 | 结果 |
| --- | --- |
| `cargo test --locked --offline --lib freebuff_root_history_ -j 4 -- --test-threads=1` | **20 passed / 0 failed / 0 ignored**；含3项真实SQLite＋回环TCP |
| 相邻 `--lib freebuff` | **393 passed / 0 failed / 2 ignored**，包含上述20项，不相加 |
| 默认生产lib、仅移除FreeBuff的代表性关线生产lib | 两项locked/offline/jobs4通过；关线不启management-ui，未跑关线单测或完整feature matrix |
| 9份Rust `rustfmt --check` | 通过 |
| effective-lines checker tests | 31 passed / 0 failed |
| effective-lines ratchet | 4899文件通过，governed source >700为0；不冒称strict或全仓功能通过 |

TCP成功场景实测清理/合并后的native消息进入原runId、原catalog fetchId和最终原bytes签名，reported SDK credits为26，显式FINISH只报告1个模型步骤，原sealed claim row不变。坏history和原history超限在网络前拒绝，返还原run，只有heartbeat/START/显式FINISH、零模型发送；其中0只指未发模型步骤的reported模型credits，不代表购买免费或真实供应商扣额为0。未执行新工具，也不把合成JSON回环外推为当前平台、实时公共SSE或完整产品验收。

7个终态阶段各5213项选定输入before/after与最终源码逐项一致。与前批373项FreeBuff完整测试名对账，无旧身份丢失，新增恰好20，两个旧browser-host ignored不变；未重复无改动的独立LocalRuntime/pipeline_lifecycle全选择器，不挪用其旧通过数。现有warning原样保留，没有无关cargo fix。

官方lexer按当前直接preimage测量：root builder97→96、step91→96、原root测试244→251、claim测试index218→220；5份新增Rust为history138、merge64、pairing tests171、merge tests106、wire tests121有效行。最大251的原root测试仍只承担默认root组装验证，处于251–500单职责允许范围；没有baseline/exception修改或新增超限文件。

9份Rust＋7份文档/修改说明共16份范围文本按直接preimage检查UTF-8无BOM、范围diff和Git空白；13份固定原文、原LICENSE/规范化NOTICE、公共guard/baseline/exceptions与原报告bytes纳入终态审计。根checker report按本批直接preimage恢复，scripts report保持不变。源码与测试审查覆盖实际输入边界、secret/权限、分配/复杂度、无后台任务/进程及原run取消/费用/资源所有权。

证据根：[gateway-freebuff-root-history-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-root-history-20261107)。以`audit.json`、`test-identities.json`、`final-lines.json`、各阶段receipt/log/source快照、`reference-provenance.json`和`scoped-diff.patch`为准。测试peer await close，SQLite测试数据只在本批linshi/TEMP；终态进程清点另见`final-process-inventory.json`。

## 5. 仍未完成与接力范围

公共 `freebuff_business_admission_required` guard 保持原 bytes；**模板仍累计新增 17 个，无新增整卡 `offline_ready`，P50/P60/P70 与总目标未完成。** 本子链不能替代公共 producer、正常 UI 逐次价格/余额确认、三公共协议完整执行、public closeout/probe/全部阶段取消、durable run/远端 lease、Server/跨进程/原生包及真实平台验收。

下一步仍需真实上下文计量/摘要及其持久 history owner、实际工具/child loop 和费用回调合同，再接完整公共消费者；不应重复添加 descriptor、误用 compiled 身份改变 opaque wire 行为，或把可发送的合成工具历史当完整 harness。无真实凭据/供应商、Redis/Docker、现有实例或数据库、出包、提交/推送操作，不复用已消费的 P34 一次性容器授权。
