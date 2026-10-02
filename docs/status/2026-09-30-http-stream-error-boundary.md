# HTTP 协议流错误边界：缓冲 Responses 接线，2026-09-30

接续 [HTTP 候选与有界读取](2026-09-30-http-candidate-validation.md)。本批收拢本地
stream error，并把领域错误接入真实 Anthropic → Responses 缓冲桥接链。
**生产 HTTP 依赖仍是 rquest，主 SSE/观察层尚未迁移，整体依赖安全未关闭，
没有新正式 release。** 原有 AWS、SQLx/RSA、安全 workflow 和三个 reader 改动保留。

## 实际完成的边界

`src/protocol/stream_error.rs` 定义：

- `ProtocolStreamError`：本地 SSE 解码、协议状态/限额和 JSON 编码错误，保留
  `serde_json::Error`/IO 来源；不再由业务 owner 构造 HTTP 客户端 Error。
- `StreamError<T>`：区分原始 `Transport(T)` 和 `Protocol(ProtocolStreamError)`。
  Display 转发原文，Error::source 保留原始对象；类型本身不引入重试决策。

共享 bounded decoder、Anthropic/Bedrock 本地状态和工具检测状态都改为产生本地
协议错误。旧公开 stream 接口仍保持 `rquest::Error`，由唯一的
`stream_error_legacy.rs` 集中转换，保留 decode/connect/timeout 标志与既有文案。
该兼容转换有意用于旧接口，**不支持新客户端，也不是生产迁移已经完成**。

Responses translator、工具检测、Accio 和 accumulation 新增同 E 的
`*_with_error` 泛型入口；旧公开入口继续委托，避免改变原调用点和测试的类型推断。
`tool_stream::ByteStream<E = rquest::Error>` 和内部 wrapper 保留调用者的 E。

真实生产入口 `stage_send/responses_bridge.rs` 委托独立的
`responses_bridge_stream::accumulate`：

```text
response.bytes_stream()
→ map_err(StreamError::Transport)
→ Accio Anthropic-like → OpenAI
→ XML tool detection
→ OpenAI → Responses
→ Responses accumulation
→ 原 GatewayError / canonical response / route feedback
```

入口仅映射一次 transport error；后续本地错误产生 `Protocol`，不落回 legacy
转换。保留 model、req_id、tools、tool_choice、conversation hint、工具检测开关、
text/usage/finish 和转换顺序。原 controller、指标、provider 记录、permit、candidate
选择、pack_response 和 Next/Stop 决策未修改。

accumulation 仍把流错误转换为原 `GatewayError::server_error`。因此领域区分保留到
accumulation，**没有把 route failure classification 改成新的协议/传输分类**。
这不意味着禁止原有 provider fallback，也没有把本地错误送进 connect-error 分类。

## 资源和安全复核

原 SSE frame、工具原字节/text/chunk、Bedrock state/output 和 Responses body 上限不变。
EOF、原始字节转发、序列号和完成事件逻辑不变。新链不创建任务、channel、数据库连接
或后台 owner，没有新增网络请求、重试或无界容器。

这不是全链有界性验收：Accio 既有 parser buffer 本批只泛型化，没有新增 frame admission；
它在无法解析且未遇到换行的输入上仍可能持续增长。其限额/扫描成本需要独立行为合同，
不能用下游 tool/Responses 限额掩盖。上游 chunk 分配、进程 RSS 也不是本批保证的上界。

在 typed buffered bridge 中，首个错误立即结束 accumulation，并在 route feedback
之前销毁整条 owned chain；取消 future 也递归 drop。测试覆盖不读取错误后的 chunk、
pending cancellation 和 exactly-once upstream drop。

不能把这描述为所有 translator 在 yield error 的瞬间都释放上游：原 `unfold` 状态机
有些会暂存上游，直到下一次 terminal poll 或 consumer drop。本批保持该行为；
缓冲 bridge 随 accumulation 返回立即 drop。Bedrock 和 TrackedStream 的原 terminal
释放合同没有改动。usage/archive observers 仍走旧接口，不据此宣称它们已经领域化。

错误文案只转发原有来源或固定限额/分配诊断，没有增加原始响应体或凭证日志。
本地错误保留历史 `error decoding response body` 前缀，以免改变既有客户端合同。
来源链内部增加领域层是显式的新接口语义；legacy 转换恢复旧客户端的来源包装。

## 验证实况

修改前七组原合同共 **245/245**：Responses 42、工具检测 59、Anthropic 55、OpenAI 58、
Accio 19、Bedrock unit 3、shared decoder 9。修改后相同原合同均通过；新增 15 项领域
合同通过（错误类型/来源 4、Responses 4、工具检测 4、真实 bridge 3），另有工具放置
合同 5/5。各组实际执行结果为 46/63/55/58/19/3/9、领域错误 4 和 bridge 3。

| 门禁 | 本批结果 |
| --- | --- |
| 上述根 unit 合同 | 265/265 |
| 候选领域错误 / 真实 bounded decoder 源码 | 4/4 |
| 根 Windows default all-targets check | 通过 |
| 根 feature-disabled lib check | 通过 |
| Bedrock eventstream / stream observation contracts | 13/13、13/13 |
| 根 formatter | 修正后的最终复核通过 |
| checker 测试 / ratchet | 31/31；最终 ratchet 通过 |
| diff / 编码 / 源码保护 | 全部通过 |

首次来源身份测试错误地直接比较两个 `dyn Error` 指针，连同可能重复的 vtable 一起
参与比较，结果为 3/4。修正为下转具体错误类型后比较对象地址，生产实现不变，
根实际重新编译并通过 4/4；候选相同稳健断言也通过。初次失败回执保留。
此外，PowerShell JSON 数组包装导致第一次 scoped formatter 的参数无效；改为
Python 管理 scope 后通过，没有因此修改 formatter、checker 或源文件策略。

编译仍有既有 Gemini 等 unused/dead-code warnings；隔离候选因没有加载 legacy adapter
和完整生产入口，报告 `into_source` 与默认限额常量未使用。未添加 allow 或把门禁描述为
零 warning。

候选复用上批锁图及隔离 Rust 1.98，manifest/lock 字节一致；正常用户工具链和仓库
pin 仍为 1.95。本批直接编译仓库的领域错误与 bounded decoder 源码，验证精确限额、
超限终止/释放、真实客户端 body error 的原始来源和控制标志，以及取消释放。
候选 `Response::from` 的 body error 仍是 `is_request()`，不是旧客户端的 decode；
没有隐藏上批已确认的差异。

这四项不覆盖候选上的完整 Responses/tool/observation pipeline，也不代替新依赖下
的 Gateway 全量编译、TLS、系统代理或真实 provider 验收。旧 TLS/transport/audit 证据
仅按未变锁图复用，本批没有再次联网刷新 RustSec 或宣称完成新审计。传递依赖
`wreq-rt 0.2.2-rc.4` 仍是 prerelease。

## 权威有效行数

本批全部手写源文件不超过 500；没有新增软例外、修改 checker/baseline 或增长旧大文件。

| 文件（相对仓库） | 修改前 → 后 |
| --- | ---: |
| `src/protocol/mod.rs` | 71 → 75 |
| `src/protocol/stream_decode.rs` | 307 → 307 |
| `src/protocol/anthropic/to_anthropic.rs` | 455 → 455 |
| `src/protocol/bedrock_converse/eventstream.rs` | 329 → 329 |
| `src/protocol/openai/stream_translate.rs` | 241 → 241 |
| `src/protocol/responses/to_openai_chat.rs` | 487 → 487 |
| `src/protocol/responses/to_responses.rs` | 392 → 419 |
| `src/protocol/responses/accumulate.rs` | 309 → 325 |
| `src/protocol/responses.rs` | 23 → 28 |
| `src/protocol/tool_inject/streaming.rs` | 379 → 426 |
| `src/protocol/tool_inject.rs` | 98 → 98 |
| `src/protocol/accio/line/stream_translate.rs` | 333 → 338 |
| `src/protocol/accio/line.rs` | 375 → 376 |
| `src/protocol/accio_disabled.rs` | 293 → 299 |
| `src/pipeline/stage_send/tool_stream.rs` | 120 → 125 |
| `src/pipeline/stage_send/responses_bridge.rs` | 198 → 186 |
| `src/pipeline/stage_send.rs` | 334 → 335 |
| `src/protocol/stream_error.rs` | 新增 61 |
| `src/protocol/stream_error_legacy.rs` | 新增 6 |
| `src/protocol/stream_error_tests.rs` | 新增 58 |
| `src/protocol/stream_error_test_support.rs` | 新增 50 |
| `src/protocol/responses/to_responses_error_tests.rs` | 新增 94 |
| `src/protocol/tool_inject/streaming_error_tests.rs` | 新增 110 |
| `src/pipeline/stage_send/responses_bridge_stream.rs` | 新增 39 |
| `src/pipeline/stage_send/responses_bridge_stream_tests.rs` | 新增 76 |

短 compatibility adapter 是独立的客户端耦合边界，使领域 owner 可在隔离候选上编译，
不是无职责的一行代理。小型 bridge owner 隔离真实接线，以便不构造 AppState、数据库
或付费请求就能执行同一生产链的合同。

## 证据、回滚和下一步

证据目录：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-stream-boundary-20260930-183411`

- `before/` 保存 17 个修改前 owner 的原始字节；`before-hashes.json` 覆盖全部已有输入。
- `lines-before-complete.json` / `lines-after.json` 为权威 lexer 测量。
- `*-receipt.json` 保存实际命令、cwd、退出码、耗时和日志路径。
- `completion.json` 汇总源码保护、编码、门禁和独立仓库状态；`validation_complete=true`，
  `pending_or_failed_gates=[]`。初次失败在 superseded attempts 中保留，不充当最终门禁。
- `candidate-probe/` 是独立 harness，使用仓库原源码路径，不复制替代 decoder 实现。

当前保护核对：仅 17 个已有 owner 发生预期改变，3743 个其他已有输入 hash 不变，
原 dirty cache、S06、manifest/lock、工具链、安全策略和前批源码均保留。回滚只涉及本批
owner、模块注册和新增文件，不触碰数据库、凭证或 release；执行前仍须核对后续修改，
不得覆盖其他人的工作。

最终独立 Git 状态项：Neuro 93、Gateway 67、Platform 1、Talk 2，Hook/Loom/Tea clean。
这些是 status 项而非修改文件数；本批只写 Gateway 和外部 linshi，没有清理其他仓库。
Gateway HEAD 保持 `c3a74e248402253eca6d6c8ec60685af516814f9`，工作树仍未提交。

下一阶段先把主 SSE translator 和 observation 改成保留调用者 E，再在 preflight 接入
领域错误；单独处理客户端 Gemini route 对 S06 固定错误类型的反向边界，不接管 S06。
之后才进入生产 HTTP 依赖、锁图和工具链切换，并验证分类/脱敏/重试、完整编译及产品
发布门禁。桌面 GLib owning-parent 链仍是独立未关闭项。

本批没有真实凭证或付费上游调用，没有数据库、现有 release、Docker、代理、DNS、证书
或网络映射变更，没有 commit、push、reset、stash 或 memories 更新。
