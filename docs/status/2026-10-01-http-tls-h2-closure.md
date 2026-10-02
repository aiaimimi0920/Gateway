# 2026-10-01 TLS 握手与 HTTP/2 本地合同验收

## 结论与范围

在 [Windows 公开 HTTP 路由闭环](2026-10-01-http-packaged-router.md) 的 19 项证据之外，
完成 **12 项 HTTP factory/profile 的 TLS/H2 合同**，以及 **1 项已交付 EXE 的
默认信任根拒绝合同**。全部通过，没有修改生产源码、根依赖锁图、机器代理或系统信任库。

正向本地 TLS/H2 使用单独的验证程序，而非替换已交付 EXE。该程序编译 Gateway
HTTP factory 的原字节副本，并使用与根 manifest 完全一致的 wreq/wreq-util 声明。
所用 150 个 registry 包的 name/version/checksum 均存在于当前生产根 lock，
没有依赖版本漂移；这不表示验证程序与完整 Gateway 有相同的全量 feature 并集。

本段证明完整 TLS 握手和实际 HTTP/2 帧，不再仅停留在读取 ClientHello。
但不等于真实浏览器的完整 JA3/JA4 一致性、ALPS 服务端交互、真实供应商接入，
也不等于已打包 EXE 的正向 TLS/H2 router 全链路。

## 隔离方式

- 测试脚本、Cargo probe、证书、私钥、日志和回执均放在 `linshi/gateway-tls-h2-20261001`。
  私钥仅用于本地合成证书，不复制到仓库、候选包或公开材料。
- 使用短期合成 CA 与 leaf；通过 wreq `tls_cert_store` 仅设置当前验证进程的信任根。
  保留默认链验证与 hostname 验证，未调用关闭验证的开关。
- `.resolve()` 将 `tls-fixture.invalid` 定位到 loopback，`.no_proxy()` 禁用 probe 代理。
  没有修改 hosts、Windows 代理或证书存储；这不是默认代理发现的重复验收。
- Python OpenSSL 服务端与 wreq BoringSSL 客户端实际完成握手。TLS 版本由服务器
  分别限定为 1.2 和 1.3，客户端没有改掉 profile 的版本配置。
- 服务端逐帧读取 HTTP/2 preface、SETTINGS、ACK、HEADERS、DATA、WINDOW_UPDATE、
  RST_STREAM。每帧至多 16 KiB、每次连接至多 64 帧、请求体至多 64 KiB，
  有 socket/子进程 deadline 和已等待完成的线程 owner。
- 伪头检查只解析新连接首个请求的静态 name 索引，跳过字符串值；不是完整通用 HPACK 实现。

## 12 项 factory/profile 合同

| 合同 | 实测结果 |
| --- | --- |
| plain、Chrome131、Chrome136，各 TLS 1.2 与 TLS 1.3 | 6/6；握手成功，ALPN 为 `h2`，HTTP/2 POST Body 与返回文本一致，peer DER 与预期 leaf 原字节一致 |
| 不受信任 CA，保持客户端默认根 | 拒绝，`CERTIFICATE_VERIFY_FAILED`，服务端无完成握手、无 HTTP 请求 |
| 受信任 CA，但 leaf 域名不符 | 拒绝，`CERTIFICATE_VERIFY_FAILED`，无 HTTP 请求 |
| 受信任 CA，但 leaf 过期 | 拒绝，`CERTIFICATE_VERIFY_FAILED`，无 HTTP 请求 |
| 上游先送合法正文，再发送 `RST_STREAM(INTERNAL_ERROR)` | 客户端先收到预期正文，再暴露 body error，没有把错误当作正常 EOF |
| 客户端收到正文后取消 | 服务端收到 stream 1 的 `RST_STREAM(CANCEL=8)`；同一 TLS/H2 连接上的 stream 3 请求成功 |
| 服务器只提供 HTTP/1.1 ALPN | Chrome136 完成 TLS 1.3 并回落 HTTP/1.1，POST Body 与响应正确 |

证书三种负向、RST_STREAM、取消和 HTTP/1.1 回落以 Chrome136 验证，
没有将这六项外推为所有 profile 的完整笛卡尔积。

### Chrome131 / Chrome136 的实际 wire

两种 profile 在 TLS 1.2/1.3 下的首个 SETTINGS 都是以下顺序和值：

| Setting | ID | Value |
| --- | ---: | ---: |
| HEADER_TABLE_SIZE | 1 | 65536 |
| ENABLE_PUSH | 2 | 0 |
| INITIAL_WINDOW_SIZE | 4 | 6291456 |
| MAX_HEADER_LIST_SIZE | 6 | 262144 |

同时观察到：

- connection `WINDOW_UPDATE` increment 为 `15663105`，对应 `15728640 - 65535`。
- 伪头顺序为 `:method, :authority, :scheme, :path`。
- HEADERS priority：dependency 0、exclusive true、wire weight 字节 219。
- 客户端 ACK 服务器 SETTINGS；服务端也 ACK 客户端 SETTINGS。

这些断言源于本地已固定的 `wreq-util 0.2.0` Chrome `http2_options!(3)` 配置，
并由真实帧核对。不是仅从配置对象推断 wire，也未声称等同所有真实 Chrome 流量。
plain 的 SETTINGS、伪头顺序和 window 与 Chrome 不同，保留原始观察，不强套 Chrome 断言。

## 已交付 EXE 的默认信任根拒绝

使用同一个不可变 Windows 候选 `gateway.exe`，独立 SQLite 数据、合成 access key
和 HTTPS loopback provider。服务器证书当前有效，SAN 明确包含 `127.0.0.1`，
唯一不满足的是自签名根不受信任；没有向 EXE 提供自定义 CA 或跳过验证配置。

结果：

- 三次 TLS 尝试都得到 `TLSV1_ALERT_UNKNOWN_CA`，完成握手 0、HTTP 字节 0。
- 公开 Chat Completions 请求返回 500 Network 错误，不伪装成成功响应。
- 消息余额退回并保持为 1。
- 子进程代理指向本地不转发捕获器，捕获器连接数 0。
- Gateway 正常 drain 退出，TLS/代理夹具线程均退出。

此处的三次尝试是现有重试策略的观察；没有在本轮更改 TLS 错误重试或 HTTP 状态映射。

## 失败记录和修正

初次 probe 构建遇到模块路径解析与 `CertStore` 导入位置问题；只调整独立 probe，
最终通过 `rquest::tls::trust::CertStore` 使用公开 API。原构建失败日志保留。

首轮矩阵 `r1` 在域名不符 case 的服务端断言处失败：客户端已明确报
`CERTIFICATE_VERIFY_FAILED`，但 Windows 交付的是 TCP `ConnectionResetError`，
而不是 Python `ssl.SSLError`。修正夹具只在客户端明确证书验证失败、握手未完成、
HTTP 请求 0 的条件下接受该 reset；并未降低证书安全断言。最终 `r2` 12/12 通过。

## 构建、完整性、行数与证据

- 使用隔离 Rustup 的 Rust 1.98.0，Cargo 单 job；离线构建与后续
  `cargo build --offline --locked -j 1` 均退出 0，formatter 通过。
- 验证程序有一个未使用 factory `client()` 的 dead-code warning，未隐藏或禁用。
- 生产 factory 三个副本逐 hash 相同，生产入口配置未变；上一交付的 3,793 个既有
  工作输入无修改、无缺失。候选包 858 个文件仍逐 hash 匹配。
- 最终审计确认本次 15 个 TCP 端口均不接受连接，没有残留 Cargo/rustc/Gateway/probe。
- 新手写文件有效行数：Rust probe 98、TLS fixture 173、matrix 129、证书生成 32、
  packaged TLS 105、audit 58。使用仓库官方 lexer 测量，全部不超过 500；
  Python 语法和 UTF-8 无 BOM 检查通过。生产源码未改，不重复全量编译或行数门禁。

证据根目录：

```text
C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-tls-h2-20261001
```

关键文件：`build-receipt-r2.json`、`dependency-parity.json`、
`factory-source-parity.json`、`r1/result.json`、`r2/result.json`、
`packaged-r1/result.json`、`effective-lines.json`、`audit.json`、`final-check.json`。

未提交、推送、部署、调用真实提供方、覆盖旧 release 或删除 `linshi` 外内容。
完整供应商验收、已打包 EXE 的正向 HTTPS/H2 router、Linux/Docker/macOS、公开发布
安全门禁和历史凭据处置仍需独立完成，`wreq-rt 0.2.2-rc.4` 的 prerelease 状态未改变。
