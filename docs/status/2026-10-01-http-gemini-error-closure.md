# Gemini 流式错误接口与候选客户端编译闭环

接续会话 `01a0f544-d50a-7de1-9631-431919bae8a1`，用户明确允许接管
Gemini 流式错误接口、共享 legacy 适配和相关聚焦测试。
本检查点更新[上一轮候选整库兼容检查](2026-09-30-http-candidate-root-compatibility.md)
中“共享 legacy 错误转换导致候选无法编译”的当前结论；原失败记录保留为历史。

**该限定错误接口阻塞已解决。生产与隔离候选各通过 291 项聚焦单元测试，
九个 integration targets 各通过 57 项；候选默认及关闭默认 feature 的全目标
编译通过。生产依赖仍为 rquest 5 / rquest-util 2、Rust 1.95.0，没有执行
生产 wreq 切换、正式发布或部署。整体 HTTP 迁移并未完成。**

## 接管边界与实际修复

Gemini 新增 `translate_openai_sse_to_gemini_stream_with_error<E: Send + 'static>`。
原 raw `rquest::Error` 入口保留，委托泛型实现；启用与关闭 Gemini feature 的
导出路径都已接线。此投影器不构造本地协议错误，因此不要求
`E: From<ProtocolStreamError>`。正常文本、工具参数、usage 和 finish 的 wire
投影逻辑不变，没有开展其他 Gemini parser 重构。

流收到错误后交付原对象，并立即释放上游、缓冲和工具/output 状态，后续不再
轮询 late data、不恢复输出、不伪造成功完成。`unfold` 使用终态 `None`，外层
`.fuse()` 保证 EOF 后重复轮询安全。Gemini HTTP response builder 直接消费
typed stream，使用 `TrackedStream::new_with_error`，不再反向转换客户端错误。

共享 `stream_error_legacy.rs` 去掉两个反向 `From` 实现，现在只将 raw 输入
包装为 `StreamError::Transport`，再交给原泛型协议状态机。
没有使用伪造请求构造本地协议错误，也没有改动原
`src/protocol/responses/to_openai_chat.rs` 状态机。

### Rust library 接口兼容性说明

以下旧入口继续接受 raw 客户端 stream，但返回的错误类型从 `rquest::Error`
变为 `StreamError<rquest::Error>`，这是实际的 Rust 返回类型变化，不是完全
source-compatible 的替换。固定要求 raw 输出的库调用者需要接受领域错误类型。

- `translate_openai_sse_to_anthropic`
- `translate_openai_chat_sse_to_legacy_completions`
- `translate_responses_sse_to_openai_chat` 及其旧 limit 入口
- `translate_openai_sse_to_responses`
- `translate_openai_sse_to_bedrock_eventstream`
- `wrap_streaming_tool_detection`

正常 HTTP wire 格式不因上述返回类型变化而改变；Gemini 的错误终态和取消清理
按本轮合同验证，不能把错误后仍继续输出当成需要保留的正常行为。

## 聚焦回归与真实 HTTP slice

新增两个小型测试 owner：
`src/protocol/gemini/api/stream_error_tests.rs` 和
`src/http/routes/gemini_stream_error_tests.rs`。
覆盖 UTF-8 跨 chunk、CRLF、分段工具参数、usage、STOP，非零大小 Box 错误对象
地址保持，partial output 后错误终止、原 Protocol variant 保留，以及未轮询、
Pending、排队输出时的取消释放。实际 Gemini response builder 和 Axum Body
验证领域 source chain、终止及 exactly-once 清理。

既有协议、观察器和错误测试改用真实响应 Body 产生客户端错误，不再依赖
`serde_json::Error -> rquest::Error` 构造器；原原因、对象身份、分类与终止断言
保留。source chain 测试支持最多遍历 16 层。URL secret 脱敏单元测试使用本地
invalid-header request builder 产生错误，不发送网络请求。

`tests/sse_typed_pipeline_contract.rs` 新增 Gemini slice：真实随机 loopback
HTTP 经 `stage_send`、typed stream 和 Gemini translator，使用实际
`ProtocolFamily::GeminiGenerateContent`，验证文本、usage、finish 和一次请求/
完成观察。这不是完整 router、`run_pipeline`、真实提供方或代理验收。

## 候选专属适配与 Body 分类差异

候选继承前阶段已审阅的 wreq 0.16.1 / wreq-util 0.2.0、Rust 1.98.0、URI、
安全 Debug 和 TLS Profile 适配及完整锁图。全目标编译进一步暴露两处候选
测试 owner 的 API 差异，仅在隔离副本修复：

- `src/upstream/common.rs` 测试读取 `Request::uri()`，不使用旧 `url()`。
- `src/upstream/gemini_canvas_upload_bytes_tests.rs` 使用公开 HttpBody/
  `BodyExt::collect().to_bytes()`，不依赖不存在的 `Body::as_bytes()`。

零拷贝测试仍验证原分配地址；先释放外部 selected owner，再消费请求 Body。
fallback 内容独立所有权及空 encoded 不回退断言保留。生产对应两个文件原字节
未改，不修改其上传业务实现。独立只读复核未发现这些断言被实质削弱。

最后更新的 `http_client_body_contract` 明确接受两个已确认的分类 flag 对，而非
假称客户端低层行为完全相同。两个客户端的实际 `--nocapture` 输出分别为：

```text
BODY_CLASSIFICATION decode=true request=false body=false
BODY_CLASSIFICATION decode=false request=true body=false
```

第一行为生产 rquest，第二行为候选 wreq。测试仅允许 `(is_decode, is_request)`
为 `(true, false)` 或 `(false, true)`，继续要求原始 cause 留在 source chain，
且不是 outgoing body、connect 或 timeout 错误；真实 Gateway 分类必须为
`ErrorKind::Unknown`、非重试。原 Body 预算、取消和固定脱敏错误合同未删除。

## 实际门禁与失败记录

所有 Cargo 门禁串行、单 job、`--offline --locked`，使用隔离 TEMP/data 和
明确的生产或候选 cwd，没有并发重建、正常 Rustup home 修改或新网络映射。
这些是聚焦 runtime tests 与全目标编译，不是完整测试 suite。

| 门禁 | 真实结果与回执名 |
| --- | --- |
| 修改前生产聚焦单元 | 279/279；`before-unit` |
| 修复后生产聚焦单元 | 291/291；`after-unit-r3` |
| 候选聚焦单元 | 291/291；`candidate-unit` |
| 生产九个 integration targets | 57/57；`production-integration` |
| 候选九个 integration targets | 57/57；`candidate-integration-r2` |
| 最后 Body 夹具修改后的生产复验 | 5/5；`production-body-final` |
| 候选 Body flag 记录 | 1/1；`candidate-body-classification-final` |
| 生产默认 / 关闭默认 feature 全目标编译 | 均退出 0；`production-all-targets`、`production-disabled-all-targets` |
| 候选最终默认 / 关闭默认 feature 全目标编译 | 均退出 0；`candidate-final-all-targets`、`candidate-disabled-all-targets` |
| 生产 / 候选最终 formatter | 均通过；两个 `*-format-check-final` |
| 行数 checker 测试 / 最终 ratchet | 31/31；`checker-tests`、`ratchet-final` |
| 完整候选锁图审计 | 452 包，退出 0，漏洞 0、warnings 空 |

生产 57 项和两个全目标编译门禁在最后 Body 夹具调整前执行；其余生产源码
未变，最后该 target 单独编译和复验 5/5，不将旧结果冒充整个矩阵重新执行。
候选 57 项及最终两个全目标门禁均包含最后 Body 版本。
关闭默认 feature 的门禁是编译检查，不是该配置下完整 runtime suite。
编译日志仍有既有 unused/dead-code 和 linker warnings；上表 warnings 空仅指
Cargo audit 的审计 warnings。

九个 integration targets 的生产/候选通过数一致：Bedrock eventstream 13，
Body 5，分类 4，TLS 3，network timeout 1，pipeline send 7，typed SSE 4，
stream observation 13，observation limits 7。

本轮失败均保留原日志：Responses 测试 collector 固定 raw 类型已泛型化；
`unfold` EOF 后重复 poll panic 已由 `.fuse()` 修复；候选错误的 Gemini 枚举名、
旧客户端错误构造、Body API 测试不兼容均已按真实诊断修正。
候选首次 integration 在执行前因缺失 `http_client_classification_contract`
target 退出 101；旧候选快照确实没有该文件。现将生产既有测试原样复制到候选，
验证其 hash 与本轮开始时完全相同，并保留 supplemental provenance。
没有修改生产分类器、manifest 或测试断言来消除该输入遗漏。

## 锁图审计与增量质量

本轮于 `2026-10-01T07:14:48Z` 联网核对官方 RustSec HEAD，与本地 clean
数据库同为 `9b3a3b73a7f42606494c943e95f8196e9994df46`。
最终于 `2026-10-01T08:12:19Z` 对完整候选锁图执行：

```text
cargo audit --no-fetch --deny warnings --db <verified-rustsec-cache> --file <candidate-root/Cargo.lock> --format json
```

锁图 SHA-256 为
`7ebaad772cdbe353cceeffc01025bd5ffbb35fe078d2f5e3887c47d3f24ec9e4`，与上一阶段
完整候选锁图原字节相同；没有 ignore、target 过滤或局部探针替代。
`wreq-rt 0.2.2-rc.4` 仍是 semver prerelease，不能称为全 stable 图。
这只是经本轮联网核实公告快照的 Cargo 依赖审计，不是全项目安全验收。

最终所有本轮 owner 和候选测试适配均不超过 500 有效行，无新例外或 baseline
更新。完整文件列表见 `lines-owner-final.json` 和 `lines-candidate-final.json`。

| 关键 owner | 修改前 → 后有效行 |
| --- | ---: |
| Gemini stream | 393 → 403 |
| Anthropic translator | 481 → 486 |
| Responses → Responses | 419 → 424 |
| Tool streaming | 426 → 429 |
| Body contract | 99 → 108 |
| 新 Gemini 投影 / HTTP 测试 | 139 / 66 |
| 候选 request builder 测试 owner | 143 → 146 |
| 候选上传 Body 测试 owner | 49 → 51 |
| 受保护的 Responses → OpenAI 状态机 | 500 → 500，原字节未改 |

终态实现不新增后台任务、阻塞 I/O、重试或缓冲副本；丢弃流会释放其资源 owner。
测试仅使用合成错误和 secret、随机 loopback 端口及既有 deadline/预算，不调用
提供方或使用真实凭证。新泛型错误不降级为 transport，URL 脱敏合同仍有效。
既有 Gemini parser/buffer/tool 有界性及 Accio 缓冲不是本轮已治理的问题。

## 输入保护、交付及后续边界

证据根目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-http-gemini-closure-20260930`。
开始日期保留为目录名，跨日执行时间以回执为准。

- `before-hashes.json` 记录 3,779 个原输入；本轮仅 24 个授权 owner 改变，
  其余 3,755 个原输入保持 hash，新增仅两份 Gemini 测试和本检查点。
- 生产 `Cargo.toml`、`Cargo.lock`、`rust-toolchain.toml` 及三个受保护状态机/
  上传 owner 保持本轮起点原字节，不表示这些文件相对 Git HEAD 本来就是 clean。
- `candidate-supplemental-provenance.json` 记录缺失分类合同的原样补齐；
  候选其余变化限定为授权 owner 和候选专属 API 测试适配。
- `*-receipt.json`、原失败/成功日志、审计和测量回执均保留；
  `completion.json`、`final-source-protection.json` 是最终聚合和输入保护核对。
- Neuro、Gateway、Hook、Loom、Platform、Talk、Tea 分别记录 Git HEAD/status；
  Gateway/Neuro `git diff --check` 检查。没有将其他子项目状态记录当成其测试通过。

不 commit、push、reset、stash；保留原 dirty worktree、真实凭证、`.ng`、数据库、
代理、Docker 和既有 release。没有删除 linshi 外内容或修改 memories。

下一阶段仍需明确生产依赖切换及回滚方案，再验证完整 router/run_pipeline、
真实提供方和代理、HTTP/2 settings/系统代理兼容以及正式构建与 release。
本轮不提前执行这些操作，也不把候选全目标编译或局部 HTTP slice 当作发布完成。
