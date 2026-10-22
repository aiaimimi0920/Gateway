# WorkBuddy 三路真实调用对照

## 范围与证据

使用官方 WorkBuddy 国际版 5.8.4 的未修改内置 CLI 2.161.0，按桌面启动器的
`CLIENT_INFO_*` 设置运行。不是桌面 UI 点击测试，也不是重新实现的伪客户端。
沿用已授权的同一份服务商凭证，目标 `ai.hybgzs.com`、模型 `claude-opus-5`。
所有 Gateway 均为隔离实例，通过正常管理 API 导入凭证；未修改 DSH 或日常实例。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-workbuddy-profile-20261021`。
其中包含私有测试运行数据，不应整体提交、发布或上传；凭证与用户身份在对比报告中脱敏。

三路为：

1. 官方 WorkBuddy 内置引擎 → HTTP 字节观察点 → 服务商。
2. 同一引擎 → 入站观察点 → Gateway → 出站观察点 → 服务商。
3. 独立普通 HTTP 客户端 → 入站观察点 → Gateway WorkBuddy 套件 → 出站观察点 → 服务商。

第三路重用第一路 JSON 业务正文，但不复制 WorkBuddy 身份、会话、agent 或追踪头。
HTTP 观察器不重新序列化转发字节，服务商 TLS 证书正常校验。
由于 HTTP 观察器自身发起服务商 TLS，另用只转发原始字节的 CONNECT 隧道独立测试
客户端/Gateway 的真实 ClientHello；不能拿 HTTP 抓包证明 TLS 相同。

## 实测发现及修复

初始三路均 HTTP 200，但网络请求并不相同。`live-before-diff.json` 保留反例：

- 原生 WorkBuddy 的 conversation、agent、privacy 与 B3 关联追踪字段被入口丢弃。
  增加精确软件身份识别和 13 个字段的受限名单：单值、可打印 ASCII、每项最多 512 字节。
  重复或非法值拒绝；仅显式 WorkBuddy profile 接收这些转发字段，服务商显式配置优先。
  不扩大到任意 `x-*`，不转发入站认证、Cookie、用户/设备身份或 Gateway 内部控制字段。
- HTTP/1 头名称大小写、顺序和 Connection 与原生不同。
  按实测版本组织共有头，并使用出站连接池自己的 keep-alive 行为，不透传调用方 hop-by-hop 控制。
- Chat 消息级 `agent`、`conversationRequestId` 被规范化丢弃。
  仅在 OpenAI Chat 同协议、消息主体完整相等时保留这两个有界字符串字段。
  不恢复策略改写的消息、不覆盖模型映射、不向其他协议注入这些扩展。
- TLS 原先仍带 Chrome GREASE、ALPN、ALPS、ECH 与不同的密钥交换组。
  新增请求级 WorkBuddy TLS 配置与独立连接池分组，保留原证书校验和代理策略，拒绝重定向。
  第一次修复复测又发现强制 HTTP_11 会让库注入 ALPN 和 padding；因此改为无 ALPN 的自然
  HTTP/1.1 协商，不把这两个非随机差异排除出比较。

HTTP 最终复测 `live-direct-03`、`live-gateway-03`、`live-other-03` 均成功；
WorkBuddy 明确返回 `is_error=false`，普通客户端接收完整 SSE `[DONE]`。
`live-final-http-diff.json` 显示：

- 直连与两条 Gateway 出站的共有头名称、大小写和相对顺序相同。
- WorkBuddy 经 Gateway 的入站/出站 JSON 语义相等，不再丢失消息级扩展。
- 普通客户端经 Gateway 的出站 JSON 与直连重用的正文语义相等。
- 原生 WorkBuddy 已提供的 13 项上下文均保留；没有把缺失字段当作随机差异忽略。
- 同一次 WorkBuddy 入站/出站的动态上下文值差异列表为空。
- 三路回执均验证 `selected_provider_authorization_matches=true`；不同进程的 HMAC 盐不同，
  不能用跨进程摘要字符串不同推断凭证发生变化。

TLS 最终复测 `tls-direct-02`、`tls-gateway-03`、`tls-other-03` 均完成真实服务商调用。
`live-tls-final-diff.json` 同时对照两次直连，四份 ClientHello 的非随机结构完全相等：
套件顺序、曲线、签名算法、扩展顺序、无 ALPN、key share 长度及 247 字节记录长度一致。
三个最终隧道的 `forwarded_tls_bytes_equal=true`；不是用 MITM 的 TLS 代替 Gateway 的 TLS。
这不意味着应用数据分片、TCP 或长期行为已经全部相同。

最终 HTTP/TLS 两条 Gateway 路径使用同一冻结测试二进制，SHA-256：
`9f4634875f980acda46bbbfc03483b8a6f49aa2115843fd9c2821f58b989e076`。

## 有意保留的差异

不能把三路称作逐字段、逐字节完全相同：

| 差异 | 原因与边界 |
| --- | --- |
| `X-API-Key` | 原生同时发送它和 Bearer。Gateway 保留所选 adapter 的 Bearer 认证，不以风格为理由擅自增加第二认证头，更不转发入站 key。 |
| `X-User-Id` | 原生由其凭证上下文产生；入站使用 Gateway 本地 key，不能把该用户标识当服务商身份复制出去。 |
| 普通客户端缺失会话/agent/privacy/trace 头 | 没有真实上游会话证据，不造会话、不冒称 agent intent 或隐私状态。 |
| `Accept-Encoding` | 原生为 `gzip, compress, deflate, br`；Gateway 为实际支持的 `gzip, deflate, br`，不虚报旧 compress 解码能力。 |
| 普通客户端请求 ID 形状 | Gateway 自生成 UUID 请求 ID 与原生 32 位格式不同；已有原生请求 ID 保留，不改写调用者关联标识。 |
| JSON 序列化字节 | canonical 正常重新序列化，语义相等并不意味着键顺序、空白或 body hash 相等。 |
| 两次独立 WorkBuddy 的业务文本 | 隔离 HOME 路径、环境描述和本轮 ID 不同；用同次入站/出站对照确认不是 Gateway 漏转。 |

完整 TLS 应用数据分片、TCP 分段、长期重试/连接复用模式、辅助接口和桌面 UI 行为
不属于本次单轮 ClientHello/HTTP 字段对照的已验结论；不承诺 Gateway 无法被识别。

## 验证状态

最终源码 `live-final-r2-contracts.log`：26 项全部通过，其中 WorkBuddy 合同 7 项，
既有 profile 合同 8 项、Claude wire 6 项、Codex fidelity 4 项、runtime 矩阵 1 项。
矩阵包含 90 次实际回环 HTTP 请求。另有 3 项 WorkBuddy 入站名单/边界单元测试通过，
见 `live-fix-check-1.log`；该入站模块在随后 TLS 调整期间未变。

`live-final-r2-gates.log`：checker 自测 31 项、effective-line ratchet、官方 formatter、
`git diff --check` 全部通过。修改源码最大 441 有效行，未扩大任何 >700 文件；
`live-final-source-receipt.json` 记录各文件有效行、SHA-256 和无 BOM 检查。
`live-final-r2-build.log` 记录最终 `cargo build --locked --bin gateway` 成功。

此前失败/中间证据保留：`live-tls-diff.json` 是 ALPN/padding 修复前的反例，
`live-tls-final-diff.json` 才是最终结果。一轮过时源码的回归编译主动停止后，已用最终源码
完整重跑上述定向合同；不能把中间编译退出记录误判为最终测试失败。

测试隔离 WorkBuddy/Gateway 进程已退出；未修改 DSH、未部署日常实例、未提交推送，
未执行全库测试、完整桌面 UI 流程或所有长期连接/重试场景。
