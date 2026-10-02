# HTTP 候选整库 API 适配与错误分类验证

接续 [主 SSE 领域错误接线](2026-09-30-http-sse-typed-pipeline.md) 和
[客户端候选验证](2026-09-30-http-candidate-validation.md)，恢复会话
`01a0f544-d50a-7de1-9631-431919bae8a1` 的最后编译任务。
**生产仍使用 rquest 5 / rquest-util 2、Rust 1.95.0；整库候选编译仍失败，
HTTP 迁移和正式发布没有完成。** 实际执行时间以回执的 UTC timestamp 为准。

## 实际推进范围

沿用 `linshi/gateway-http-next-20260930-213930/candidate-root` 的真实源码副本、
原 `build.rs`、真实 Web ready/hash 输入，以及隔离 Rust 1.98 / 单 job 编译缓存。
没有删除旧验证、覆盖 release、修改正常 Rustup home 或创建网络映射。

上一轮的候选根库编译实际退出 101，有 17 个错误，不是仍在运行：

- 8 处错误脱敏 API：`without_url()` 已更名为 `without_uri()`。
- 7 处最终响应地址读取：`Response::url()` 已更名为 `Response::uri()`。
- `wreq::Client` 不实现 `Debug`，原 `UpstreamClient` 的 derive 无法编译。
- `wreq::Error` 不实现 `From<serde_json::Error>`，旧协议错误转换无法编译。

前三类只在隔离副本适配。新增 18 有效行的独立 `client_debug.rs` 保留非敏感
timeout、策略及配置存在性；不输出 HTTP 客户端、bearer token、执行器地址、
数据库连接或运行时对象。构造、资源所有权、请求、fallback 和释放逻辑没有改变。
Gemini 相关 HTTP 消费点只在副本做最终 URI API 替换，工作仓库对应文件和整个
`src/protocol/` 均保持原字节；这不是接管 S06 的 Gemini 流式实现。

适配后相同的 `cargo +1.98.0 check --offline --locked --lib` 仍退出 101，
从 17 个错误收敛到 1 个：

```text
error[E0277]: the trait bound `wreq::Error: std::convert::From<serde_json::Error>` is not satisfied
  --> src\protocol\stream_error_legacy.rs:7:9
```

这个单一诊断位于共享 legacy 转换，不代表只有一个调用者需要迁移。多个协议
旧公开入口仍要求客户端错误实现 `From<ProtocolStreamError>`；Gemini route 还会
将 typed stream 反向转换给 S06 的固定错误类型接口。后续必须协调 Gemini 接口，
同时收缩旧公开入口和相关测试的客户端错误构造依赖。
没有用伪造请求、禁用协议或隐藏编译失败制造兼容层。

## 新增的可复用回归合同

生产仓库仅新增 `tests/http_client_classification_contract.rs`，138 有效行。
四项测试调用真实 `classify_network_error`，只访问随机 loopback 端口：

1. 连接拒绝保持 `Network`、可重试和 provider fallback，并清洗 query token。
2. 等待响应头超时保持 `Timeout` / HTTP 504 / 原 retry delay，不变成 connect 错误。
3. 已收到 HTTP 200 后的截断 body 保持非 connect、非 timeout 的分类。
4. 已收到 HTTP 200 后的停滞 body 保持 timeout，但不满足 connect-only 重试条件。

夹具使用合成 secret、`no_proxy()`、3 秒外层 deadline、8 KiB 请求头上限。
没有 detached task，超时会丢弃整个 join future；正常结束释放 listener/socket。
不使用真实凭证、不调用外部提供方、不写入已有数据库或本地 `.ng`。
这证明错误分类及 OAuth connect-only 条件，不替代 pipeline 已交付后不重放合同。

生产路径运行该 integration target，实际通过 4/4。候选局部工程复制真实
`src/error.rs`、classification/diagnostics 子模块和完全相同的测试，使用
`neuro_gateway` 库名，没有伪造 GatewayError、分类器或 routing 代用品。
候选局部图的所有 registry package 的 name/version/source/checksum 都在完整候选
锁图中。局部通过不是完整 Gateway 根库编译通过。

候选局部工程另复用原 `http_client_tls_contract.rs`。首次编译确认新
`Emulation::Chrome131/136` 常量的值类型是 `Profile`，而夹具参数仍声明
`Option<Emulation>`。仅加入 `Profile` import 并改参数为 `Option<Profile>`，
没有改 cipher、curve、signature、ALPN、TLS version、Chrome ALPS 或任何断言。
这些类型适配也留在隔离根候选的测试副本中，生产 TLS 测试没有改动。
最终同一次 Cargo 运行通过分类 4/4 和 TLS 3/3。

## 完整候选锁图与联网审计

本轮联网执行 `git ls-remote https://github.com/RustSec/advisory-db.git refs/heads/main`。
官方 HEAD 与本地 clean 数据库同为
`9b3a3b73a7f42606494c943e95f8196e9994df46`，因此复用该已核实缓存，没有改写
以前的数据库快照。随后对完整候选 `Cargo.lock` 执行：

```text
cargo audit --no-fetch --deny warnings --db <verified-rustsec-cache> --file <candidate-root/Cargo.lock> --format json
```

实际退出 0：完整候选锁文件 452 个包，漏洞 0、warnings 空。
没有 ignore、target 过滤或跳过 yanked 检查；不是此前 151 包小探针的审计。
`cargo metadata` 的 Windows filter 只用于本机编译准备，不用于该完整锁图审计。
这仍是 RustSec Cargo 依赖证据，不是全项目 OSV、secret、容器或运行时安全验收。

生产锁图 454 包，候选锁图 452 包；按 name/version/source/checksum 比较，
移除 32 个旧节点、加入 30 个节点，完整条目在 `lock-review.json`。
确认旧 `rquest`、`rquest-util`、`boring2`、`boring-sys2`、`tokio-boring2` 和
LRU 0.13 链消失。主要新节点是 wreq 0.16.1、wreq-util 0.2.0、btls 0.5.6、
LRU 0.18.5、wreq-proto 0.2.6；同时更新 Tokio 1.53.1、HTTP 1.5.0 和多个
futures 子包 0.3.34。不能将这些共用依赖升级描述为仅换客户端名字。
**wreq-rt 0.2.2-rc.4 是 semver prerelease**，候选不能称为全 stable 图。
WASI 版本的 build metadata 中带有 `rc` 不会被误算成 semver prerelease。

## 本轮门禁与证据边界

| 门禁 | 实际结果 |
| --- | --- |
| 原客户端上的新分类 integration target | 4/4 |
| 候选真实 error 模块的分类合同 | 4/4 |
| 最终候选分类 + 两个 Chrome/普通客户端 TLS 合同 | 7/7 |
| 完整候选 Cargo 锁图审计 | 452 包；退出 0；漏洞 0；warnings 空 |
| 原生产 Rust formatter | 通过 |
| 隔离根候选 / 局部工程 Rust formatter | 均通过 |
| 仓库行数 checker 测试 / ratchet | 31/31；通过 |
| Gateway / Neuro `git diff --check`、UTF-8 无 BOM 和输入保护 | 通过 |
| 隔离 Gateway 根库编译 | **失败：1 个 legacy 错误构造诊断** |
| 候选全目标、关闭 feature、完整 pipeline 和产品 release | **未验收** |

前阶段 267 单元和 43 integration 的生产源码未变化，不重跑或将其冒充 wreq
整链结果。根候选的全目标测试还需要处理旧客户端专属夹具；本轮只暴露并适配了
TLS 参数类型，不能把 lib 错误数当作后续所有门禁的完整错误清单。

失败回执保留：原 17 错误编译、适配后的 1 错误编译、新测试首次使用错误的
crate/futures import、局部图首次 futures pin 不一致，以及 TLS 夹具类型错误。
只修正已确认机制，不删断言或降低门禁。新测试最终使用仓库既有
`neuro_gateway` / `futures` 名称；局部图 pin 已对齐完整候选并重新核对。

## 权威有效行数

下列源码适配只存在于隔离候选，全部不超过 500：

| 文件 | 修改前 → 后 |
| --- | ---: |
| `src/console/chatgpt_oauth/material.rs` | 150 → 150 |
| `src/provider_quota/codex.rs` | 151 → 151 |
| `src/provider_quota/accio_http.rs` | 169 → 169 |
| `src/provider_quota/generic_balance.rs` | 118 → 118 |
| `src/upstream/gemini/web_reverse/bootstrap.rs` | 66 → 66 |
| `src/upstream/client/canvas_stream_generate/bootstrap.rs` | 247 → 247 |
| `src/upstream/gemini_canvas_upload_http.rs` | 294 → 294 |
| `src/upstream/client/canvas_stream_generate/send.rs` | 463 → 463 |
| `src/upstream/gemini_canvas_direct_http_json_helpers.rs` | 224 → 224 |
| `src/upstream/client.rs` | 411 → 411 |
| `src/upstream/client_initialization.rs` | 43 → 45 |
| `src/upstream/client_debug.rs` | 新增 18 |
| `tests/http_client_tls_contract.rs` | 170 → 170 |

生产新增分类测试为 138 行；证据目录内的适配、审计、测量、回执脚本也均不超过
150 行，以 `final-lines.json` 的权威 lexer 结果为准。没有新例外、baseline 更新、
旧大文件增长或源码压缩。URI 适配不增加响应正文读取或缓存；既有 quota/body
有界性、Accio parser 风险和提供方行为不由本轮证明安全。

## 可恢复交付和下一步

证据根目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-http-next-20260930-213930`

- `candidate-original-hashes.json`、`compat-before/` 保存真实候选输入和适配前 owner。
- `candidate-baseline-check-*`、`candidate-compat-check-*` 保留完整编译退出和日志。
- `lock-review.json`、`full-candidate-audit*` 和 `rustsec-*-head.stdout` 保存完整锁图及联网核对。
- `candidate-classification-tls-contract-r2-*` 保存最终 7 项运行结果。
- `final-source-protection.json` / `completion.json` 保存原 3,777 个生产输入的
  hash 核对、候选范围、独立仓库状态和通过/失败的分开汇总。
- 生产 4 项合同和本轮 checker/ratchet 回执位于前阶段
  `linshi/gateway-sse-typed-20260930-192824/classification-*`；命令和真实 cwd 均保留。

本次只在 Gateway 新增该检查点和分类测试；没有修改已有生产源码、manifest、
lockfile、pin、S06 协议或原 dirty 缓存。其他独立仓库状态单独保存，不清理或
将 Gateway 结论外推给它们。没有 commit、push、reset、stash 或任何发布切换。

Gemini 接口的限定接管问题已经提出，尚未收到明确授权。授权后应按既有状态机
和 wire 合同做最小 typed-error 接口迁移，并处理其他 legacy 公开入口/测试，
再跑真正的候选全库/全目标/feature/pipeline 门禁。只有通过后才能考虑生产依赖、
工具链和 release 切换。桌面 GLib 链、Accio 缓冲及真实提供方/代理验收仍独立未完成。
