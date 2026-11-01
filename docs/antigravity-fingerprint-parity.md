# Antigravity 原生指纹对照

## 对照对象与方法

2026-11-01 使用本机已安装、未修改的原生二进制：

- `C:/Users/vmjcv/AppData/Local/Programs/Antigravity/resources/bin/language_server.exe`
- hub 2.15.0，Windows/amd64，CL 983516863。
- App EXE SHA256：`A96922E26A71C79F17CA8F3BD40E27F5118A9C56EDDF15C716323E182A89D2BB`。
- language server SHA256：`3F1D5996FE3AF7C2556DB7F580B76B4231857A5B20D53E90B2C4135F75A297A3`。

TLS 对照保留真实官方 SNI `daily-cloudcode-pa.googleapis.com`，通过本地 CONNECT
接收首条 TLS record 后关闭连接，不完成握手、不转发到 Google、不发送应用层凭据。
原生八个样本结构一致；Gateway 从公开的 `upstream::antigravity::probe` 进入真实发送 owner，
不能用另一个手写演示客户端代替产品出站。

HTTP 对照通过原生支持的 `cloud_code_endpoint` 指向本地替身。使用独立测试 home、
测试项目及模型目录，调用正常的 `StartCascade` / `SendUserCascadeMessage`，实际观察到
checkpoint 和 agent 的 `streamGenerateContent?alt=sse` 请求。
之前因替身模型目录不完整返回的错误，不代表原生平台不支持生成。

采样观察到非空 Authorization，因此不声称独立 home 能排除所有系统凭据来源。
捕获处理器只在内存处理必要头值，落盘前即去除 Authorization/Cookie 原值；
不读取或修改日常客户端 credential store，不导出登录状态。
所有本轮启动的 language server 子进程在测试 finally 中等待退出；
进程被主动终止的 exit 1 不是一次真实 Google 生成失败。

## 实测差异与修复

| 项目 | 修复前 Gateway | 本次原生样本及修复目标 |
| --- | --- | --- |
| UA | `antigravity/2.0.3 windows/amd64` | `antigravity/hub/2.15.0 (aidev_client; os_type=windows; arch=amd64; cl=983516863)` |
| TLS JA3 MD5 | `6aac88aefeb1c3f8b576782dbe085b6e` | `9b7dcdf3f997f1fb7b4409c94cb7ef36` |
| ALPN | 存在 | 不广告 ALPN；原生使用 HTTP/1.1 |
| TLS 扩展 | 默认 BoringSSL/wreq 集合 | `0,11,65281,23,18,5,10,13,50,43,51` |
| Accept | `application/json, text/event-stream` | 不发送 |
| 自动 gzip | 普通头排序 | 自动 `Accept-Encoding: gzip` 位于普通头之后 |
| 生成请求 | 依下游模式选 JSON/SSE，固定 Content-Length | 统一 SSE 上游，Transfer-Encoding: chunked |

完整首航结构见 `tests/fixtures/antigravity/native-hub-2.15.0-client-hello.json`；
HTTP 元数据见 `tests/fixtures/antigravity/native-hub-2.15.0-http.json`。
比较 cipher 顺序、全部扩展顺序与稳定内容、签名列表、supported groups/versions、
key share 分组和长度、record 大小及版本、compression 和 session ID 长度，
不是仅比较 JA3。Client random、session ID 值、临时公钥必须每次重新生成，不能复制。

原生 HTTP/1.1 头顺序为：

```text
Host
User-Agent
Content-Length                 # 目录请求
Authorization
Content-Type
Accept-Encoding
```

生成时第三个头改为 `Transfer-Encoding: chunked`，没有 Content-Length；
正文为一个 JSON chunk，随后是正常终止 chunk。
原生 agent 外层包含 `project/requestId/model/userAgent/requestType/request`；
requestId 形状为 `agent/<uuid>/<unix_ms>/<trajectory_uuid>/1`。
Gateway 对应字段保持自有可信 owner，不能由请求 extras 覆盖。

## 实现与安全边界

复用仓库现有 native IO 的连接、证书验证、代理 CONNECT、取消与有界响应能力，
但增加不可变、实例级 `AntigravityHub215` profile；默认 CPA 构造及其 TLS/HTTP2 配置不变。
Antigravity 拥有自己的初始化单例和池；失败时不回退 Chrome、wreq 默认指纹或浏览器。
未启用 `cpa-rust-transport` 的构建明确返回配置错误，而不是偷偷更换指纹。

系统信任仍启用，域名验证不关闭。可在进程启动时通过
`GATEWAY_ANTIGRAVITY_CA_CERTIFICATE` 添加最多 1 MiB PEM 私有 CA；
它不会读取 CPA 的 CA/test-HTTP 环境设置，不支持请求控制 CA 或关闭验证。
生产仍只允许官方 Cloud Code HTTPS origin，没有新增 HTTP 测试环境开关。
代理来源和明确空 NO_PROXY 规则复用 Gateway 的既有默认策略，
不修改进程环境、注册表、系统代理或 OS 根证书。

## 不宣称完全相同的部分

- **完整 TLS 行为**：本次验证的是新连接首航。HRR、会话恢复、所有服务端签名与证书
  算法、应用数据 TLS record 分片、错误及关闭时序，不因 ClientHello 相等而自动成立。
- **ML-DSA**：保留原生广告的 `2308/2309/2310` 编号，不代表向底层新增了对应算法实现；
  不能据此承诺 ML-DSA-only 服务端与原生客户端同等兼容。
- **应用内容**：Gateway 转换调用方的 Canonical 请求，不复制原生 agent 的完整系统提示、
  内置工具、checkpoint 工作流或 labels，不伪造设备/遥测身份。
  JSON 空格、字段顺序、正文大小、随机 ID 和内容不是逐字节一致性承诺。
- **真实提供方验收**：本轮替身测试不提交 Google 生成，不证明真实 OAuth、刷新、额度、
  所有模型及多轮工具均通过；模板不能因此标蓝。
- **版本范围**：固定本次实测 Windows hub 2.15.0，不代表所有历史/未来版本或其他平台。

## 定向验证入口

```powershell
cargo test --locked --offline --jobs 4 --test antigravity_fingerprint_contract
cargo test --manifest-path crates/cpa-http-transport/Cargo.toml --target-dir target --locked --offline --jobs 4
cargo test --locked --offline --jobs 4 --test antigravity_contract --test console_antigravity_oauth_contract --test cpa_profile_contract --test cpa_wire_contract
```

`antigravity_http_fingerprint_contract` 是显式 opt-in 的本地 HTTPS 测试。
它需要专门生成的短期测试 CA、leaf DER 和 PKCS8 key，绝不安装 OS 根信任。
设置 `ANTIGRAVITY_WIRE_FIXTURES` 指向该隔离目录，
分别以 `ANTIGRAVITY_WIRE_MODE=trusted|untrusted|hostname` 在独立测试进程执行：

```powershell
cargo test --locked --offline --jobs 4 --test antigravity_http_fingerprint_contract -- --ignored
```

trusted 场景验证真实生产 owner 的两次目录请求、下游非流式和流式生成，
并检查复用一条连接、HTTP 头字节形状、framing、JSON 及完整 SSE 解码。
其他两种场景必须拒绝不受信任/域名不符证书，且不能发送应用层请求。
实际结果及证据位置记录于
[本轮 progress 文档](progress/2026-11-01-antigravity-fingerprint-parity.md)。
