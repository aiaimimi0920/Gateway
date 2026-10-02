# HTTP client 安全迁移基线，2026-09-30

接续 [AWS owning-parent 升级](2026-09-30-aws-dependency-upgrade.md)，本批核清
`rquest` 替换接口并补齐缺失的 TLS wire 基线。**尚未替换生产依赖，也没有移除剩余
LRU/RSA 告警**。当前成果是可复用的迁移保护和明确阻塞，不是客户端迁移完成。

## 当前联网阻塞

本机没有 `wreq-*` 源码，已安装的 stable 与固定工具链都为 Rust 1.95.0。
本批真实请求 crates.io 的 wreq、wreq-util、rquest metadata 和 Rust 1.98.0 官方
分发 manifest，默认代理与进程内直连均在 TLS 阶段报：

```text
[SSL: UNEXPECTED_EOF_WHILE_READING] EOF occurred in violation of protocol
```

补充的 PowerShell TLS 1.2 请求同样失败：`基础连接已经关闭: 发送时发生错误。`。
没有关闭证书或 revocation 校验、修改全局代理、安装未知工具链、切换镜像来源或
创建网络盘。根因尚未确证，不把这些错误描述为候选不存在或没有修复版本。

上一批取得的 stable/RC/MSRV 矩阵仍只是保存快照。本批没有成功刷新候选 metadata、
取得源码或校验新 archive，不能证明任何 wreq 版本的当前可用性/API 兼容性。
在这些条件具备之前，不改 Cargo manifest/lock，不把未经编译的 alias 写进生产。

## 已确认的接口与 owner

生产 `UpstreamClient` 使用 Chrome136 的 `http` 和没有 emulation 的 `plain_http`；
Splitter 两个 client 使用 Chrome131，readiness 有配置超时，流式 proxy 没有短整体超时。
这些 owner 不得在迁移中混合：`src/upstream/client_initialization.rs`、
`src/splitter/service.rs`。quota、refresh、probe 还存在普通 client，不能全量强加指纹。

直接接口除 request builder / headers / json / form / query / bytes_stream 外还包含：

- 与 axum/http `Method`、HeaderMap、StatusCode 的类型互通；
- `Response::from(axum::http::Response)`、`Body::wrap_stream`、`Response::chunk`；
- `Error::from(serde_json::Error::io(...))`、`without_url`、`with_url`；
- `is_timeout`、`is_connect` 分类，特别是 OAuth code 仅在 connect failure 时重试一次；
- 有界 decoded body、charset/BOM decoder、取消时释放 stream；
- 请求级 redirect policy、最终 response URL；
- 默认系统代理和连接复用。反向代理 client 不等于显式出站 forward proxy。

保持 `rquest` / `rquest_util` 的消费端 alias 可以作为后续候选编译方向，以避免
机械改写大量协议类型及 S06 文件；本批没有实施或证明该方向可行。

## 本批新增保护

`tests/http_client_tls_contract.rs` 在 loopback 捕获真实 BoringSSL ClientHello，
不是只断言 builder 字符串：

- 对 Chrome131、Chrome136 校验去除 GREASE 后的 cipher 顺序、支持曲线、签名算法、
  TLS 1.3/1.2 参数与 `h2` / `http/1.1` ALPN 顺序；
- 两个 Chrome profile 分别保留 ALPS codepoint 17513 / 17613，并拒绝串用；
- plain client 不带这两个 Chrome ALPS 扩展。

期望来自缓存 `rquest-util 2.2.1` Chrome 配置的实际源码，并由真实 wire 运行确认，
不是从同一次捕获动态生成期望。每次捕获最大 16 KiB、整体 deadline 3 秒，绑定 IPv4
loopback、关闭该 fixture client 的代理，不联系第三方服务、不使用凭据、不跳过证书验证。
fixture 不完成 TLS server handshake；捕获与发送使用 join，没有 spawned server task，
超时或取消会释放局部资源。解析不使用 unsafe，字段读取和记录累积都有界。

`tests/python/test_gateway_http_client_profiles.py` 将这套 profile 约束连接到生产
构造 owner，并在内存中验证五个错误变体均被拒绝：删除指纹、profile 串用、给 plain
加指纹、替换 Splitter profile、给 proxy 加短整体超时。未写入这些错误变体。

这些检查**不等于完整 JA3/JA4 身份或真实 Grok 验收**；没有验证 TLS server 证书、
SNI/DNS、HTTP/2 SETTINGS、真实上游接受度，也尚未证明候选保留系统代理默认行为。
后续必须用候选跑相同 wire/body/error/cancellation 门禁，再补这些独立边界。

## 本批实际验证

| 检查 | 结果 |
| --- | --- |
| 新 TLS wire 回归 | 3/3 |
| 新生产 profile 合同及五个错误变体 | 2/2 |
| transport timeout / HTTP 504 分类 | 1/1 |
| 有界 body、charset/BOM、body error、取消和 deadline | 6/6 |
| OAuth token transport / 不重放已交付 code | 3/3 |
| stream observation / 错误、早退、释放、usage | 13/13 |
| 根 Windows `cargo check --offline --locked --all-targets` | 通过 |
| formatter / checker tests | 通过；checker 31/31 |
| ratchet / strict | 通过；2606 文件、14 个单列 runtime assets，受治理源码 `>700=0` |
| `git diff --check` | 通过 |

首轮 charset filter 错用了文件名，实际执行 0 项，不计为有效通过。保留该回执，改为
真实模块路径 `protocol::upstream_body::charset_tests` 后 6/6；本批临时 runner 已增加
Cargo test 零用例拒绝条件。没有删除或排除原有测试来制造通过。

两个新文件有效行分别为 **170**、**43**；都不超过 500，没有新软例外。
Cargo manifest、三份 Cargo lock、工具链、生产 Rust、S06 和运行数据未修改；
上一批源码 hash 和三个既有 dirty cache 在最终 completion 回执逐项核对。

## RSA 的独立迁移边界

只读 SQLx owner 调查确认根锁图同时有：

```text
sqlx -> sqlx-mysql -> rsa
sqlx -> sqlx-macros -> sqlx-macros-core -> sqlx-mysql -> rsa
```

仅关闭 sqlx default features 的先前隔离 probe 未移除 rsa。本机 SQLx 0.8.6 同族没有
查到“只调官方 dependency/features、保留现有官方 FromRow derive、同时清除 rsa”组合。
当前没有查到 MySql/Any、query!/query_as!/query_scalar!/migrate! 或 sqlx 自定义 derive
属性，但有 **66 个 FromRow derive、28 个源码文件**。

直接使用同版本 core/PostgreSQL/SQLite runtime crates 是可讨论的方向，不代表已验证。
它还必须消除官方宏链并保持字段名、类型、错误和泛型映射，以及当前 Tokio、rustls
roots、JSON/time、SQLite bundled、WAL/Full synchronous。没有据此替换数据库代码、
修改 schema 或强加本地 facade/自制 derive 来绕过审计。

## 停止与继续条件

整体依赖安全仍未关闭，不增加 ignore、不放宽发布门禁，不生成新正式 release。
本批没有 commit、push、Docker 重启或 .ng/真实数据库变更；其他独立仓库 dirty 状态保留。

下一步需要可验证取得候选源码、metadata/checksum 和必要官方工具链。取得后先做
隔离 API 编译与 TLS/transport 比较，满足兼容合同后才切换生产；不能把当前 rquest
绿色回归冒充候选已通过。RSA 行映射迁移另行设计和验证，不与 HTTP 客户端盲目合并。

本批精确请求、初次零用例回执、最终测试日志、源码 hash 与 completion：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-http-migration-20260930-100550`
