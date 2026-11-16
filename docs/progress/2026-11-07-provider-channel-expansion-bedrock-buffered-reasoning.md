# Bedrock：有序 reasoning 非流式响应与公共 JSON

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
继承 `main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981` 的未提交实现；HEAD 不是本批源码交付凭据。
本批范围是原生 JSON 解码、真实 AWS EventStream 的**非流式聚合**、三公共 JSON 及原生 Converse 内容输出，不是实时流式 reasoning、完整签名多轮或 W06 完成。

## 改了哪些内容

1. 新增独立 [bedrock_turn.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_turn.rs) 和 [wire.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_turn/wire.rs)。canonical 的可选 `bedrock_turn`、公共 JSON 的 `gateway_bedrock_turn` 保存原生有序 `content`；reasoning 不混入普通答案文本，不冒充 Anthropic thinking/OpenAI reasoning，也不借用 `vertex_turn`。Debug 不显示 reasoning/signature，错误不带原内容。
2. 原生 JSON 保留 reasoningText、optional signature 和完整 redactedContent 的原字符串；合法空 text/signature/bytes 不丢失。两个 union、类型、未知字段、工具身份/重复 ID、Base64 和总序列化字节均校验。与已有流式状态机对齐：JSON 的 `tool_use` stop 必须存在工具调用。
3. [Snapshot](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/upstream/turn.rs) 按 AWS block index 收集有序内容。signature/text 线性追加；redacted 每片先解码 bytes，最后一次 Base64 编码，不直接拼接带 padding 的 Base64 字符串。空工具参数沿原 stop owner 规范为 `{}`；工具的 dense 公共 index 不与 AWS block index 混用。
4. 普通非流式 Bedrock 在生产中实际请求 ConverseStream 并聚合，因此并未只做一个 JSON parser。完整 CRC/frame/messageStop/metadata/EOF 校验通过后，才从快照生成 canonical 响应；截断、错误、非法 block/union 或预算超限均不返回部分成功。
5. [三公共 packer](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/pack.rs) 输出显式 Gateway 扩展。Chat/Messages/Responses reader 均核对扩展与可见文本、工具数量/顺序/ID/name/input 一致；Chat 禁止对该原生投影做旧 HTML entity 修复或文本工具注入。并存的 Google/Bedrock turn 声明拒绝，不合并来源。
6. [原生 Converse route](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/bedrock.rs) 经正常 pipeline 后恢复原生有序 content，不再把 text/tool 平铺后遗漏 reasoning。[路由兼容表](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/protocol_resolution/request_compatibility.rs) 补上现有 native planner 已支持的 Messages/Responses；legacy Completions 仍不开放。
7. 新字段缺省 `None`，旧 canonical JSON 可读且不序列化空字段。其他响应构造点仅补必要的 `None`；S06/Gemini 等实现没有借此重构。没有升级存储版本、修改凭据或合并渠道额度。

## 预算与生命周期

- 有序快照累计保留原始字符串/bytes 的预算为 4 MiB，并以有界 writer 核对最终 JSON（含 escaping、结构和 Base64）的 4 MiB 限制；4096 个 block 的原上限继续生效。不是所有请求体或进程 RSS 的统一 4 MiB 限制。
- 没有 reasoning 时，可选快照超过预算会丢弃自身，原来的较大 text/tool-only body 支持继续；若之后才出现 reasoning，明确失败，不能复活一个被截断的快照。不会把前缀丢失伪装为完整内容。
- 快照跟随调用栈所有权，失败/取消时直接释放；不增加后台任务、队列、线程、进程、重试或持久对象。新增收集为随字节线性追加及有界 block map 查找，最终序列化不反复扫描增长前缀。
- 实时 stream 仍在首次 reasoning update 明确报错，并在交付错误前 Drop 原 source；不输出成功 finish/usage/DONE。不以本批非流式 carrier 冒充已经保真的原生实时 SSE。

## 红绿、运行证据与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-reasoning-response-20261107`。
官方依据沿用紧接前批固定的五份 AWS API/userguide 原文，见[请求历史回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-signed-history.md)及其证据根 `sources/receipt.json`；本批未新增外部文档结论或联网生成。

- `red-buffered-v1` 是真实行为红测：2 passed / 5 failed，复现 JSON/聚合 reasoning 拒绝、公共 reader 丢状态和非法扩展未拒绝；循环内只声称实际走到的首个失败，不虚算所有分支均红。
- `green-buffered-v1` 的初始七项通过后继续扩大验证。`buffered-final-v1` 为16 passed、公共路由1 failed，不能作为整批绿证据；新增诊断后的 `red-route-v2` 精确复现 `/v1/messages` 被兼容表排除，calls=1 是此前 Chat 的一次成功，不是 Messages 已发送。
- `red-validation-v2` 为16 passed / 1 failed，复现 JSON 的 tool stop 无调用仍被接纳；Cargo 在该 harness 失败后没有运行后续 route harness。optional-signature 夹具的 stopReason 同步修为真实的 end_turn，而非继续以错误 tool_use 充当合法样本。
- `adjacent-final-v1` 在既有 EventStream adversarial 测试失败：旧夹具把现已支持的合法 reasoning delta 当作 unsupported。仅把该变体从 position 1 移到 position 4，改测复用已 closed 的 text block，保留原拒绝与脱敏断言；没有为了旧预期恢复拒绝合法 reasoning 的生产行为。修正后十二组相邻合同全部通过，旧失败日志保留。
- 首次收尾 audit 被 Git 对继承 CRLF 文件的 `CRLF will be replaced by LF` 提示中断；没有 whitespace 诊断，尚未执行共享报告恢复。审计命令局部指定 `-c core.safecrlf=false` 后重验，仍要求逐文件无 stdout/stderr 及合法退出码，不修改全局/仓库 Git 配置或为测试改写源码行尾，记录见 `audit-first-attempt.json`。

最终验证如下；中间红绿和重复运行不累计：

| 检查 | 最终结果 | 回执 |
| --- | --- | --- |
| 新增响应与正常公共路由合同 | 18 passed / 0 failed / 0 ignored | `buffered-final-v2.json` |
| 十二组相邻 Bedrock 合同 | 84 passed / 0 failed / 0 ignored | `adjacent-final-v2.json` |
| 库 `bedrock` / `vertex_turn` / `canonical` / `unpack_` 名称筛选 | 分别 30 / 12 / 65 / 27 passed；四组有两项重合，去重为132项 | `library-*-v1.json` |
| 本批相关 Rust 最终完整身份去重 | **234 passed / 0 failed / 0 ignored**；按 harness + 完整测试名去重 | `test-identities.json`、`audit.json` |
| 正常运行栈生产 lib | `--locked --offline --no-default-features --features runtime-defaults -j 4 --lib`，exit 0 | `production-final-v1.json` |
| 代表性关闭 Bedrock 的生产 lib | 原运行栈去掉 Bedrock feature，exit 0；不是完整 feature 矩阵 | `disabled-final-v1.json` |
| Rustfmt | 本批45项 Rust 文件，exit 0 | `rustfmt-final-v2.json` |
| 官方有效行 checker 测试 | 31/31，消费的 scripts 输入与最终一致，未重复运行 | `checker-final-v1.json` |
| 有效行 ratchet | 4802 files、14 immutable runtime artifacts；governed source >700=0，未改 baseline/exception | `ratchet-final-v3.json` |

十个终态阶段均在各自执行期间无所选输入漂移。除新增18项的最终阶段外，其余九阶段均匹配当前5115项输入；新增18项之后只改了上述不被两个新 harness 导入的相邻 adversarial 夹具，其生产/自身测试输入未变，不为全局快照整齐重复运行。checker 另按实际消费的 scripts 子集核对，不把未消费的 Rust 变化说成 checker 输入未变。

`audit.json`、`effective-lines-final.json` 与 `scoped-diff.patch` 记录45项 Rust（8项新增）、5份文档的范围、UTF-8 无 BOM、本地链接、直接 preimage、before/after 官方有效行及逐文件/整仓 diff check；53个直接 preimage 原件保留。本批行数报告归档为 `ratchet-report-v3.json` 后，根共享报告按本批直接 preimage 原字节恢复，恢复核对见 `shared-report.json`，不从 HEAD 或其他子批备份重建。上述定向检查仍有编译 warnings，不声称全仓绿、零 warnings、独立进程或整包验收。

测试 owner：
[响应合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_reasoning_response_contract.rs)、
[异常/预算/释放](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_reasoning_response_contract/adversarial.rs)、
[合成 AWS 夹具](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_reasoning_response_contract/fixtures.rs)、
[公共 router 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_reasoning_route_contract.rs)。
随机 loopback 上游复用 legacy-auth fixture，不放宽显式 IAM/Bearer 的 AWS 目的地 allowlist。测试经正常公开 auth/route/send/pack，检查三公共 JSON 与原生 Converse 的 content、实体字面量、工具、usage、一次健康成功/调用及 active 释放；并核 legacy Completions 不调用上游。公共 router 使用 in-process Service，真实 TCP 在合成上游一侧；不是新公共监听进程、SQLite/PG 结算或 AWS 账号验收。

## 未完成与接力

1. 实时 reasoning JSON delta→normalized SSE→Chat/Messages/Responses/原生 EventStream 的有序转发和 terminal witness 尚未实现。不能只在最后加 turn，而继续让 native encoder 重排/漏 reasoning；当前显式失败保护必须保留到真实替代链完成。
2. `gateway_bedrock_turn` 是**响应内容状态**，没有模型/产品/请求前缀绑定，也不是授权、凭据、签名验证或可重放的历史 token。本批没有新增公共历史字段；跨协议下一轮必须另处理 system/prefix/工具结果和策略变更，不能把扩展任意 flatten 后声称签名多轮成立。
3. 完整参数/模态、modeled exception frame、Bedrock 专属完整取消 audit、真实公共 TCP/独立进程/原生包、AWS 权限/模型开通/额度仍未证明。普通 text/tool 流已有的离线合同不等于完整 Bedrock 能力。
4. P50 全范围无凭据集成与 devtest、P60 集中录制、P70 真实逐渠道验收继续。累计新增模板仍17，没有新增整卡 `offline_ready`。

没有读取实际 `.ng`/凭据、调用真实供应商、修改运行实例、创建容器、出包、提交或推送；P34 的历史单容器授权不复用，也不要求现在录制凭据。
