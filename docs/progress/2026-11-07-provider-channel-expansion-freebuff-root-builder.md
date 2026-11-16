# FreeBuff：默认 CLI root 文本、native 请求与原 run 发送接缝

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留继承 dirty edits，不提交或推送；HEAD 不是本批来源凭据。本批是内部请求组装子链，不是 R06 整卡、公共业务或总计划完成。

## 1. 本批真实改动

承接[同 snapshot 模型输入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-root-model.md)，新增 `root_builder`：同一 `BusinessRequests` 的原 handle/displayName/policy/compiled 提示 → 默认 `createBase3CliRoot` 的完整 system 文本 → native messages/tools/provider/stream → 原 `CatalogRun` 签名/发送/解码/费用 owner。不是只添加 canonical opening，也不是把 definition 字段放在未消费的 descriptor 里。

`RootHostContext` 只接受受信 host 已渲染的日期、项目知识、git 和系统信息；Gateway 不扫描调用者目录、不执行其中命令或解析插入文本内的占位符。静态模板单遍展开，host/model 文本原样插入，不递归替换。canonical opening 始终在第一个 system 内容的 byte 0，CLI appendix 追加在 core 后；显示模型名用同 row 的 displayName，不泄露 opaque handle 作为模型介绍。

Muse/Solar 的固定 `instructionsPrompt` 是每条真实 user message 后的独立 user 消息，不插在 tool result 后；Flash 的固定 batching section 与 `<system_reminder>` step reminder 配对，后者只在当前请求末尾。输入契约为 host 的新鲜、未含生成提醒的 bare native history snapshot，消费后不写回 host 历史；重建不会自行累积 step reminder。native 图片、assistant reasoning/tool_calls、tool result 与 schemas 按 owned Value 移交，不 flatten、不替换模型或覆盖 host 原生字段。

默认 CLI root 宣告的 15 项 tool name 必须由 host 提供唯一的 function/schema。缺项、重复、foreign name/type 或缺 parameters object 在本地拒绝。这是本实现的默认 CLI host 契约，**不是服务端要求所有 Gateway 请求必须有这 15 项工具的证明**；没有实现 noWeb/noAskUser/workspace 分支。提供 schemas 不等于 Gateway 已执行工具，也不授予本机文件、终端、网络、UI 或安装技能的权限。

请求顶层 `provider={data_collection:deny}` 沿固定 builder→SDK 的实际映射；不因 compiled Anthropic 身份给 opaque handle 误加 Bedrock pin。Codebuff provider 的固定配置 `includeUsage:undefined` 与 strict/BYOK 分支不同，本批不猜加后者的 stream_options。上下文字符串 aggregate 1 MiB、最多 4096 项原生历史是本地资源政策，不冒称上游上限；最终 body 仍由既有 `1..=50 MiB` 有界 serializer 冻结、原样签名并发送。

`execute_root_step` 与原 `execute_step` 共用 `execute_built_step`。预算在 root build 前开始；预取消不先验证/渲染，build/serialize 拒绝返还原 run 且不增加模型尝试。后续 send/decode/EOF、unknown 费用、显式 FINISH 均复用原 owner，不新建 START、claim、重试或平行生命周期。同步组装/签名不可抢占，不承诺硬实时或全请求/FINISH 总预算。

## 2. 仍未实现，不能由提示文本冒充

lean 且有 maxContextLength 才生成固定 `# Context budget` 文本，按来源的 IEEE-754 floor/round 算提示阈值；lean 缺 context 不猜上限，仍禁 batching。缺 policy 的旧 row 仍可组装默认 root；本批没有把 cache idle/floor 当 context token budget，也没有实现 runtime 默认/摘要机制。

**可信 host 仍必须在调用前真实完成上下文计量/摘要、原生 tool history 清理与可执行工具合同。** 本批只实现渲染和请求组装，lean 文本所描述的自动摘要不是 Gateway 已有能力；定向 lean 测试只核文本，不执行 lean generation。完整工具/child loop、摘要及 child 费用归属、assistant prefill/未完成 tool call 的实际 host 处理、持久历史与实际 context policy 消费仍需接线。不能把本内部 API 或合成 schemas 当完整 harness，不能从 Loom 搬实现或自动获得调用者本地权限。

公共 producer/正常 UI 逐次当前价格余额确认、三公共协议、公共 closeout/probe/完整取消、START/FINISH 未知与持久 run/远端 lease、Server/跨进程/原生包和真实平台仍缺。公共 `freebuff_business_admission_required` guard 保持原 bytes，模板仍 17、无新增整卡 offline_ready，P50/P60/P70 与总目标继续。

## 3. 来源与许可

未重新联网；逐项复核既有固定归档 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`，SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`。完整 base3、CLI/helpers、runtime strings/run-agent-step/system-prompt、SDK/provider、LICENSE/NOTICE 的原文 SHA 见证据根 `all-reference-provenance.json`；不声称当前线上服务端合同已验，公开归档仍无注释所指的服务端 root-instruction-contract。

本批实质移植固定提示文本，保留 Apache-2.0 [LICENSE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/root_builder/LICENSE)、[NOTICE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/root_builder/NOTICE) 与[修改说明](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/root_builder/README.md)。Rust 文本正常计入有效行，不声明 immutable/generated 排除；其余 Gateway 许可证不变。source redistribution 已有原许可/notice；本批不分发 binary，**P50 出包还必须把这些 notices 随包交付并验证**，不能拿生产 lib 编译当包许可验收。只静态提取/重建公开字面量，没有执行第三方 TS、SDK 或下载依赖。

21份固定原文逐项与归档 bytes/SHA 一致；LICENSE原样保留。上游NOTICE原文末尾是一个ASCII空格且无EOF换行，本仓仅移除该空格并补最终LF，copyright/attribution文字不变；原bytes仍在临时参考证据，修改说明与审计按这个精确转换核对，不伪称NOTICE byte-exact。

## 4. 终态验证与范围审计

初始透传接缝 `red-v1` 实际 **0 passed / 4 failed**，分别缺 system、两类提醒和工具能力检查；不是编译负测，也不是声称这 4 项是旧公共业务的回归。完整 system 测试的 fixture 名字按原 row 修正 `Open→open`，没有降低断言。首次修复后 `focused-v1` **13/13**，无失败重跑；default/batch/lean 三种完整文本的 bytes/SHA 向量由固定 TS 字面量静态重建，非从 Rust 输出取 hash。

| 实际终态检查 | 结果 |
| --- | --- |
| `cargo test --locked --offline --lib freebuff_root_builder_ -j 4 -- --test-threads=1` | 13 passed / 0 failed / 0 ignored；含 4 项真实 SQLite＋回环 TCP |
| 相邻 `--lib freebuff` | **373 passed / 0 failed / 2 ignored**；已包含上述 13 项，不相加 |
| 默认生产 lib、仅移除 FreeBuff 的代表性关线生产 lib | 两项 locked/offline/jobs4 通过；关线不启用 management-ui，未运行关线单测或完整 feature matrix |
| 10 份 Rust `rustfmt --check` | 通过 |
| effective-lines checker tests | 31 passed / 0 failed |
| effective-lines ratchet | 4894 文件通过，governed source >700 为 0；不冒称 strict 或全仓功能通过 |

4 项 TCP 覆盖两步 default CLI 组装→最终原 bytes 签名→同原 run/claim/fetchId→每步费用累计/显式 FINISH，以及缺工具、预取消、最终 serializer limit 三种零模型发送/原 owner 返还。两步的 SDK reported credits 为 39，原 sealed claim row 不变；零尝试 FINISH 的 0 仅指未发模型步骤，不代表购买免费或真实供应商扣额为 0。未执行新 root 的真实工具或 lean 自动摘要，也未把 JSON 回环外推为真实平台/SSE/所有能力验收。

7 个终态阶段各 5208 项选定输入 before/after 与最终源码逐项一致。与前批 FreeBuff 测试完整名集合对账：原 360 项无丢失，新增恰好 13，两个旧 browser-host ignored 保持；未重复无改动的独立 local_runtime/pipeline_lifecycle 全选择器，不挪用其旧通过数。现有 warning 原样保留，不做无关 cargo fix。

官方 lexer 以本批直接 preimage 为准：catalog index 209→210，step owner 66→91，claim tests index 216→218；7 份新 Rust 分别为 builder97、prompt83、guidance29、CLI12、reminders2、unit tests244、wire tests134 有效行，最大244，无 baseline/exception 调整。10 Rust＋6 接力文档＋3 许可/说明共19份范围文本 UTF-8 无 BOM、直接 preimage/new diff、Git diff check 与固定参考/原许可一致性均审计；公共 guard/baseline/exceptions 原 bytes 保留，根 checker report 按直接 preimage 恢复，scripts report 未变。

收尾审计三次前置失败均保留：`audit-v1-failure.json` 为Git safecrlf提示使“stderr必须为空”断言失败，后仅在该审计命令加 `-c core.safecrlf=false`，不改持久Git配置；`audit-v2-failure.json` 为原NOTICE的真实trailing whitespace，未关掉空白检查，而按上述最小转换修正；`audit-v3-failure.json` 为核转换时误以为原NOTICE有EOF换行，hex实读后修正审计假设。没有改运行源码或掩盖Rust失败，因此不重跑已冻结且消费输入未变的Cargo/formatter/checker；19份文本最终范围检查实际重跑通过。

安全/资源审查：只依赖同 row 不可变选择与原 run；输入边界限额/工具身份检查，插入内容不再次作为模板或命令执行；有界元数据、owned 原生数据移动，无新磁盘/凭据/迁移、后台任务、队列、重试或进程。peer 正常 await close，SQLite 测试数据位于本批 linshi/TEMP。没有操作真实凭据、供应商、Redis/Docker、现有实例、出包、提交或推送，不复用已消费 P34 授权。

最终只读进程清点：本批verifier/Cargo/rustc/lib-test均0；没有停止其他项目进程。见 `final-process-inventory.json`。本批终态验证已结束，没有需要接回的exec session或后台本批测试。

证据根：[gateway-freebuff-root-builder-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-root-builder-20261107)。以 `audit.json`、`test-identities.json`、`final-lines.json`、各阶段 JSON/log、`all-reference-provenance.json`、`scoped-diff.patch` 与最终进程清点为准。
