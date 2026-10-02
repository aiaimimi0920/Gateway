# HTTP 主 SSE 领域错误接线，2026-09-30

接续 [缓冲 Responses 错误边界](2026-09-30-http-stream-error-boundary.md)。本批把主 SSE
translator、观察层和实际 `stage_send` 流式输出改成保留调用者错误类型。
**本批源码接线、聚焦测试和编译/静态门禁已通过。生产仍用 rquest，
Rust pin 仍为 1.95.0；HTTP 客户端切换、整体依赖安全和正式 release 均未完成。**

## 迁移边界与兼容性

Anthropic、OpenAI legacy completions、Responses → OpenAI、Bedrock、Cohere 和 Kiro
增加或接入 `*_with_error` 入口。原 rquest 公开入口继续委托同一实现，保留既有调用者
的类型推断与 wire 行为。Responses 的旧入口留在 facade，叶文件只保留泛型协议实现；
Kiro 的旧状态注入测试入口也保持固定类型，原状态机测试无需修改。

`TrackedStream<E = rquest::Error>` 保留旧 `new` / `new_with_started_at`，新增
`new_with_error` / `new_with_started_at_and_error`。usage、completion、archive 新增
泛型入口，不重新包装错误对象；三个观察 wrapper 的 pin 操作改用安全的
`inner.as_mut().poll_next(cx)`，删除不必要的 unchecked projection。

真实生产链已接入：

```text
上游 HTTP / Bytes / Grok 输出
→ 原有首包预检分支（需要预检的 adapter 仍用原 rquest 错误分类）
→ StreamError::Transport
→ 同 E 的协议转换与工具检测（本地失败为 Protocol）
→ 原位置的 usage / completion / archive 观察
→ TrackedStream<StreamError<rquest::Error>>
→ PipelineOutput::Sse
→ HTTP Body 边界装箱，保留错误来源链
```

上图不改变各 adapter 的观察顺序：普通 usage 仍在原转换前位置，Kiro usage 仍在
转换之后。原首包 network classification、provider fallback、permit 释放、路由选择、
成功/失败记录和完成回调未改；领域 variant 本身不增加任何重试决策。

Bedrock / Cohere 的 HTTP 消费入口使用新泛型 translator。Gemini route 是明确的
临时反向兼容边界：在调用 S06 的固定类型 translator 前，唯一一次
`stream.map_err(rquest::Error::from)` 恢复旧类型。`Transport` 返回同一个 rquest
错误对象，`Protocol` 复用既有 legacy 转换。**没有修改 S06 的 Gemini 协议实现，
这个兼容层仍不支持新 HTTP 客户端。**

本批没有把所有既有上游实现内部的本地错误重新分类。仍以 rquest 错误输出的上游
接口，在进入本批 typed chain 时被包装为 `Transport`；只有已经领域化的协议 owner
能提供明确的 `Protocol` 区分，不能把这称为全仓错误语义迁移完成。

## 安全、生命周期和性能复核

28 个已有 owner 的逐项差异已复核：改动限于错误泛型、兼容入口、调用接线和测试注册。
未添加网络请求、认证逻辑、重试、队列、后台任务或新的业务状态；没有增加响应原文、
凭证或 token 日志。原 frame/state/output/归档限额与序列化行为不变。

`TrackedStream` 仍在 EOF、error、drop 时先销毁完整上游链，再 exactly-once 回调。
新测试验证两种错误经过三层观察后保留来源对象、快照和计数，不读取错误后的数据；
取消 pending stream 释放上游且不报告成功。keepalive 位于 tracking 外侧，不计入
上游字节/chunk 指标。HTTP Body 的错误来源仍可下转回 typed domain error。

本批不保证所有 translator 在 yield error 的瞬间释放上游：部分既有 `unfold` 实现
仍在下一次 terminal poll 或 consumer drop 时释放。生产 tracking 在看到错误时立即
销毁整条 chain，Bedrock 的既有 terminal-before-yield 释放合同也保留。

**不是全链有界性验收。** Accio 既有 parser buffer 的无换行输入增长风险未在本批修复；
其他未修改 parser、上游 chunk 分配和进程 RSS 也不在本批保证范围内。不能用下游
限额或本批泛型接线掩盖这些剩余工作。

## 验证实况

| 门禁 | 本批结果 |
| --- | --- |
| 修改前相邻 unit 合同 | 249/249 |
| 修改后相同合同及新增领域合同 | 267/267（原 249 + 新增 18） |
| loopback typed pipeline：Responses / Messages / Completions | 3/3 |
| 原 pipeline send runtime 合同 | 7/7 |
| stream observation contract / limits | 13/13、7/7 |
| Bedrock eventstream contract | 13/13 |
| Windows default all-targets check | 通过 |
| no-default-features lib check | 通过 |
| 根 formatter / checker tests / ratchet | 通过；31/31；通过 |
| 最终 diff / 编码 / 源码保护 | 通过 |

新增单元合同覆盖 decoder 精确边界/超限、传输错误、partial output 后不伪造完成、
pending cancellation、Bedrock 本地状态错误、Cohere/Kiro 错误保留、legacy 来源与标志、
完整观察链、HTTP Body 与 keepalive。集成夹具只监听随机 loopback 端口，使用本轮
临时目录与合成凭证；Responses 夹具显式声明 chat-only 上游，按实际 endpoint policy
进入桥接路径，不为测试修改生产路由。

集成共 43 项通过后，仅把 Messages 夹具的 canonical family 与真实 Anthropic 请求
对齐，单独重跑该文件 3/3，并再次通过 all-targets、formatter 和 ratchet。267 项根
unit 与其余 40 项集成对应的源码/测试没有继续变化，未重复重跑这些已通过合同。
ratchet 使用 `check:effective-lines` 的同一个 checker 入口，省略默认 JSON 输出以
保护原有 dirty `scripts/artifacts/effective-code-lines.json`；没有降低扫描范围或规则。

保留全部失败回执：初次修改前运行误用默认 target，引发冷编译，核对进程树后仅取消
本轮 Cargo，随后复用 `linshi/g95-073651` 通过 249 项。第一次修改后编译因 Kiro
旧测试入口的类型推断失败；保留兼容入口后，实际执行 267 项，其中 266 通过。
唯一运行失败是新增身份测试把 `Box<Marker>` 再次装箱，导致断言下转类型错误；
显式转为错误 trait object 后，根测试重新编译并通过 267/267。没有删减原测试、放宽
断言、修改 checker/baseline 或伪造软上限审批。

Rust 仍有既有 Gemini 等 unused/dead-code warnings，本批没有用 `allow` 隐藏它们。
本批全部 Cargo 门禁使用 offline locked 图；没有再次联网刷新安全数据库，也没有将
早前候选 TLS/audit 证据冒充当前生产客户端验收。

## 权威有效行数

全部 35 个新增/修改 Rust 文件均不超过 500；未新增例外，也未增长旧大文件。
Responses 叶文件为 500 行，后续扩展必须重新设计边界，不能继续堆叠职责。

| 文件（相对仓库） | 修改前 → 后 |
| --- | ---: |
| `src/protocol/anthropic.rs` | 347 → 349 |
| `src/protocol/anthropic/to_anthropic.rs` | 455 → 481 |
| `src/protocol/openai.rs` | 253 → 256 |
| `src/protocol/openai/stream_translate.rs` | 241 → 268 |
| `src/protocol/responses.rs` | 28 → 44 |
| `src/protocol/responses/to_openai_chat.rs` | 487 → 500 |
| `src/protocol/bedrock_converse.rs` | 78 → 81 |
| `src/protocol/bedrock_converse/eventstream.rs` | 329 → 337 |
| `src/protocol/cohere.rs` | 489 → 494 |
| `src/protocol/kiro.rs` | 355 → 377 |
| `src/protocol/kiro/event_stream_translate.rs` | 100 → 99 |
| `src/protocol/kiro_disabled.rs` | 55 → 69 |
| `src/upstream/stream.rs` | 110 → 131 |
| `src/upstream/stream/usage.rs` | 213 → 220 |
| `src/upstream/stream/completion.rs` | 164 → 171 |
| `src/upstream/stream/archive.rs` | 97 → 105 |
| `src/http/sse.rs` | 115 → 124 |
| `src/http/routes/bedrock.rs` | 105 → 107 |
| `src/http/routes/cohere.rs` | 63 → 63 |
| `src/http/routes/gemini.rs` | 123 → 126 |
| `src/protocol/stream_error_legacy.rs` | 6 → 14 |
| `src/protocol/mod.rs` | 75 → 77 |
| `src/pipeline/mod.rs` | 353 → 353 |
| `src/pipeline/stage_send.rs` | 335 → 337 |
| `src/pipeline/stage_send/attempt.rs` | 22 → 21 |
| `src/pipeline/stage_send/stream_preflight.rs` | 152 → 155 |
| `src/pipeline/stage_send/stream_translation.rs` | 274 → 284 |
| `src/pipeline/stage_send/streaming.rs` | 326 → 302 |
| `src/protocol/anthropic/to_anthropic_error_tests.rs` | 新增 72 |
| `src/protocol/openai/stream_translate_error_tests.rs` | 新增 71 |
| `src/protocol/responses/to_openai_chat_error_tests.rs` | 新增 79 |
| `src/upstream/stream/typed_error_tests.rs` | 新增 133 |
| `src/http/sse_error_tests.rs` | 新增 77 |
| `src/protocol/stream_transport_tests.rs` | 新增 132 |
| `tests/sse_typed_pipeline_contract.rs` | 新增 97 |

## 证据、回滚与剩余迁移

证据目录：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-sse-typed-20260930-192824`

- `before/` 与 `before-hashes.json` 保存修改前 28 个 owner 和 3,769 个原有输入。
- `*-receipt.json` / `.log` 保存命令、cwd、实际退出、耗时和执行用例数；失败尝试不作
  最终通过证据。
- `lines-before.json` 与 `final-lines.json` 由仓库权威 lexer 计数。
- `final-audit.json` 确认只有本轮 28 个原有 owner 改变，其余 3,741 个输入 hash 不变。
  S06、manifest/lock、toolchain、安全策略、原 dirty cache 和前批其他源码均保留。
- `completion.json` 汇总真实通过门禁、被替代的失败尝试、源码保护和证据复用边界。
  `validation_complete=true`，`pending_or_failed_gates=[]` 仅指本批门禁，不是整体迁移完成。
- Neuro、Gateway、Hook、Loom、Platform、Talk、Tea 的独立状态记录在
  `final-<repo>-status.txt`；Gateway HEAD 保持
  `c3a74e248402253eca6d6c8ec60685af516814f9`，工作树未提交，不清理其他仓库的变化。

回滚范围只涉及 `scope-before.json` 中的 28 个 owner、本批 7 个新增测试文件和本文；
必须先核对后续修改，不能直接覆盖他人的新内容。无需数据、凭证、schema 或 release
迁移，本批也没有执行回滚、删除、commit、push、reset 或 stash。

下一步是与 S06 owner 协调消除 Gemini 固定错误类型边界，验证候选客户端下的完整
流式链路，再切生产 HTTP 依赖、锁图和工具链，执行错误分类/脱敏/重试、TLS/代理、
真实 provider、完整编译与产品发布门禁。桌面 GLib owning-parent 链仍是独立未关闭项。
本批没有正式 release、真实凭证或付费请求，也没有数据库、Docker、代理、DNS、证书、
网络映射或其他子仓库变更。
