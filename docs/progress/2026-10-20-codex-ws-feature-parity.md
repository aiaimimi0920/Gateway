# Codex WS 可观察特征差异修复

用户要求继续检查从中转站视角可观察的差异并修复。不是承诺规避任意服务商识别，
也不能把语义正确等同于网络层不可区分。参考仍是本机 Windows Codex CLI 0.155.1。

## 证据与修改

1. 历史 `live-wss-final/captures.json` 显示 Gateway 的 Upgrade GET 比真实 CLI
   多出 `Content-Type: application/json` 和 `Accept: text/event-stream`，且 Host 和
   Upgrade 控制头位置不同。现在删除未显式配置/提供的生成请求默认头，按 RFC 6455
   参考顺序构造控制头；Host 来自选中 upstream URL，nonce 每条新连接独立生成。
   显式 provider 业务头仍优先，不复制下游 Host、认证信息或任意扩展声明。
2. 官方固定版本的 WS 使用显式 Rustls，即使没有自定义 CA；不是 HTTP 的 Windows
   Schannel 分支。修复前默认 WS ClientHello 对比已观察到不相等。现在 WS 默认与
   custom CA 路径均使用对应 Rustls roots/provider 策略，普通 HTTP 保持原路径。
   不修改系统信任库、不忽略域名或证书验证、不将入口 JSON 当作 CA 配置。
3. 真实 CLI 提供 `permessage-deflate; client_max_window_bits`，此前 Gateway 不提供。
   现在 native WS 独立使用官方固定版本的两个 fork，实现实际压缩/解压及 context
   takeover，不只是补一个声明头。其他协议继续使用原 tokio-tungstenite 0.24。
   完整握手与 frame codec 均使用相同 fork；reqwest 仅用于构造已过滤的请求，不执行
   WS 请求，避免其在 execute 阶段自动添加 `Accept: */*`。真实回放还发现 fork writer
   用 HeaderMap swap-remove 提取控制头会改变剩余业务头顺序；因此内部构造时将
   transport 控制头放在末尾，再由 writer 按 CLI 的固定前缀输出，避免业务头被置换。
4. 扩展协商由固定 fork 负责，不再维护第二套协商解析器；只在协商成功后启用压缩。
   帧和解压后累计消息均限制为16 MiB，未协商 RSV1、解压超限会失败，不绕过计费结算。
5. 官方 WS dialer 默认不启用 TCP_NODELAY，而 reqwest 默认启用。WS 专属 connector
   显式设置 `set_nodelay(false)`，普通 HTTP 保持原配置。该项为源码配置对齐，不冒充
   已证明所有 TCP 分包/延迟相等。

## 固定来源

- Codex commit: `be2951ea34f0d295ed0becf97079f92fa5f6950e` (`rust-v0.155.1`)。
- `openai-oss-forks/tokio-tungstenite`: `0e5b2d73aa18dd9f0a50ee9ff199d5aef7594186`。
- `openai-oss-forks/tungstenite-rs`: `4fffad30fe373adbdcffab9545e9e9bf4f2fc19f`。
- 核对 `websocket-client/src/dialer.rs`、`codex-api/src/endpoint/responses_websocket.rs`
  与 fork 的 `extensions/compression/deflate/config.rs`，均已联网获取固定版本。

## 验证边界

所有回环实验使用隔离 HOME/数据和合成 key。默认 TLS probe 只获取首次 ClientHello，
随后主动中断并有界终止 CLI；其错误/超时退出是实验设计，不代表完整 HTTPS 成功。
完整 WSS 与压缩测试另用进程专属 CA，不改系统信任库。

前一轮 `2026-10-20-codex-application-protocol.md` 中“不支持压缩”的描述是历史状态，
由本记录的实际压缩验证取代。前一轮二进制 hash 不代表本轮源码。

不承诺消除出口 IP、两段 TCP 生命周期、调度/时序和所有服务商侧检测差异。
保留帧/message/协商大小界限与鉴权、路由、计费，不能为了外观一致而取消这些边界。

## 代理与连接边界

握手 writer 改用 CLI 固定 fork，前五个控制头的大小写与序列化不再受 reqwest 限制。
连接器继续使用 reqwest 同款 `Matcher::from_system`，保留环境/系统代理与 NO_PROXY。
HTTPS proxy 先验证代理 TLS，在其中 CONNECT，再验证目标 TLS；代理认证只用于
CONNECT，不复制到目标握手。CONNECT 响应限制 8192 bytes，不吞掉隧道后续数据。
SOCKS 默认端口为1080，socks5 本地解析、socks5h 代理解析。连接、代理TLS、隧道、
目标TLS和握手共用请求超时，取消时 drop 对应 IO，没有新增长期后台 relay task。

## 本轮验证

本轮验证于 2026-10-21 收尾，所有真实对照使用本机 CLI 0.155.1 与合成凭证。

- 集成测试 `codex_application_contract` + `codex_native_fidelity_contract`：7 passed。
- 最终源码定向 lib 回归：71 passed，3471 filtered out；覆盖 header 顺序、压缩、
  解压上限、CONNECT/NO_PROXY、原生 JSON/Responses 等本次相关路径。
- `cargo check --locked --all-targets`：通过，0 errors；保留现有无关 warnings。
- 独立 `cargo build --locked --bin gateway`：通过，0 errors，25 warnings（含链接器提示）。
  此构建不复用测试 profile 的二进制身份，真实 CLI 对照对独立构建重新执行。
- checker：31 passed；ratchet通过，3085 files，governed source >700=0；fmt/diff通过。
- plain WS 与 custom-CA WSS：CLI direct/via 均 exit 0，每路两条 create；同一输入回放
  的消息体一致，握手顺序/大小写/头值在排除随机 nonce 后一致。
- WSS deflate：全部 create 实际压缩，连续两轮 context takeover 的压缩 payload
  SHA256一致（排除随机 mask）；服务端提供 h2/http1.1 时，双方 WS ALPN 均为空。
- 默认 roots WS：首次 ClientHello 归一化后相等；该实验主动中止握手，不能作为
  默认 roots 完整 HTTPS 成功的证明。
- HTTP CONNECT 和 HTTPS proxy：真实 CLI 经 Gateway 的 WSS 均 exit 0，代理认证成功，
  origin没有收到 Proxy-Authorization，CONNECT没有收到 origin Authorization。
- 普通 HTTP custom-CA 回归：direct/repeat/via/repeat 均 exit 0，同一请求原始HTTP回放一致。
- 真实 CLI compaction V2：direct/via/resume 均 exit 0，compact body回放一致。

验证二进制：`target/debug/gateway.exe`。
SHA256：`2dc3870bea2502dd76afeea2b789fcd8eab1fc8e47a18573a6dc7ee23066c288`。
尚未提交、推送、制作发布包或部署到现有运行实例。

回执根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-codex-application-20261020`。
最终真实证据：`ws-feature-plain-built`、`ws-feature-wss-built`、
`ws-default-tls-built`、`ws-proxy-built`、`ws-feature-compact-built`。
普通HTTP回归位于相邻 `gateway-codex-transport-20261020/ws-feature-http-built`。
源码和这六组同一二进制回执的绑定见 `ws-feature-source-receipt.json`。
失败的初次握手顺序对照保留在 `ws-feature-plain-final` 与
`ws-feature-wss-before-order-fix`，没有覆盖失败证据。

边界：SOCKS代码与默认端口已复核，但本轮没有启动真实SOCKS服务做端到端验证；
系统代理使用与reqwest相同的matcher，未修改注册表或系统CA。不同 CLI 版本、
平台、provider override和长期连接行为需要各自重新取证，不从本次短对话对照推断。
