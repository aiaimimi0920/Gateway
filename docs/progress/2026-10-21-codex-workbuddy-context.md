# Codex CLI 入站的 WorkBuddy 上下文转换

## 参考与发现

本轮继续读取已验证官方 WorkBuddy 国际版 5.8.4 安装包内的 CLI 2.161.0，
参考 `dist/codebuddy-lite-wb.mjs` 的 `buildConversationHeaders`、
`getEffectiveConversationRequestId`、`resolveSessionAgentType`、
`resolveModelCallAgentType`、`applyCompactionHeaders` 及隐私设置分支。
这些是本地固定版本的证据，不宣称已经检查未来或所有版本。

真实 Codex CLI 0.155.1 对照发现：选用 WorkBuddy profile 后，原生 Codex 的
`session-id`、`thread-id`、`x-client-request-id`、`x-codex-turn-metadata`、
`x-codex-window-id`、`x-codex-beta-features` 仍然外发，但没有 WorkBuddy 会话关联头。
其中 turn metadata 还带安装 ID、sandbox 等不应因软件风格转换而向目标服务商传播的细节。

本轮证据仍位于 `linshi/gateway-workbuddy-profile-20261021`：
`codex-before-messages-01` 保存修复前同一 Codex 会话两轮真实调用。
两轮会话 UUID 相同，但原生 WorkBuddy conversation 头均缺失。

## 新增转换

仅显式 WorkBuddy profile 且存在有效 Codex UA/originator 成对身份时执行：

| Codex 真实来源 | WorkBuddy 出站 |
| --- | --- |
| 一致的 thread/session UUID，含 metadata 中的对应值 | `X-Conversation-ID`，续聊保持稳定 |
| `turn_id` | `X-Conversation-Request-ID`，使用无连字符 UUID 格式 |
| `root_turn_id` | `X-Root-Request-ID` |
| 当前实际请求 ID | `X-Request-ID` 与 `X-Conversation-Message-ID` 关联；UUID 转为 32 位格式 |
| `request_kind=turn` | `X-Agent-Intent: craft`、`X-Agent-Purpose: conversation`，采用 WorkBuddy 普通编码会话的默认语义 |
| `agent_name=/root` | `X-Agent-Type: main` |
| 有效且不等于本会话的真实 parent thread UUID | `X-Parent-Conversation-ID` 和 `X-Agent-Type: subagent` |
| 已有有效 W3C traceparent | 补齐对应 B3、X-Trace-ID，并保留 sampled 标志 |
| 没有 traceparent 的实际 Codex 请求 | 创建本次转发请求的随机 trace/span，再生成一致的 W3C/B3 关联字段；不创建会话 ID |

缺失、非法或相互冲突的会话标识不产生会话映射，也不通过消息内容猜会话。
没有 turn 语义时不声明普通会话的 agent purpose/type。
服务商显式头配置优先；自定义认证头不能被重解释为消息 ID 或 trace 后复制到其他字段。
认证头生成仍由 adapter 管理。

同时规范化 Codex 继承的 Accept 为实测 WorkBuddy 的 `application/json`，
服务商显式 Accept 优先。移除未由服务商显式设置的 Codex 专有头与已转换的旧会话别名，
避免产生 WorkBuddy UA 配 Codex 安装/窗口标识的混合身份。
Responses 正文的 `client_metadata` 只清除 `x-codex-*` 软件元数据，
保留普通业务 metadata、会话标签、模型、提示词、工具、缓存和推理参数。

## 其他特殊字段与行为

- 原生 WorkBuddy 的 `X-Parent-Conversation-ID`、`X-Compaction`、`X-Context-Compacted`
  已纳入有界转发名单。Codex 宣称支持 remote compaction 不代表本次实际发生了压缩，
  因此不据此生成后两个状态字段。
- `X-Private-Data` 来自产品通道/隐私设置，不是固定的软件标志；没有明确设置就不补造。
- `X-Session-ID`、`X-Product-Version`、`X-Model-ID`、`X-Data-Tag` 等常量定义不能证明
  每次模型调用都应发送；本轮没有把常量表全量塞进请求。
- `X-User-Id`、enterprise/department/tenant 等字段属于认证上下文，不把 Codex 或
  Gateway 本地账号标识当作服务商账号；不附加第二份 API key。
- WorkBuddy 的官方 service-proxy、账号、hook 和遥测请求不等于自定义模型服务商的辅助接口。
  本轮不对凭证服务商盲目重放这些请求，不创建官方登录态或伪造健康检查流量。
- 先前已验证的 WorkBuddy TLS 配置不变；不新增 TLS 降级、关闭证书验证或放开重定向。

## 真实调用的协议边界

第一次隔离测试使用该服务商 Responses 路径，服务商返回 200 但抓到的流未出现完整
response completion，Codex 报 `error decoding response body`，不计作成功。
没有通过伪造完成事件掩盖失败，也没有将“风格”自动绑定为另一协议。

后续隔离实例显式采用该凭证已验证可用的 Anthropic Messages 接入，使用 Gateway
既有 Responses → Messages 桥接。修复前此链路两轮已正常完成，适合对照修复后的字段。
入站仍是真实 Codex CLI，模型与凭证不变；日常配置、DSH 和正式实例未修改。

## 修复后真实验证

最终二进制 SHA-256：`3db97fbb898b6fb0ca66cefd3a9c48ffefcb130b36823bea7c155309e75b78a2`。

`codex-after-messages-02/context-verification.json`：真实 Codex 首轮加同一会话 resume，
两轮均 HTTP 200、CLI exit 0、`turn.completed`，且均存在 assistant 消息。
逐字段断言全部通过：会话稳定，turn/message 分别变化，root 对应真实源值，agent/trace 一致，
软件标识正确，无 Codex 专有头或伪造的账号/隐私/压缩字段。服务商凭证 HMAC 核对一致。
入站 `/v1/responses`、出站 `/v1/messages`，没有因风格选项自行切换协议。

保留第一次修复后 `codex-after-messages-01`：其字段断言同样通过，但第二轮虽 exit 0 和
`turn.completed`，没有 assistant 消息，且观察到连接重置。因此不将这一组当作完整对话成功，
另做上述一次有界复测。原始 receipt 的 success 仅检查退出码/完成事件/认证；验收以增强后的
`context-verification.json` 中字段与回复两项结果为准，不隐去失败证据。

本轮验证 HTTP 字段及会话关联；TLS 实现未改变，未重新执行前轮完整 ClientHello 对照。

## 本地回归

- `cargo build --locked --bin gateway` 成功。
- 六个定向合同测试共 34 项通过，包括新增上下文转换 8 项、原生 WorkBuddy、Codex、Claude
  和 profile runtime 合同；没有失败或忽略项。编译仍有现存未使用导入等警告，未扩大范围修改。
- effective-lines 检查器自测 31 项通过、ratchet 通过，`cargo fmt --all -- --check`
  与 `git diff --check` 通过。
- 本轮六个源码/测试文件有效行数分别为 198、42、176、47、65、190；全部 UTF-8 无 BOM，
  hash 与计数保存在 `codex-context-source-receipt.json`。
- 隔离 Gateway 和 WorkBuddy 测试进程无残留；没有停止用户已有 Codex 进程。
- 未提交、推送或部署；未修改日常 CLI、DSH 和正式 Gateway 配置。
