# Claude 出站 TLS 实际对照与修复

## 测试边界

沿用用户授权的 ai.hybgzs.com、claude-opus-5 与同一私有 API key，未改日常实例。
证据位于 linshi/gateway-claude-wire-20261021 下的 tls-*、no-proxy-hello-01 目录。
该目录含私有配置，禁止整体发布。抓包只输出公开 ClientHello 字段和 TLS record 元数据，
不写 TLS 会话密钥，不输出凭证，不安装 CA，不关闭证书/主机名验证。

本次改为受限 CONNECT 字节隧道：只准连接授权域名的 443 端口，转发客户端原始 TLS 字节，
没有 TLS 解密或代建 TLS。收/发摘要验证转发字节未变。这与上轮 HTTP reverse tap 不同，
能够观察 Gateway 自身 ClientHello；但隧道改变 TCP 连接/分包，因此不证明 TCP 指纹一致。
原生 PktMon status/filter 查询被系统拒绝访问，未启动系统抓包，未修改全局过滤器。

## 修复前证据

tls-direct-01/02 两次真实 CLI 请求成功，去除临时随机值后的 ClientHello 完全相同。
tls-gateway-01 也调用成功，但有以下确定差异：

| 项目 | Claude Code 2.1.278 | 修复前 Gateway |
| --- | --- | --- |
| ALPN | http/1.1 | h2, http/1.1 |
| Cipher suites | 17 项 | 28 项 |
| Supported groups | 4588,29,23,24 | 4588,65074,29,23,24 |
| OCSP / SCT | 有 | 无 |
| ClientHello record 长度 | 1495 | 1509 |

所以旧代码不能认定为服务商无法区分。另以合成凭证在无代理回环端点捕获 CLI ClientHello：
除 SNI 及相应长度外，与代理真实调用样本相同，确认 HTTP/1.1 特征并非只由代理模式引入。

## 实现

新增 src/http_client/native_claude.rs，使用现有 wreq/BoringSSL 能力配置参考套件顺序、
supported groups、OCSP、SCT 与 HTTP/1.1。独立客户端/连接池只供精确匹配已测参考 UA 的
Claude 原生流式 Messages 路径使用；其它版本仍走原 plain client，不改变 Codex、浏览器
通道或其它服务商的 HTTP/2 设置。证书校验、代理策略、出站超时和禁止重定向规则保持。
没有修改 Cargo.toml、Cargo.lock 或新增依赖。

## 修复后真实结果

- tls-gateway-fixed-01/02：两次真实 CLI 经 Gateway 调用成功；规范化 ClientHello 与
  tls-direct-01 完全一致，包含扩展顺序、内容、套件顺序、key share 结构和 record 长度。
- tls-dsh-fixed-01：从实际 DSH 网页发送一轮，200 且有 message_stop；实际出站
  ClientHello 与同一 CLI 基线相同。截图为该目录 tls-ui.jpg；原 Crow 会话/DeepSeek 已恢复。
- tls-record-direct-01 / tls-record-gateway-01：均成功，客户端 record 类型/数量一致，
  最后 record 长度相差 1 字节；测试工作目录名也相差 1 字符，因此追加等长目录对照。
- tls-record-directx-02 / tls-record-gateway-02：ClientHello 与客户端 record 序列完全
  相同，长度依次为 1495、1、53、9310；Gateway 这次收到服务商 429（上游负载饱和），
  因此不是成功样本，失败记录保留。
- 冷却后的 tls-record-gateway-03 调用成功；与 tls-record-directx-02 的规范化
  ClientHello、客户端 TLS record 类型、版本和长度序列完全一致，长度为 1495、1、53、9310。
- http-after-tls-01：最终代码再次真实调用成功（200、message_stop）。请求体字节一致，
  头名称、大小写及顺序一致，会话标识一致；缺失头、额外头、静态值变化均为空。

规范化排除 ClientHello random、session ID 的随机字节、临时 key share 字节；保留其
长度、类型和顺序。不是只比较一个 JA3 hash，也不把服务器生成的回复长度当成客户端差异。

## 软件与安全验证

最终编译成功。local-tls-01 六组回环测试通过，保留应用层字节/头序、会话别名、认证边界
与重定向保护。tls-trust-03：自签名回环端点在仅控制进程显式信任测试 CA 时可完成 TLS；
Gateway 不信任该 CA，连接失败且没有发出 HTTP 凭证/请求体，未关闭验证来达成指纹匹配。
早期 trust-01/02 测试脚本分别遗漏 Windows RST 和过度期待公开错误包含证书原文，已修正
为控制端点可用性与 HTTP 字节拒绝的断言；产品对外错误仍保留泛化 Connect 信息。

定向 Rust 检查通过：Claude 库测试 8、wire 合同 6、profile 筛选 1；另跑完整 profile
合同 8、native 回归 86、Anthropic 回归 90，均无失败。这些测试集合存在重叠，不相加
宣称独立测试总数。有效行 checker 31 通过，ratchet、cargo fmt 和 git diff --check 通过。

生产文件有效行变化：http_client.rs 16→18、upstream/client.rs 421→422、
client_initialization.rs 45→48、upstream/client/native_claude.rs 145→149；
新增 http_client/native_claude.rs 为 48 行。未扩展历史超限文件。

## 不得扩大结论

本轮已修复可观察的初始 TLS 握手差异，但不是“完整网络指纹相同”的证明。未覆盖原生
TCP/IP 指纹、全部连接复用/会话恢复/重试/取消序列、所有业务长度、其它 CLI 版本或平台。
DSH 未发送会话标识以及 beta 能力差异仍沿用上一份 HTTP 对照记录；会话 ID 应由调用端
正确提供，不能按消息内容猜测或让所有调用共享伪会话。未启用 fallback-credit 等未授权能力。
用户已明确本轮只修改 Gateway：不修改 DSH 调用端，保留并报告缺少稳定会话 ID 的差异。
没有正式部署、提交、推送。需要完整 TCP 对照时，先提供可访问 PktMon 的管理员执行环境。
