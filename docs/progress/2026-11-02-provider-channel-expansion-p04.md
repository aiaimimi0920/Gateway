# 分渠道扩展：Bedrock AWS EventStream 入站（第 4 批）

> 2026-11-02；main，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交或推送。
> 承接[第 3 批认证修补](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p03.md)，继续执行[总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)；不是 W06/P50 或真实 AWS 验收完成。

## 1. 实际实现与边界

新增 `protocol/bedrock_converse/upstream` owner，不再让 Bedrock buffered/streaming 入站依赖 Accio 的 SSE/wrapper parser。完整 Converse JSON 也由 Bedrock 自己解析。

- 身份从 `:message-type / :event-type / :exception-type` 二进制头读取，JSON payload 是事件本身，不套 SDK union wrapper。
- prelude CRC 在信任长度前验证；message CRC 在解释 header/JSON 前验证。EOF 半帧、结构长度矛盾、类型、重复/矛盾头、非法 UTF-8/JSON 均拒绝。
- 不套用 Smithy 明确禁止客户端验证的 service-only 24 MiB payload / 128 KiB headers 规则。使用 Gateway 自身既有 64 MiB 单帧字节预算和 4096 header 数量预算约束解码 map；不把本地资源预算冒称协议合法性要求。
- header 十种类型有界解析；`:content-type` 可缺省，存在时必须是 string/json。异常和 unmodeled error 终止，诊断不回显原始消息、未知 event/stop 字符串或业务内容。
- messageStart、block identity、工具增量/完整 JSON、blockStop、messageStop、metadata 有明确状态；文本不强求 tool-only 的 contentBlockStart。
- 正常成功要求 messageStop、其后 usage metadata、完整 HTTP EOF；不在 stop 提前丢 usage，不因未知/malformed stop reason 伪造正常结束。metadata 缺失视为不完整，不造零计量。
- AWS 内容块索引映射到稠密的 OpenAI 工具序号；文本不占工具 index；工具身份只发一次，参数分片不重放。无参数工具与 buffered 的 `{}` 规范化保持一致。
- 单流 ≤4096 blocks，活跃工具参数保留 ≤4 MiB；buffered 复用既有 64 MiB body owner。stream 惰性逐事件投影，不创建后台任务；取消/drop/终止错误释放整个源链。
- `stage_send/stream_preflight` 增加准确的 Bedrock translator 分支；后续既有 SSE usage、协议投影和结算链继续使用，不重起第二套 pipeline。

### 明确未完成

本切片支持 text/tool/usage。真实 reasoningContent、signature、redactedContent 及其他新 content union 的端到端保真尚未完成；当前拒绝而不是丢弃后报成功。canonical response、协议投影和后续请求签名内容必须一起设计，不能只把 thinking 改名为正文。
原 OpenAI→Bedrock 出站 encoder 仍使用无 header 的 wrapper payload；其现有 CRC/预算测试不证明原生 ConverseStream 客户端线格式正确。本批只修入站，出站真实 framing/lifecycle 继续作为 W06 必做项。
Console 多字段模式/SecretPatch 回环、目录策略、精确账号测试、真实 IAM 权限/额度、打包及集中验收仍未完成，不把 W06 标 offline_ready。

## 2. 红测、回归和证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p04`。

| 检查 | 当前结果 |
| --- | --- |
| 原实现红测 | 5 tests，0 passed / 5 failed；真实头被解析为空成功、CRC/异常/截断未拒绝；bedrock-upstream-red.log |
| 首次扩展编译 | 测试 headers 数字类型推导报 E0689，修为显式 u32；bedrock-upstream-green-initial.log；不算测试通过 |
| 独立审查红测 | 17 tests，15 passed / 2 failed，复现 AWS block index 被当 OpenAI tool index；bedrock-upstream-review-red.log |
| 官方 client/service 约束红测 | 2 tests 均失败，复现错误套用 service-only payload/header 大小；bedrock-service-limits-red.log。已改为独立 Gateway 资源预算，最终回归重新冻结源码 |
| 最终入站/既有公开合同 | 20 项入站 + 5 项 Converse + 13 项既有出站合同全部通过；bedrock-final-v2.log；旧出站合同不证明 native framing |
| 实际 stage_send 回环 | 3 passed；bedrock-final-v2.log；真实随机 loopback HTTP，含 buffered、stream 与失败/drop 结算，不是内存 Response 或直接 parser 调用 |
| 相邻回归 | lib bedrock_ 29 passed、protocol::stream_error 4 passed、call_flow 40 passed；regression-*.log；非全仓回归 |
| feature 隔离 | 不启用 Accio 的精简 feature 组合：20 入站 tests passed，bedrock-without-accio-v2.log；关闭 Bedrock family 的 lib cargo check exit 0，bedrock-feature-disabled-v2.log；均保留 Gemini 既有必要 feature |
| 格式/checker/ratchet/编码/diff | cargo fmt check、31 项 checker tests、ratchet、UTF-8 无 BOM、65 个本地引用、scoped whitespace 与 git diff --check 均通过；源码冻结 v2 未漂移 |

合成 fixture 使用有效 CRC 和真实 header-tagged payload；覆盖逐字节碎片、普通与不连续工具索引、UTF-8、post-stop cache/usage、长度/header/JSON/顺序/工具错误、两种 CRC、限额、异常、取消、终止前 drop 和错误前释放源。
新 pipeline 合同使用随机 loopback listener、合成凭据和真实 stage_send；临时目录必须通过测试进程 TEMP/TMP/TMPDIR 指向本证据目录，不能清理用户默认 temp 或在线 .ng。
前批 Console 整套已知的 `legacy_compiler_rejects_extreme_token_expiry_without_panicking` 未在本批处理，不声明全仓测试绿灯。

关键复现命令（仓库根；各 Cargo 命令串行执行）：

```powershell
$e = 'C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p04'
$env:TEMP = "$e/runtime-temp"
$env:TMP = $env:TEMP
$env:TMPDIR = $env:TEMP
rtk proxy cargo test --locked --no-default-features --features runtime-defaults --jobs 4 --test bedrock_upstream_eventstream_contract --test bedrock_pipeline_eventstream_contract --test bedrock_converse_contract --test bedrock_eventstream_contract
rtk proxy cargo test --locked --lib --no-default-features --features runtime-defaults --jobs 4 bedrock_
rtk proxy cargo test --locked --lib --no-default-features --features runtime-defaults --jobs 4 protocol::stream_error
rtk proxy cargo test --locked --lib --no-default-features --features runtime-defaults --jobs 4 call_flow
rtk proxy cargo test --locked --no-default-features --features line-gemini-web-reverse,line-aws-bedrock-converse-official-model-api --jobs 4 --test bedrock_upstream_eventstream_contract
rtk proxy cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4
```

后两条保留 line-gemini-web-reverse 是为了绕过既有全关 feature 组合的缺符号问题，不涉及 Gemini 执行器修改；这不等于宣称任意 feature 组合均能编译。

## 3. 有效行、信任边界与独立审查

既有 owner 有效行前后：bedrock_converse.rs 93→99、bedrock_converse_official_api_common.rs 155→154、stream_preflight.rs 262→266、stream_error.rs 69→69。
新增 production owner：upstream.rs 24、frame.rs 72、headers.rs 111、response.rs 149、state.rs 238、stream.rs 95；新增测试/fixture 分别为主合同 115、fixtures 74、adversarial 211、stream 217、limits 117、pipeline 216。精确路径和数值保存在 effective-after.json，最大新增源文件 238，有职责内聚的既有 preflight 266；没有超过 500 的新增/修改源码。
新增职责分成 framing、headers、状态、stream、buffered/JSON response；测试按 fixture、adversarial、资源、流生命周期、pipeline 分开。没有修改依赖、checker/baseline/例外来绕门禁。
只读独立审查发现工具索引归属问题，已增加红测并修补；确认 reasoning/signature 是真实支持缺口，不拿安全拒绝代替完整功能。审查不是执行结果，最终以冻结源码对应日志为准。
进一步核实官方原文 `Services MUST validate these additional size restrictions. Clients MUST NOT validate them.` 后，补两项红测纠正 service-only 大小约束。初次 source-hashes-frozen.json 保留为修补前证据；最终来源改由 source-hashes-frozen-v2.json 绑定，不冒用初次绿测代表最终版本。
最终 checker 日志为 checker-tests-final-v2.log、ratchet-final-v2.log；ratchet 扫描 3906 files，14 个精确哈希绑定 immutable runtime artifacts，governed source >700=0。使用官方 checker 的同一 ratchet 模式、不写报告入口，保留本轮开始时已脏的 scripts/artifacts/effective-code-lines.json；没有重生 baseline 或增加例外。

## 4. 官方协议依据

- [Smithy AWS EventStream](https://smithy.io/2.0/aws/amazon-eventstream.html)：两种 CRC、长度及 typed header、event/exception/error。
- [Converse 推理顺序](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html)：文本不要求 blockStart，messageStop 后 metadata。
- 官方 Rust SDK 固定 `44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7`：[Event deserializer](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/bedrockruntime/src/event_stream_serde.rs#L79-L215)、[Header reader](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/aws-smithy-eventstream/src/smithy.rs#L89-L118)、[stopReason](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/bedrockruntime/src/types/_stop_reason.rs#L74-L86)。仅行为参考，没有复制/安装/执行 SDK。
- [ContentBlockDelta](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlockDelta.html)、[Reasoning delta](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ReasoningContentBlockDelta.html)：reasoning、signature/redactedContent 不能默默丢弃。

## 5. 接力与运行边界

本批没有读取/试用历史凭据、实际 AWS 推理/生成或费用；没有更改在线实例、存量账号、数据库、旧包、S06/Gemini owner，没有构建 EXE/包、重启、提交或推送。
下一步补 Bedrock reasoning/signature 贯通与真实出站 framing、正常多字段材料/secret 链；Qwen 普通 API 按既有 native 分页/地域合同继续实施，OAuth/Web/媒体其余卡片继续按总计划追踪。
全目标保持 active；不能把本批输入解析切片当作全部 16 项开发或集中验收完成。

### 下一最小切片：原生出站 wire 与生命周期（尚未实施）

只读审查入口为 src/protocol/bedrock_converse/eventstream.rs、tests/bedrock_eventstream_contract.rs 和 src/http/routes/bedrock.rs。不能只给现 encoder 加三个 headers 就宣称修复完成：

- emit_event 显式区分 event type 与未包装 JSON；编码三个 string headers 并将它们纳入长度/CRC，messageStart 正确输出 role=assistant，不以 model 代替。
- 原 encoder 未输出任何 contentBlockStop；须在 messageStop 前关闭实际打开的文本/工具块。文本固定 0 与 OpenAI tool index 0 会冲突，分配 Bedrock 自有索引映射。
- 工具身份未齐时不能先输出参数；开始前参数需有界等待，开始后的 id/name 变化不能默默替换。现有“开始后更名仍成功”的旧测试必须改为正确生命周期预期，而非守住错误 wire。
- finish_reason、独立尾部 usage、DONE、完整 EOF 分开处理；不能丢 finish 后 usage、用稀疏字段填零覆盖计量、重复输出 stop，或把缺终态 EOF 当成功。保留拉取式背压与错误/drop 即释放，不增后台 drain。
- 当前路由将 encoder Err 保留为 HTTP body error，尚无原生 AWS exception frame；如果新增异常投影，必须同时保护 typed 错误与内部结算归因，不能吞错误伪造成功。
- 独立 decoder 验证 headers/CRC/未包装 payload，补混合 text/tool、身份迟到/变化、尾部 usage、重复/截断/错误/drop 负测，并保留全部资源上限保护。仅 encoder 测试不能外推路由级结算或真实 SDK 互通。
