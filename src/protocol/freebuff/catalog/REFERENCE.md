# 当前 catalog/device 协议来源

2026-11-08[前台自动摘要/canonical history子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-summary.md)继续复核同一固定archive的13份原文，主要消费conversation-summary/compaction/run-agent-step与common compaction-summary/policy。结构化指令、提取/wrapper和reported threshold已实质适配Rust；原Apache-2.0 LICENSE/NOTICE及root_builder修改说明继续适用，P50仍需随包分发。独占in-memory history保留当前用户，summary沿原run签名/费用/取消/FINISH，不冒充持久history/background/idle/manual、实际工具/child或公共业务；终态证据和native差异见回执，未重新联网或执行第三方实现。

2026-11-08[root context子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-context.md)静态复核同一既有固定归档的token-counter/context-token-count及测试、run-agent-step、conversation-summary、compaction-policy等29份原文。实际消费者明确禁止BPE，采用provider usage anchor＋cheap delta；旧model-config注释不作真实tokenizer合同。Rust native适配/原run预算gate已接，17新增/相邻410项、两生产lib与静态门禁通过；自动摘要/持久history、实际工具/child与公共producer仍缺。不重新联网或执行第三方实现，逐项来源/原许可、范围审计和适配差异见回执。

2026-11-06 通过 GitHub connector 联网核实官方公开仓库，固定 commit
`CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`。
同 commit LICENSE 为 Apache-2.0，blob `446a604f1bd40e5d75149f9300fe294400d11f3f`，沿前批已经核验的固定证据；本批没有复制 SDK/React controller，也没有下载运行参考代码，仅参考 wire/type 独立实现 Rust owner。

| 参考 | blob SHA | 采用内容 |
| --- | --- | --- |
| [hosts.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/constants/hosts.ts) | `849ea1f7834c1c327e055ed2a9293303eb0119f1` | FreeBuff browser sign-in/marketing origin，不据此推断业务 host |
| [smoke README](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/freebuff/e2e/README.md#L88-L92) | `27523c01ddef2638c74aec1c9688276c7045340c` | 明确的 `www.codebuff.com` 业务与 `freebuff.com` 登录分 host 构建示例；不是正式 binary receipt |
| [CLI session API](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-api.ts#L133-L141) | `ae62085e3f3bc29ab8914caeda9438f29e98dd91` | 配置业务 origin，公开 fallback 为 `https://codebuff.com` |
| [catalog types](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/types/freebuff-model-catalog.ts) | `9ad6d8bb6cb8e05eb334524e43306779e5564119` | 固定 path/header、签名 payload、core row、issuedAt/refreshAt、optional fetchId |
| [device signing](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/util/freebuff-device-signing.ts) | `32d37a38d403f4ff4dce6fdcb27ffff1ed3ced0b` | Ed25519、公钥注册、host/account scope；不照搬吞错/后台注册/写队列 |
| [CLI key owner](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-device-key.ts) | `41c3193284a95a51bb98c66956638cb75c96ed29` | 安装 key 与登录 fingerprint 分离，catalog GET 不带 fetchId；本模块不另建文件存储 |
| [CLI catalog](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-model-catalog.ts) | `43e191aa959cba071dd5611ce899b07752a49c1c` | Bearer/protocol/client、有效 catalog 后准入注册，临时错误保留 snapshot |

## 本地约束和未实现边界

`BusinessOrigin` 只允许调用者显式选两个已证业务 host，不能从 typed 登录目标自动转换；CLI/Desktop 显式选取并参与本地 scope，不伪造版本号。纯 parser 的任意输入不能注入 `CatalogSession`，有效网络 fetch 后才准许注册。

按本地信任边界增加有界输入、重复 key/handle 拒绝、无 redirect/retry/default secret headers 和单 deadline；无效/永久响应明确失败，无静态 fallback/default agent。fetchId 缺省或空串兼容；server issuedAt 控制未来 row，refreshAt 不冒充 expiry。

catalog GET和公钥registration POST之外，新增独立[signed business wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md)：显式session inspect、CLI单次admission/heartbeat/精确attempt end，以及最终completion bytes签名发送。每个请求固定path、同snapshot handle/fetchId、无redirect/retry/unsigned fallback；这些是真实回环传输测试，不是正常管理/公共调用入口已放行。未解释 optional agent compaction/effort/placement 等行为，不将这些元数据提升为已支持能力。

协议 owner 的 key 仍由调用者显式导入，不在协议模块生成/覆盖 key 或新增磁盘 store。后续 LocalRuntime 复用原 seal/SQLite 实现安装 key 与精确 registration 持久 owner，经受限版本/scope/public/keyId receipt 恢复 capability；PKCS#8 DER v1/v2 核对 raw32 公钥，不是官方 device record schema v2。无 Serialize/Debug 材料输出不等于内存 zeroization。

正常管理 origin/client 选择与只读目录已接；本地持久 owner、显式注册 API 与后续 signed catalog 另见 [设备子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-device-runtime.md)，正常注册/未知重投/重新注册 UI 见 [状态与 UI 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-device-ui.md)。稳定目录选择/显式业务目标已接正常账号保存与 Grant 门禁，不改变登录受众；范围与验证见[业务目标子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-business-target.md)。signed transport不是消费确认或active admission capability；持久pending/unknown claim/恢复、费用确认/当前价格余额重验、agent/run/lease/probe/公共推理、Server/跨进程/原生包仍未闭合。dedicated reuse只有公开常量/语义证据，不猜其method/body/完整headers。不能以目录200、transport2xx或可路由声明推断模型可调用、免费或拥有权益。

本轮再次下载同 commit device signing 及其 [客户端测试](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/util/__tests__/freebuff-device-signing.test.ts)，后者 blob `0e74d3647dce589d42725b88a128ef657da21def`；fake fetch 的重复注册可返回不同 keyId。没有核到公开服务端 uniqueness/幂等合同，因此本地 unknown marker 不自动重 POST，显式重投必须消费当前精确 attempt ID，不宣称幂等。

后续[run/context 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-run-context.md)补齐同 commit `sdk/src/impl/database.ts`、CLI catalog root/session identity/cost mode 与 SDK llm 原文：START/FINISH 不走 completion 的签名 hook，START 使用实际 root ID 并返回 runId；FINISH credits 为累计参数而非 transport 常量。内部 active owner、一次 START、host metadata 和 signed completion/body 借用已有子链；完整 root/compaction、费用确认、decoder/FINISH、持久 run、正常公共入口与 Server 等仍缺。原文 URL/UTC/SHA-256 见该批 `sources.json`，不把固定客户端当当前服务端或真实计费证明。

后续[冻结报价子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-purchase-quote.md)核对同 commit 的 `cli/src/hooks/use-freebuff-chat-admission.ts`、`common/src/util/freebuff-model-selection.ts`、`freebuff-session-pools.ts`、`freebuff-price-changes.ts` 既有存档，4 份 SHA-256 与原 `source-references.json` 一致。仅参考“确认后需重验费用/不能把 wallet=session 当金额 ceiling”的语义及公开投影算法，独立实现不可变完整 meter/60 秒本地 quote 与持久 owner 组合；不复制 React controller，不采用 absent/unpriced 的 legacy 免费兜底，不声称服务端 CAS、正常 UI/公共入口或真实费用已验。与官方 intent 比较的差异和本地未发送 claim 保留代价见子批回执。

## root 模型输入的后续承接

2026-11-07[同snapshot root模型输入子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-model.md)复核同commit既有归档中的catalog row schema、compaction-policy、SUPPORTED_FREEBUFF_MODELS及ID依赖、base3、CLI catalog builder、compiled lookup与Apache-2.0 LICENSE；11份原文SHA、21项静态符号解析和独立digest向量见该批证据根。本批未重新联网，不声称当前线上已验，未执行第三方实现。

本仓独立读取optional compaction/legacyDigests，把同一row的handle/displayName/policy/compiled身份传到内部BusinessRequests。模型提示按row digest顺序第一个已识别身份判断，不从opaque handle/UI名猜测；固定21项不是当前授权模型列表，不改变wire model、路由、认证或额度。cache/token数值与metadata额外界限是本地策略；缺policy仍缺、坏policy拒绝，未知digest保持未知。仅补第23行当时未承接的模型输入，不证明runtime compaction、root prompt/工具/child、公共消费或完整harness已完成；公共guard不变。

## 默认 CLI root 文本的实质移植

2026-11-07[root builder 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-builder.md)进一步复核同一既有归档的完整base3/CLI helper/runtime renderer/SDK provider与原LICENSE/NOTICE，静态重建默认/batch/lean文本向量，没有重新联网或执行第三方实现。这一批与前面的独立wire实现不同，**实质移植了提示文本**；Apache-2.0原[LICENSE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/root_builder/LICENSE)、[NOTICE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/root_builder/NOTICE)、来源与修改说明已随源码保留，不声明immutable资产豁免。P50出包还须验证这些notices随binary分发；本批没有交付包。

默认完整system/两类提醒和受信host原生数据已实际进入原CatalogRun的有界组装/签名发送，不只保留descriptor或opening；可信host工具执行、真实context计量/摘要与完整loop仍缺，提示文字不构成其验收。工具集合检查为本默认CLI host契约，不断言上游强制所有工具，不由compiled身份偷换wire handle或provider pin，不改变公共guard/认证/额度。固定原文逐项SHA和许可一致性、13新增/相邻373项通过（含13）及未验证边界见回执和证据根。

## root native history 的后续准备

2026-11-07[history子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-history.md)静态复核同一既有归档的common util/messages及测试、runtime run-agent-step/util/messages、native chat converter、model-config和原LICENSE/NOTICE等13份原文，URL/SHA见该批reference-provenance.json；没有重新联网或执行第三方TS。既有Apache-2.0许可/NOTICE不变，root_builder README补本次适配说明，P50仍需随binary分发验收。

采用未回答call在真实user边界/EOF清理、保留freestanding结果、请求准备先于生成提醒，以及连续assistant合并/metadata shallow later-wins语义。输入已是native Value，故不把providerExecuted当internal capability，不重做internal→wire reasoning抑制或猜signature来源；原生parts保留，非空混形content明确拒绝，metadata-only空消息保留政策等差异以回执为准。raw snapshot有界计数、owned合并进入原run最终签名发送，不是持久history修复或完整web sanitizer。

另核supportsAssistantPrefill实际消费agentTemplate.model，opaque handle不偷用compiled身份加continuation；contextPrunerBudgetForModel确有400000固定runtime默认，不能把row缺maxContextLength说成来源没有默认，也不把它当已验模型最大context。真实tokenizer/预算/摘要、工具/child、公共producer/逐次UI与durable run/lease等仍缺；公共guard不变，不新增模板或整卡offline_ready。真实红测/夹具失败、最终验证与源码审计见新回执。
