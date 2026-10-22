# CPA Rust TLS 配置实验

状态：**TLS 配置已实现并验证；尚未接入 Gateway 请求执行链，不替代 Go worker。**

该独立 library 固定使用 `rama-boring = 0.6.20`，提供
`configure_context` 和 `configure_connection`。它不启动子进程，不引入 Go
runtime；底层仍是 C/C++ BoringSSL，并非纯 Rust 密码实现。

调用方必须使用独立的 TLS context，在创建连接前配置 context、在第一次握手前
配置 connection。信任根、主机名验证、认证、代理、HTTP、超时和取消属于调用方。
该 library 不接收凭证，也不关闭证书验证。不要把它用于已经开始握手的连接。

## 已验证范围

- 对照真实 CPA / Go 1.26.1 的首次 ClientHello，归一化后的所有已解析字段一致。
- Windows 下与 `wreq 0.16.1` 同进程链接、创建客户端并运行成功。
- 使用 Windows 信任根向已授权测试服务商完成 TLS 1.3 握手，ALPN 为 `h2`，
  协商曲线为 `X25519MLKEM768`，证书验证成功；真实 ClientHello 对照也一致。
- 本地测试重复三次检查 record、cipher、扩展内容/顺序、key share 长度，
  并检查 `SslVerifyMode::PEER` 未被关闭。

## 不在已验证范围内

- HTTP/1.1 原始字段序列、HTTP/2 SETTINGS/HPACK/帧顺序、连接池及取消语义。
- 完整 Gateway 路由、SSE、错误分类、解压及真实模型对话。
- TLS 会话恢复、HRR、TLS 1.2 服务端及跨平台行为。
- 完整 CPA 身份/正文兼容模式。

`UnknownValues::Preserve` 只保留广告编号，并不实现不支持的密码算法。
特别是 Go 广告的组 `4587`、`4589`，不能因为出现在 supported_groups 中就声称
BoringSSL 可以完成这些组的协商。服务端选择不支持的算法仍可能握手失败。
因此该实现不能被宣传为 Go TLS 的全行为等价实现。

## 本地验证

将构建产物放在批准的临时目录：

```powershell
cargo test --locked --manifest-path crates/cpa-tls-profile/Cargo.toml --target-dir C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-rama-prototype-20261021/target
cargo fmt --manifest-path crates/cpa-tls-profile/Cargo.toml -- --check
```

2026-10-21 的端到端探针和脱敏对照位于
`linshi/gateway-cpa-rama-prototype-20261021`：`compare.py`、`comparison.json`、
`live-comparison.json`。真实握手只进行 TLS 协商，没有发送 API 凭证或付费模型请求。
首个默认库样本不匹配；应用本库配置后的离线样本与真实握手样本均匹配。

本轮检查：library 单测通过（一个测试内重复三次），`cargo fmt --check`、
仓库有效行数检查器的 31 项测试、ratchet 和 `git diff --check` 通过。
`lib.rs` 为 43 有效行，`tests.rs` 为 92 有效行。
Clippy 未运行成功：当前 `1.98.0-x86_64-pc-windows-msvc` 工具链未安装
`cargo-clippy.exe`。未运行 Gateway 整体构建、打包或端到端模型调用，
因为这个 library 尚未成为 Gateway 运行时依赖。
