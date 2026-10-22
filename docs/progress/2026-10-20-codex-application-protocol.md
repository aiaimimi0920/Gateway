# Codex 应用协议续进

恢复来源：`01a15905-e45b-7153-bb2e-c6bdd7fc29e3`。
最后授权是保留凭证替换、请求级鉴权、路由和计费，补齐应用协议与可实现的网络保真，
明确传输层边界；不再要求两段 TLS 连接的错误等同于客户端本地建连错误。

## 实现范围

- `/v1/responses/compact` 及 `/v1/new-api/responses/compact` 经完整 pipeline，
  使用已选 Responses endpoint 的 `/compact` 子路径。保留原始 JSON、query 和允许的
  Codex metadata；模型映射仍生效，不注入 create 专属的 stream/store/include 等默认值。
- 两个 Responses URL 增加 GET WebSocket，支持当前 Codex 的串行 `response.create`、
  `generate:false` 预热、`previous_response_id`、opaque/encrypted items 和终态事件。
  每个 create 重新鉴权、准入和结算，不用握手身份代替后续权限检查。
- 原生 WS 通过既有 reqwest native client 构造策略执行 upgrade，复用 TLS、自定义 CA
  和代理策略；每条下游 WS 懒创建专属 HTTP/1.1 client，不与普通 HTTP ALPN 池混用。
  没有直接使用另一个默认 Rustls transport 来冒充之前的 Schannel 路径。
- 上游连接仅属于一条下游 WS；绑定 URL、选中认证材料和 provider-owned headers。
  身份变更拒绝复用，连接中断不重放已交付流的 create。正常终态才归还连接。
- 内部 SSE envelope 复用 usage/completion/audit/settlement taps，对外仍是 WS JSON。
  上游 error/failed/incomplete 先转发原事件，再触发失败结算，不伪造 completed。
- 帧、消息、query、解码缓存有上限；逐帧 await 写入提供背压，不创建无界队列或
  脱离连接的 relay task。空闲 300 秒、下游写入 30 秒超时属于 Gateway 的明确边界。

## 本轮实际发现的缺陷

1. 首版在 WS 建连前后直接取消 pipeline，会使已预扣而尚未建立 TrackedStream 的
   请求缺少退款。新增测试观察到余额 `1 != 2`。改为通知 pipeline 内部取消实际 transport
   send，保留 ctx 执行既有失败 finalizer。独立审查进一步发现 auto_route 的 source
   预扣也发生在 stage_send 内，因此不能取消整个 stage_send；最终取消边界位于该预扣
   及 ctx 记录之后。断开时允许准入事务完成，不承诺无条件即时取消。
2. 首版仅绑定三个认证头，遗漏服务商自定义身份头。绑定加入有序的 provider headers。
3. 长等待时显式 flush 下游 Pong；终态后优先 drain stream，避免把紧接着的下一轮
   create 误判成并发取消。
4. 官方当前 WS 文档表示不使用 `stream`，但本机真实 Codex CLI 0.155.1 抓包在
   预热和生成两种帧中都发送 `stream:true`。首版拒绝该字段导致 CLI WS idle timeout
   并回退 HTTP。实现按实际参考客户端兼容：保留显式 true，省略时不新增。
5. 真实 CLI 0.155.1 的 remote compaction 已使用普通 `/responses` 上的
   `input: [{"type":"compaction_trigger"}]` V2 协议，不调用旧 `/responses/compact`。
   自定义 provider 的 `name` 必须匹配 `OpenAI` 才启用此能力；最初 probe 未匹配该能力，
   没有生成预期请求，不能算通过。后续分别验证 CLI V2 和独立 compact endpoint。
6. 补充自定义 CA + 双 ALPN 的 HTTPS WS probe 后，复现 direct exit 0/ALPN null，
   Gateway exit 1/ALPN h2。GET Upgrade 不会自动让 reqwest 禁用 HTTP/2，普通 HTTP
   的 ALPN 配置不能直接用于 RFC 6455 WS。修复为 WS connection-local client 使用
   `http1_only`，自定义 CA 克隆配置清空 ALPN（匹配参考 WS 客户端），普通 HTTP
   ALPN 保持原样。证书验证、根证书、代理及连接身份绑定未绕过。

## 验证记录

本轮隔离证据根：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-codex-application-20261020/`。
测试仅使用回环 upstream、合成凭证、独立 local data / CODEX_HOME；不连接真实服务商，
不修改系统证书、真实账号或常驻实例。初版失败证据保留，不覆盖为成功。

最终源码对应的 debug EXE：`target/debug/gateway.exe`，SHA-256：
`af99ad5e81e2584e78182f7bb02cb9627454c6333a3f1968e2a5e94e38809f26`。

已完成的最终二进制回归：

| 验证 | 结果 | 证据 |
| --- | --- | --- |
| 真实 CLI WebSocket | direct/via 均 exit 0；各 2 帧；精确重放 2 帧上游 bytes 相等 | `live-ws-final-2/verification.json` |
| 真实 CLI HTTPS WebSocket | 双 ALPN 服务端下 direct/via 均 exit 0；精确重放相等；TLS ALPN 均为 null，没有误选 h2 | `live-wss-final/verification.json`、`tls.json` |
| 真实 CLI remote compaction V2 | direct/via 各发生 1 次；首轮和 resume 共 4 次均 exit 0；精确请求重放相等 | `live-compact-v2-final-2/verification.json` |
| HTTP 异常/重定向 | 403、408、502、504、307、308、持续 500/503 共 8 组的状态序列、请求数、连接数及 CLI exit 对齐；307/308 重放 bytes 相等 | `http-sequences-final-3/verification.json` |
| 自定义 CA | direct/via/repeat 均成功，重放 HTTP bytes 相等；所有捕获的 TLS 为 TLSv1.3、ALPN `http/1.1` | 相邻 `gateway-codex-transport-20261020/custom-ca-application-final-2/verification.json` |

HTTP 失败序列的首轮 probe 在路径拼接处发生 Python `TypeError`，未改产品源码，修复
probe 后另建目录全量重跑；原失败目录保留。时序耗时并不相等，
不把请求数、连接数对齐解释为网络生命周期逐毫秒一致。

`codex_application_contract` 与 `codex_native_fidelity_contract` 最终共 7 tests 通过。
其中应用合约覆盖 compact 原始 JSON/opaque output/扣费、未授权 WS、同连接串行两轮、
凭证替换、下一轮额度复核、非法帧、上游 error 退款、Ping/Pong、流中断和握手中断退款。
有效行数 checker 31 tests 通过；ratchet 通过（14 个单独归类的 immutable artifacts，
governed source >700 为 0），formatter 与 `git diff --check` 通过。

最终源码的定向 Rust 单元回归共 64 tests 通过，使用以下过滤集合（未声称运行全部
3536 个 library tests）：

```powershell
rtk cargo test --locked --test codex_application_contract --test codex_native_fidelity_contract -- --test-threads=1
rtk cargo test --locked --lib -- native_codex codex_wire native_json protocol::responses:: --test-threads=1
```

构建警告不等于零：除已有 unused/dead-code 警告外，WS handler 的 public 签名引用
crate-private `ClientRequestId`，存在 `private_interfaces` 警告；当前内部 router 编译和
真实请求均已通过，不将它描述为零警告构建。
最终 `rtk cargo check --locked --all-targets` exit 0，0 errors、44 warnings。
本轮 probe 的 Gateway/CLI/Python 进程已结束；未停止其它项目进程。最终 26 个范围内
文本的 UTF-8 无 BOM 验证与 SHA-256 清单保存在证据根 `final-source-receipt.json`。

## 明确边界

- 参考目标是 Windows Codex CLI 0.155.1 的串行 Responses 工作流，不是所有最新 SDK
  的全部 WebSocket 功能。并发 `stream_id` multiplexing、binary frames、压缩扩展协商
  当前不支持；并发 create 不排队，会结束当前连接。
- 下游握手先完成，路由需等 create 的模型和请求内容后才能选择上游。因此上游握手
  headers/status 不可能原样成为已经完成的下游握手，后续失败使用 WS error 表达。
- 不承诺 WS 握手头顺序、Sec-WebSocket-Key、mask、分片、压缩、Ping 时序、关闭帧、
  TCP/TLS 生命周期和源 IP 等同于直连。HTTP 原始字节和 TLS 的旧证据不自动覆盖 WS。
- 请求策略默认值、模型映射、显式 provider 配置仍优先于逐字节保持；不兼容的
  tool/protocol bridge 明确拒绝 compact/WS，不悄悄变成普通 create。
- `generate:false` 也走既有请求准入/消息计数；不是额外发明的免计费通道。
  token usage 按上游报告处理，真实服务商价格、账户授权和生产财务仍需单独验收。
- 没有进行跨 OS、其它 CLI 版本、真实服务商风控或发布打包验收；没有提交、推送或部署。
- 本次取消修复及退款回归针对 WS caller disconnect。现存 request-budget deadline
  仍包围候选处理，数据库 reserve 提交/返回间发生总预算超时的事务安全不是本轮已证明
  的能力；没有执行 PostgreSQL auto_route 提交边界故障注入，也不声称 crash exactly-once。

## 协议来源

本轮已联网获取并保留：
- `https://developers.openai.com/api/docs/guides/websocket-mode.md`
- `https://raw.githubusercontent.com/openai/openai-python/main/src/openai/types/responses/response_compact_params.py`
- 官方 Codex `rust-v0.155.1` tag 对应 commit
  `be2951ea34f0d295ed0becf97079f92fa5f6950e`：`model-provider/src/provider.rs`、
  `core/src/compact_remote_v2_attempt.rs` 和 `core/tests/suite/compact_remote.rs`。
  本机版本输出匹配 0.155.1，但未把本机 binary 与官方 release artifact 做 hash 对照。

与实际 CLI 抓包冲突时，明确记录版本差异，不把最新文档误当 0.155.1 的实际 wire。
