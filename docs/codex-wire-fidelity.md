# Codex 网络保真

目标是消除 Gateway 自身引入的、原生 Codex 请求不具有的差异，而不是只复制 User-Agent。
参考客户端为本机实际安装的 Codex CLI 0.155.1，Windows；其他版本、平台和真实服务商的
风控接受情况不能从这一组本机证据自动推出。

## 适用路径

入口 `/v1/responses`，服务商 adapter 为 `openai_compatible`、Direct HTTP，
选择 `codex_cli` 或对非 Claude 模型生效的 `auto`，目标仍为 Responses。
协议桥接、需要强制 streaming 聚合的非流式请求、产品专用通道与浏览器 transport
不进入此分支；已流式的原生 Responses 不因 OAuth preset 只有 responses_path 而被排除。
现有认证、过滤、路由、预算、重试、熔断、并发许可和审计阶段仍先于或包围发送阶段执行。

Gateway 使用服务商配置指定的目标和凭证，绝不复制入口的 Host、Authorization、Cookie
或内部控制头。与原生直发比对时，必须使用相同的目标、上游凭证和实际请求。
显式模型映射、协议转换、工具注入和配置缺省值本来就会改变请求；不能一边要求改变这些
业务参数，一边宣称修改后的整个请求与修改前逐字节相同。

## HTTP 保留规则

- 入口以受现有 body limit 约束的 Bytes 读取 JSON；拒绝重复对象键，防止入口路由与
  上游解析器对重复 `model`、role 等字段作不同解释。
- 独立保存原始字节，不把内部 marker 塞入可外发 JSON。只有最终 RequestPlan 的 JSON
  与原始 JSON 完全相等时才复用原字节；否则发送正常生成计划，不能绕过业务修改。
- 空 `tools: []` 不再被当作“已发生 XML 工具注入”，不会因此丢弃 developer、ID、type、
  client_metadata、prompt_cache_key 或未知字段。真实工具注入仍使用既有转换路径。
- 保留原始 query 的重复键、顺序和转义形式，长度上限 8192 bytes。
- 固定允许转发软件身份、Accept、session/thread/request/window/turn state、beta 和 turn metadata；
  禁止任意头 wildcard 转发。普通值限 1024 bytes，turn metadata 限 16384 bytes，要求可见 ASCII。
- 保留客户端显式提供的 `x-openai-subagent` 和 `x-openai-internal-codex-residency`，
  不为缺失值生成默认身份或驻留要求。两者使用普通值边界，服务商显式配置仍优先；
  名称中的 `internal` 属于上游协议，不表示 Gateway 管理权限，不放开其他内部头。
- 同样保留 `x-codex-parent-thread-id`、`x-codex-routing-hint`、`x-openai-memgen-request`；
  这些原生关联/路由/请求类型字段不解释成 Gateway 内部指令，也不会由 Gateway 伪造。
- 成对且有效的原生 Codex UA/originator 优先；任意设备 UA 不会因此得到透传授权。
  保留原生已知头的顺序，由 HTTP 库为选定目标和实际 body 生成 Host/Content-Length。
- 不向原生通道注入 Chrome 的 sec-ch-ua、sec-fetch、HTML Accept、Accept-Encoding 等默认头。
  不凭空生成独立 Version 头。Gateway 自己生成的 x-request-id 留在本地日志/响应；
  客户端显式提供的合法 trace 和服务商显式配置仍按原有权限边界处理。
- buffered 和 SSE 使用同一 native sender；SSE 仍保留 completion 校验、背压和取消清理。
  native 网络错误保留自己的类型，不伪装为另一 HTTP 客户端的错误；诊断先移除 URL。

## TLS 版本依据

普通 HTTP 的默认系统信任路径使用 `reqwest 0.12.28` 与系统 native TLS；
不开启 native-tls-alpn、自动压缩或浏览器仿真。
Windows 上为匹配本次参考版本，局部设置 TLS1.2 上限，不改变其他 transport，
不重新启用 TLS1.0/1.1，也不关闭证书或域名验证。
Linux native-tls 需要系统 OpenSSL；Docker 构建/开发镜像显式安装 `libssl-dev`，
Bookworm 运行镜像显式安装 `libssl3`，不依赖其他包偶然带入该依赖。

这不是依据单次抓包盲目关闭 TLS1.3：
[Codex 0.155.1 的锁文件](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/Cargo.lock)
锁定 native-tls 0.2.14；该版本的
[Windows Schannel 实现](https://github.com/sfackler/rust-native-tls/blob/v0.2.14/src/imp/schannel.rs)
可用协议列表最高为 TLS1.2。Gateway 保留维护中的 native-tls 版本，以局部配置复现这个上限，
而非全局降级依赖。未来更新 Codex 参考版本必须重测，不把这个上限理解为永久不变的 CLI 行为。

HTTP 自定义 CA 路径使用显式 Rustls；WS 无论是否自定义 CA 均使用显式 Rustls，
不套用上述 Schannel/TLS1.2 上限。WS 使用固定 CLI fork 完成握手与压缩协商，
通过独立连接器保留系统代理、NO_PROXY、HTTP CONNECT 和 HTTPS proxy 双层 TLS。
这些修复及实际验证范围见下述 WS feature parity 记录，不从 HTTP 历史回执推断。

## 验证与不可外推的范围

受控验证使用独立 HOME、合成凭证、仅允许注册回环端点的代理，以及隔离 Gateway 数据目录；
不会触及真实账号、实例、驱动或防火墙。后续完整 HTTPS 测试经用户明确授权，
短暂在 CurrentUser Root 安装唯一测试 CA，并在 finally 中删除该证书、核对信任库恢复；
详见后续 transport 记录。本次续进没有再次修改信任库。
逐字节 HTTP 比对覆盖真实 exec 主请求、续聊和 TUI 请求；query 单独验证。
TLS ClientHello 比对必须排除协议随机值，比较 cipher、extension 顺序及内容、SNI、ALPN、
session ID 长度与 record 长度，而不只看一个 JA3 hash。

早期 ClientHello 接收端会主动断开，该实验本身不证明完整 HTTPS；后续已单独完成
完整 HTTPS 主请求/续聊验证和八类成功/异常序列对比，见 transport 记录。
本轮没有真实服务商端的识别/接受回执。不同连接的随机数、密钥、源端口、TCP 序号、
时序及密文字节本来不同，Gateway 也不能保证源公网 IP 或服务商的所有侧信号相同。
上述 Schannel 证据仅对应 HTTP 默认系统信任路径；Codex 的 `CODEX_CA_CERTIFICATE` / `SSL_CERT_FILE`
[自定义 CA 分支](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/http-client/src/custom_ca.rs)
会切换到 Rustls，不能借用本轮 Schannel 证据。受信任测试 CA 仅用于受控测试，
不是关闭证书验证，也不证明真实服务商接受情况。
普通非 Codex 调用者没有原生 Codex 的全部 prompts/tools/session metadata；
套件不会伪造这些业务状态，所以也不对这种输入承诺完整网络字节相同。
不同 OS/CLI 版本、辅助 API、模型/工具转换及站点额外识别策略均不在这组 HTTP 逐字节证明内。
新增 compact 与串行 Responses WebSocket 的实现、独立验证和传输边界见
`docs/progress/2026-10-20-codex-application-protocol.md`。HTTP/TLS 历史证据不自动
覆盖 WS；尤其不能把 JSON 事件保真理解为握手、压缩、分片或连接时序完全一致。
后续 WS 默认 TLS、Upgrade 请求头和实际 permessage-deflate 修复见
`docs/progress/2026-10-20-codex-ws-feature-parity.md`；该记录更新此前压缩未支持的状态。

复测证据与门禁记录见 `docs/progress/2026-10-20-codex-wire-fidelity.md`。
后续原生可选元数据修复见 `docs/progress/2026-10-20-codex-optional-metadata.md`；
此前完整抓包及二进制 hash 不自动覆盖该后续源码改动。
后续实际 CLI 双链路重抓包见 `docs/progress/2026-10-20-codex-live-comparison.md`。
完整 HTTPS、连接池边界和证书拒绝补验见
`docs/progress/2026-10-20-codex-transport-closure.md`。
后续指定 OAuth 账号真实五轮对照、原生 preset/body 修复和成功 SSE 状态头透传见
`docs/progress/2026-10-21-codex-oauth-five-turn.md`。该对照仍明确保留 Cookie、辅助 API
及未测真实 WS 等差异，不构成服务商不可识别承诺。
后续 Cookie 会话与真实 settings GET 的可选兼容模式、指定凭证新一轮 5＋5 对照见
`docs/progress/2026-10-21-codex-native-session.md`。该模式默认关闭，仅限独立 owner key
和明确的官方 OAuth 候选；普通 API-key 模式不会因此伪造账号设置调用。
