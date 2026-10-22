# CPA Rust 传输：运行路径、并发、HRR 核验

接续 `cpa-rust-hpack-20261021.md`。状态：继续完成若干关闭项，但没有达到可删除
Go worker 的条件。特别是额外 TLS 组已从“待验证”变为“确实不支持”。

## 本轮代码修复

1. `crates/cpa-http-transport/src/initialization.rs`：单一、可共享的阻塞初始化作业。
   Gateway 的总 deadline 覆盖初始化等待，发送只使用剩余时间。取消等待不会重复
   启动初始化；成功和配置失败都缓存到进程结束，避免初始化风暴。
2. `body_limit.rs`：保留跨 IO/rquest 适配器的 typed body-limit error，Gateway 将它
   映射为既有 `upstream_body_too_large`，保留 provider，不靠错误字符串猜测类型。
3. `pool.rs`：同 origin 的慢 HTTP/1 正文不再串行阻塞独立请求。空闲槽位在索引锁内
   预留，支持多连接；保留 64 origin、128 总连接槽和 128 活跃请求的资源上限。
   这些安全上限不是 Go 默认无限活动连接/默认空闲池策略的完全复制。
4. 路由文档 `auth_mode` 之前反序列化时被忽略。现以可选字段/别名 `authMode` 接入，
   传递给 preset override 和非 preset payload，凭证继承 provider；缺失时保持旧行为。
   认证选择仍与 `request_client_profile` 独立，不因 CPA 风格强制修改认证。

Gateway 单测验证配置序列化回读及两种编译分支的凭证继承；补齐旧测试中的显式
struct 初始化。Rust transport 13 项测试通过；另执行原有 CPA IPC/私有头 2 项及
profile 选择 1 项测试通过。未将 3575 项 Gateway 全量测试宣称为全部通过。

## 实际运行证据

根目录 `cargo build --bin gateway --features cpa-rust-transport` 已成功构建并启动隔离实例。
隔离管理 API 导入合成凭证，真实 CPA 参考程序与 Gateway Rust 路径各执行五轮，
均为 200、正确 SSE 正文、每组仅一条上游连接。比较分别覆盖 CPA 默认模式和
`claude-code-cli` 模式；不能混为同一种基线。

证据目录前缀：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-rama-http-20261021/`

- `gateway-e2e-20261021-180450`：CPA 默认模式与 Gateway 的首次产品路径对照。
- `gateway-e2e-20261021-180604`：CPA CLI 模式，对照发现 auth_mode 被忽略。
- `gateway-e2e-20261021-183016`：修复后的新构建复测，report 记录二进制 SHA-256。
  两边各五轮 200、SSE 正确、单连接复用；Bearer/x-api-key 差异已消失。归一化
  隔离 Host 后仍差 `anthropic-beta`、`x-claude-code-session-id`、Content-Length，
  后者对应实际正文身份/缓存注入差异，不是被忽略的随机值。

### HTTP/2 生命周期

`h2-cancel-report.json`、`h2-cancel-capture.json`：先等待响应头超时，再丢弃未结束
正文，最后显式发送新请求。3 请求共用 1 连接，观察到 2 个 RST_STREAM，最后请求
成功；释放客户端后 socket 关闭。未新增自动请求重放。

### TLS HelloRetryRequest

`hrr-report.json`：TLS 1.3 服务端分别只允许 P-256/P-384。TCP 中间切片没有终止
TLS，实际捕获 HRR 固定标识及两次 ClientHello。Go/Rust 各五轮均成功；两次
ClientHello 的已解析非随机字段在每种组配置下均一致。

`extra-groups-report.json`：用 Go TLS 服务端强制组 4587 或 4589：

| 组 | Go 客户端 | Rust 客户端 |
|---|---|---|
| 4587 | 成功，2 次 ClientHello | HRR 后失败，只有 1 次 ClientHello |
| 4589 | 成功，2 次 ClientHello | HRR 后失败，只有 1 次 ClientHello |

因此 UnknownValues::Preserve 仅能保留广告，不代表底层实现对应密钥交换。
不能把首次 ClientHello 一致当作完整 TLS 等价，也不能删除 Go worker。

### 真实服务商

`live-e2e-20261021-182251`：已授权的真实 CPA 首个请求返回 403，业务消息为：
“当前分组本时段不可调用，可调用时段为：09:00~18:00”。已停止后续调用；未轮换
凭证、未绕过服务商限制。真实服务商五轮成功验收仍未完成，不归因于 Gateway。

## 保留的边界

- 额外 TLS 组 4587/4589 的真实协商支持缺失，需底层实现而非广告补丁。
- HTTPS/SOCKS 代理已完成下述 transport crate 隔离验证；官方域名不同传输线路仍未完成。
  HTTP 明文源站经过 HTTP(S) forward proxy 仍明确拒绝，不错误发送 origin-form。
- CPA 完整 CLI 模式仍有 beta/session/正文身份和缓存注入差异；默认 CPA API Key
  模式本来保留调用方身份，不能把两种基线混用后宣称“网络一致”。
- 普通头随机顺序和 ACK 调度分布未证明等价；池安全上限保留。
- 真实服务商验证受调用时段限制，未在限制期间继续试调用。

没有提交、推送、部署、修改日常配置、删除 Go worker 或关闭 TLS 校验。

最终门禁：transport `--locked` 13 项测试、auth_mode 单测、原有 CPA/profile 3 项、
formatter、31 项有效行数 checker、ratchet 和 diff check 通过。新构建成功，保持
24 个现有库 warnings。相关 transport 手写文件最大 215 有效行。没有执行完整
Gateway 全套测试、发布包或正式服务升级。

## 续进：HTTPS / SOCKS 代理

`dial.rs` 接入独立 `proxy.rs`：HTTPS 代理先验证代理证书，再 CONNECT，再独立验证
源站证书；SOCKS5/SOCKS5h 在代理侧解析域名，支持无认证及用户名密码认证。
代理认证仅用于握手，不进入源站请求。握手受原有总 deadline/取消控制；CONNECT
响应头上限 16 KiB，SOCKS 字段按协议有界读取。不新增重试、降级直连或证书跳过。

实际先复现 HTTPS/SOCKS 的 `UnsupportedProxy`，接线后重编生产 crate 的 `http_probe`：

| 代理配置 | Go 请求成功 | Rust 请求成功 | 各自连接数 | 代理认证泄漏 |
|---|---:|---:|---:|---|
| HTTP + Basic | 5/5 | 5/5 | 1 | 无 |
| HTTPS + Basic | 5/5 | 5/5 | 1 | 无 |
| SOCKS5 无认证 | 5/5 | 5/5 | 1 | 无 |
| SOCKS5h 用户密码 | 5/5 | 5/5 | 1 | 无 |

证据仍在上述 linshi 目录：`proxy-report.json`、`proxy-*-origin.json`、
`proxy-field-comparison.json`。归一化本地动态端口后，四种代理的握手内容相同；
五轮源站请求头的名称和值相同（比较时排序，不据此证明普通头顺序或 HPACK 原始
字节相同）。这些是直接使用生产传输 crate 的 socket 测试，不是本轮新 Gateway EXE
整链路或真实服务商验收；前述 EXE SHA 不能代表这次新增代理实现。

`proxy-negative-report.json` 实际验证 HTTP/SOCKS 错误密码、HTTPS 不可信证书、
已信任但主机名不匹配的代理证书，全部失败关闭，源站零请求，错误不含凭证。
另新增四项仓库测试覆盖状态行、16 KiB 边界、截断响应、坏 SOCKS 方法/认证、
凭证长度及取消后的 socket EOF。transport `--locked` 共 17/17 通过；有效行数
checker 31/31、ratchet、scoped rustfmt、diff check 通过。

本轮相关有效行数：dial 100、proxy 119、proxy_tests 101、lib 109，均小于 250。
尚未重跑完整 Gateway 测试、重新构建/打包 EXE；TLS 4587/4589、完整 CPA CLI
身份模式及官方域名线路的缺口不因本次代理通过而关闭，Go worker 继续保留。
