# 服务商分渠道扩展：第 5 批，Bedrock 原生出站与结算归属

> 2026-11-02；W06/P10 的一个无凭据实现切片，**不代表 W06 或总计划完成**。
> 分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；包含此前未提交改动，未提交、推送、打包或重启实例。
> 本批流式 text/tool/usage 切片的定向验证已通过，结果见第 4 节。P04 历史快照不覆盖；全计划仍执行中。

## 1. 范围与渠道身份

本批接续 [第 4 批入站回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p04.md)，修复 OpenAI normalized SSE → Bedrock 原生 EventStream 的出站 wire、完整性及唯一结算 owner。
R10 仍追踪 `bedrock-api` / `bedrock-converse`；不修改存量 Provider ID，不创建账号或把 API Key、IAM、temporary IAM 当成三份独立额度。
最终额度仍按 AWS 主体、账户、region、模型/runtime quota 识别，未知关系不合并。

没有读取实际凭据，没有连接 AWS 或其他真实推理上游，没有生成付费内容；合成 HTTP 只使用随机 loopback 端口。
测试临时数据位于本批 `linshi/runtime-temp`，不会使用在线 `.ng`、路由文件或现有数据库。

## 2. 实现与安全边界

### 2.1 原生 wire 与有界投影

生产 owner 拆分为以下内聚模块，没有复制 SDK 文件或新增依赖：

- [driver](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/eventstream.rs)：惰性拉取、先排空当前输出再 poll 源；EOF 先释放源，再验证终态；错误交付前销毁源，不另起后台 drain。
- [frame](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/eventstream/frame.rs)：真实三 string headers、未包裹 JSON payload、prelude/message 双 CRC；输出队列最多 64 MiB 和 `2 × 4096 + 4` 个事件。
- [state](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/eventstream/state.rs)：`messageStart.role=assistant`，text/tool 独立 block index，各 block stop 后再 messageStop/metadata；finish、尾 usage、DONE、EOF 分开管理。
- [tools](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/eventstream/tools.rs)：身份不可变，迟到身份前的参数有界等待，start 后只补一次；最多 4096 tools，身份和参数合计最多 4 MiB；结束前验证完整 JSON，无参/空白参数补 `{}`。
- [usage](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/eventstream/usage.rs)：合并 sparse 字段，不用缺项填零；保留显式零、显式总数和 cache read/write；核心三个计数完整且均为非负 i32 范围才允许完成。

Gateway 在本方向是 **service encoder**：序列化后的单 payload 不超过 24 MiB，encoded headers 不超过 128 KiB。
大 text/tool input 在 UTF-8 字符边界分为多个完整 delta 事件，最坏六倍 JSON 转义仍受限，拼接内容不变；不是简单拒绝所有可分片的大文本。
P04 则是 client decoder，不能套用 Smithy 明确只要求 service 实施的这两个限制；两方向的边界不同，不互相覆盖。

工具 `name` 为 1–64 个 `[a-zA-Z0-9_-]` 字符，`toolUseId` 为 1–64 个 `[a-zA-Z0-9_.:-]` 字符，不自动改名、截断或伪造 ID。
可选字段 `null` 视为未提供，trim-empty 身份不占用身份位置；非 null 错类型仍拒绝。已声明身份不得被后续值替换。

### 2.2 EOF 与错误合同

本批 encoder 接收内部 **normalized OpenAI chat SSE**，支持 LF/CRLF 和碎片 UTF-8，要求完整空行分帧。
CR-only、未以空行完成的 data/event 尾帧不在本批兼容承诺内；完整 heartbeat/comment/id/retry 控制行不误当业务截断。
[严格 decoder](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/stream_decode.rs) 是加法式入口，其他 translator 的历史容忍/flush 行为不变。

finish 后仍允许尾 usage；完整 finish + usage + EOF 可不带 DONE。DONE 不意味着 transport EOF：必须等源正常结束后才发正常 messageStop/metadata。
缺 finish/usage、重复 finish/DONE、finish 后新增内容、截断、非法 UTF-8/JSON、上游 error、缺工具身份和不完整工具 JSON 均为 typed terminal error。
未知 finish、非零/多 choice、refusal/reasoning/audio 等未实现非 null 内容明确拒绝，不静默丢弃后报成功。
本批尚不生成 modeled AWS exception frame；失败通过 typed stream/body error 传播，不以 HTTP 200 或正常 EOF 冒充成功。

### 2.3 唯一结算 owner 与相同 usage 快照

[真实 streaming owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/streaming.rs) 的顺序为：

```text
normalized SSE → 既有 usage/completion/archive taps → SSE failure evidence
→ 按入站 Bedrock 协议编码/验证 → request budget → typed error evidence
→ 唯一 TrackedStream → 成功或失败 finalizer
```

编码选择依据是入站协议，而非上游 candidate 的认证/adapter；从 OpenAI 上游桥接到 Bedrock 入站也必须原生编码。
[HTTP route](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/bedrock.rs) 只交付已编码 body 和 Content-Type，不在已结算 stream 外二次编码。
取消/deadline 沿既有 stopped reason 归类，不惩罚 provider；permit 仍沿原来的交付前释放规则，没有偷偷改为持有到 EOF。

原生 encoder 完整验证并生成终态帧后，保存其 metadata 使用的同一个 `TokenUsage` 快照；只有外层成功 EOF callback 用它结算和写审计。
不再用另一个通用 SSE parser 推算 Bedrock 的最终计数。稀疏字段、显式零、显式总数和 cache 计数必须在 native metadata 与 SQLite audit 中一致。
未知/缺少核心计数不会合成零；显式 total 不因为本地重新相加而被替换。这不宣称上游计数本身已被独立计量核实。
已经看到 metadata 但未 poll 外层 EOF 就 drop，仍按现有中性取消处理，不触发成功结算；无 Bedrock-specific terminal shortcut。

最外层 byte/chunk metrics 现在观测下游 AWS wire；usage/completion/archive 仍在 SSE 层。
`metrics.latencyMs` 是从 `ctx.started_at` 到完整源 EOF 的 Gateway 实测 elapsed，不冒称 AWS 内部 inference metric。

## 3. 红测与定位记录

证据目录：[P05](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p05)。

- `native-outbound-red.log`：最初 4/4 失败，证实 headers/wrapper、缺 finish 和尾半帧错误。
- `native-pipeline-red.log`：最初 2/2 失败，证实真实 stage_send 未交付 binary 和原生验证位于成功 owner 之外。
- `native-limits-schema-red.log`：3/3 失败，证实 service payload 限制、optional null 和 tool identity schema 缺口。
- `native-usage-red-v3.log`：真实 stage_send + SQLite 的 3/3 失败，分别观察到 sparse usage 的 None/5、显式总数 5/9、显式零 2/0 分歧，再最小修补为同一快照。

usage 测试初稿漏设合成 authenticated session，导致 audit ID 为 None，panic 又触发 SQLite 未关闭的 fixture 清理 panic；Windows 以 `0xc0000409` 退出。
`native-usage-stack-diagnostic.log` 的原始栈确认这是测试配置/二次 panic，不据状态码误判生产 stack overflow。已补合成 session，在 count 断言前关闭 SQLite，保留原失败日志。
测试使用独立 16 MiB 线程和 current-thread Tokio runtime，与已有全 router 合同一致；没有修改生产线程或在线 runtime 配置。

## 4. 最终验证

源码快照：[source-hashes-frozen-v3.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p05/source-hashes-frozen-v3.json)。这是本批 21 个源码文件的 hash/有效行快照，不是 EXE 构建凭据，也不替代全工作树来源记录。
`native-final-v3.log` 已通过以下 73 项，无失败/忽略：

| 目标 | 通过数 |
| --- | ---: |
| bedrock_native_outbound_contract | 18 |
| bedrock_native_outbound_limits_contract | 3 |
| bedrock_native_pipeline_contract | 6 |
| bedrock_native_pipeline_usage_contract（真实 SQLite 审计） | 3 |
| bedrock_native_route_contract（真实完整 router/body） | 2 |
| bedrock_eventstream_contract | 13 |
| bedrock_converse_contract | 5 |
| bedrock_upstream_eventstream_contract | 20 |
| bedrock_pipeline_eventstream_contract | 3 |

测试覆盖大 UTF-8/JSON 转义文本无损分片、迟到工具身份/交错参数、无参工具、错误传播、严格 EOF、尾 usage、消费者取消、deadline/transport failure、metadata 后 drop 中性以及 HTTP 200 后 body 失败不记成功。
全 router 合同真实经过公开认证、路由和最终发送，只合成上游，不是 mock 掉 pipeline。仍不等于实际 AWS SDK/账号验收。

相关回归已通过，均使用 `cargo test --locked --no-default-features --features runtime-defaults --jobs 4`，没有运行全仓测试或付费上游矩阵：

| 目标/过滤器 | 实际通过数 | 日志 |
| --- | ---: | --- |
| `--lib bedrock_` | 30 | regression-bedrock_-v3.log |
| `--lib protocol::stream_decode` | 13 | regression-protocol-stream_decode-v3.log |
| `--lib stream_transport_tests` | 4 | regression-stream_transport_tests-v3.log |
| `--lib call_flow` | 40 | regression-call_flow-v3.log |
| sse_typed_pipeline_contract | 4 | regression-stream-observation-v3.log |
| stream_observation_contract / stream_observation_limits | 13 / 7 | regression-stream-observation-v3.log |
| pipeline_send_runtime_contract 的 `stream` 过滤器 | 20，另 40 filtered out | regression-pipeline-stream-v3.log |

过滤器间存在重叠，不把它们相加冒称互不重复测试数。`bedrock_` 包含新增的序列化后恰好 24 MiB / 超 1 字节 / JSON 转义膨胀边界单测。
73 项合同测试将第一个表中各名称分别作为 `--test` 参数；源码在冻结后未变更，复核使用本批 `validate-source.mjs`。

checker 自测 31/31，官方同一 checker 的无报告写入 `node scripts/effective-code-lines.mjs --mode ratchet` 通过（3920 文件、14 immutable runtime artifacts、governed source >700=0），日志 `checker-tests-final-v3.log` / `ratchet-final-v3.log`。
该入口避免覆盖原本已脏的 `scripts/artifacts/effective-code-lines.json`；未修改政策、baseline 或 exception。

feature 组合与最终检查均已完成：

- **不启用 Accio**：`cargo test --locked --no-default-features --features line-gemini-web-reverse,line-aws-bedrock-converse-official-model-api --jobs 4`，运行 native outbound 18、limits 3、upstream inbound 20，共 41 项通过；日志 `bedrock-without-accio-v3.log`。
- **Bedrock 关闭**：`cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` 通过；日志 `bedrock-feature-disabled-v3.log`。保留 Gemini feature 是沿用已知全关组合缺符号的工作区边界，没有接管 Gemini lane，也不声称全 feature 关闭已通过。
- `cargo fmt --all -- --check` 通过；日志 `fmt-final-v3.log`。
- 本批 21 个源码文件与冻结 hash 相符、严格 UTF-8 无 BOM；四份当前文档同样无 BOM、围栏完整，76 个去重本地链接目标存在。最终输出为 `source-check-final-v3.log` / `docs-check-final-v3.log`。
- `git diff --check` 与新增未跟踪文件的独立 `git diff --no-index --check` 通过，已检查本批 scoped diff；新增文件的正常差异 exit 1 与真正 whitespace 诊断分开判断。

编译仍有工作区既有 unused import/variable/dead code 等 warnings，未运行 `cargo fix`、升级依赖或借机改动其他 lane。本批检查不是全仓零警告、全量测试、正式发布或实际 AWS 运行证明。

有效行（仓库 lexer 实测）：

| 文件/范围 | 修改前 → 当前 |
| --- | --- |
| 原 eventstream driver | 338 → 87，职责拆入 frame/state/tools/usage |
| stream_decode | 343 → 381 |
| streaming | 429 → 446 |
| stream_diagnostics | 64 → 75 |
| Bedrock route | 107 → 103 |
| Bedrock facade | 99 → 100，本批只增加内部编码接线；P03/P04 改动保留 |
| 既有出站合同测试 | 269 → 288 |
| 新生产模块 | frame 127、state 166、tools 143、usage 64 |
| 新测试模块 | 80–177；无新增 >500 文件，无修改 checker/baseline/exception |

## 5. 接力点与未完成范围

1. W06 仍须 reasoningContent/signature/redactedContent 往返、正常空池模板、多字段 UI/SecretPatch/redaction/keep/clear、材料重绑定、精确账号测试和模型策略；不能标整体 offline_ready。
2. Qwen 普通 API 模板/地域工作空间/目录、其他 OAuth 生命周期及新增 Web/媒体 transport 仍按各 W 卡推进；不因本批修 Bedrock 而省略用户要求的平台。
3. Bedrock buffered route 与 reasoning 等完整协议保真不是本批流式 text/tool/usage 证明的范围。
4. 未生成或交付 devtest/正式发布包；Cargo 测试编译产物不作为手测 EXE 交付。没有真实 SDK/AWS 账号验收、长期资源/刷新证据或额度归属实测。集中录制仍安排在全部可无凭据实现的功能完成之后。
5. 先前 Console 全套的 `legacy_compiler_rejects_extreme_token_expiry_without_panicking` 失败不在本批修补范围，不宣称全仓全绿。

## 6. 公开协议依据

- [Smithy EventStream](https://smithy.io/2.0/aws/amazon-eventstream.html)：三类 headers、双 CRC、service/client 大小限制差别。
- AWS SDK 固定 commit `44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7` 的 [event_stream_serde.rs](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/bedrockruntime/src/event_stream_serde.rs#L79-L215)：原生 event header discriminant 与未包装 payload，仅行为参考。
- [MessageStart](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_MessageStartEvent.html)、[Metadata](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStreamMetadataEvent.html)、[Metrics](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStreamMetrics.html)、[TokenUsage](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_TokenUsage.html)、[ToolUseBlockStart](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolUseBlockStart.html)：role、必需 usage/metrics、非负 Integer 和工具身份 schema。

依据已在本批联网核实，只作公开协议参考，不证明实际账号权限、平台可用性或采用第三方代码许可。
