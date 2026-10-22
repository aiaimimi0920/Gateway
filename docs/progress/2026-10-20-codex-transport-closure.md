# Codex transport 续进核验

恢复会话 `01a1589a-383a-74c1-924c-31ffce6ab0dc` 的实际停点；不重复更改已完成的产品源码。
本次修改仅涉及验证脚本和证据说明，没有提交、推送、部署或重启真实实例。

## 已存在证据的当前有效性

证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-codex-transport-20261020/`。
本次逐文件核对 `source-manifest.json` 的 1840 个 Rust 构建输入，无 hash 差异。
清单聚合 SHA256：`e2c5564817522eb8f4bb330e7e418075d1d6eb2a5eebccda5d841667670b4976`。
本次测试开始时 `target/debug/gateway.exe` SHA256 为
`2de35af786530862652d7141972171265964be62361d30f2f291d9ff2424dff5`，
与 `https-after/http-run-receipt.json` 一致。以下 HTTPS 成功证据是复核旧结果，不是本次重跑。
随后 integration test 构建重生成 debug EXE，其 SHA256 为
`da6886f4e20bdc280f2a636dd0fc4f0767826abcd0b7ba5930479c751765e3ae`。
未修改 Rust 源码，但新二进制不能冒用旧二进制的 HTTPS 抓包身份。

- `https-after/https-verification.json`：真实 CLI 主请求与续聊均成功，
  同一输入经 Gateway 回放后完整 HTTP 字节相等；TLS1.2、cipher、ALPN 一致。
- `https-sequences-v2/verification.json`：成功、429、500、503、401、响应前断开、
  SSE 缺 completion、SSE 中途断开，共八类场景的上游请求数量、状态序列、连接数量和 CLI
  退出码一致。请求时间间隔并不相同，不宣称时序字节级相等。
- 上轮已将 native client pool 从全局共享收窄到 accepted connection 生命周期，
  保留 ConnectInfo；连接内共享、连接间独立、最后 owner 释放的回归测试已存在。
- `https-sequences-v2-trust-receipt.json`：测试 CA 已移除，CurrentUser Root 前后均 86 项，
  原记录确认 storeRestored。本次只读核对唯一 thumbprint 仍不存在，没有修改信任库。

## 本次实际补验与脚本纠错

旧脚本要求 TLS 接收端出现 `UNKNOWN_CA`，两次重跑均失败，失败产物保留在
`untrusted-resume-final/` 和 `untrusted-resume-settled/`，没有改写为成功。
原因是该断言不适用于当前 Schannel 路径：接收端可以记录握手完成，但客户端随后本地
证书链校验失败、不给接收端发送 HTTP。依赖源码 `schannel-0.1.29/src/tls_stream.rs`
的初始化状态机和 validate 函数将证书链策略校验作为独立步骤；服务端握手成功不等于
客户端接受该证书，更不保证发送 UNKNOWN_CA 告警。

调整脚本后实际运行 `untrusted_probe.py untrusted-verified`，exit 0。
`untrusted-chain-check.json` 使用 Windows X509Chain，将测试 CA 仅放入内存 ExtraStore，
结果为 `valid=false`、`UntrustedRoot`，且测试 CA 不在 CurrentUser Root。
`untrusted-verified/verification.json` 同时确认两条链路到达 TLS 接收端，但 HTTP 请求数为 0。
CLI 直发停留在网络恢复，8 秒后由 harness 终止；经 Gateway 的 CLI exit 1。
这是共同拒绝发送应用请求的证据，**不是错误反馈、重试次数或退出时刻完全一致的证明**。
脚本超时不算 CLI 自然退出，也不算上游业务成功。
integration test 重建 debug EXE 后再次执行 `untrusted_probe.py untrusted-final-binary`，
exit 0，仍为零 HTTP 请求、直发有界停止、经 Gateway exit 1。

## 验收边界

本次重新执行：native 相关库测试 49 passed；`protocol::codex_wire::tests` 2 passed；
`codex_native_fidelity_contract` 4 passed；effective-lines checker 31 passed；
ratchet 通过，3070 文件，受治理源码超过 700 有效行的数量为 0。
未调整 baseline、policy 或 exception，未运行全量测试或发布打包。
`cargo fmt --all -- --check` 和 `git diff --check` 均 exit 0。

当前范围为 Windows Codex CLI 0.155.1、Direct HTTP Responses、合成凭证和回环端点。
完整 HTTPS 和上述八类序列已补齐，不能继续描述成只测了 ClientHello。
但没有真实服务商端识别回执；证书失败路径仍存在恢复/反馈差异。
WebSocket、自定义 CA 的 Rustls 分支、其他 OS/版本、任意辅助 API、IP/时序等侧信号
仍未被这些证据覆盖。因此不能宣称所有场景完全一致或上游绝对无法识别 Gateway。
