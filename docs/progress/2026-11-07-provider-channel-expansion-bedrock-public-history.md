# Bedrock：公共多轮 reasoning/signature 历史续接

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：下述实现与最终定向验证通过；不是完整 Bedrock、W06 整卡 `offline_ready` 或 P50/P60/P70 完成。

## 1. 请求与响应合同

承接[实时响应子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-reasoning-stream.md)，本批连接三公共入口 Chat Completions、Messages、Responses 的下一轮历史，而不是再造授权、会话数据库或代理服务。

- 生产 JSON 与 SSE 响应的 `gateway_bedrock_turn` 在有序 native `content` 外新增可选 `continuation`：`source=aws_bedrock_converse`、实际 resolved upstream `model` 和 `request_sha256`。摘要是稳定 JSON `{messages,system}` 的 SHA-256；messages 是实际原生请求前缀，缺省 system 序列化为 null。Buffered 使用实际发送的最终 plan；SSE 复用同一完整、确定性 planner，包括 provider defaults，不只对早期 canonical pack 取摘要。
- 后续请求显式携带顶层 `gateway_bedrock_history` 数组，每项只有 `assistant_turn` 和完整 `turn`。ordinal 对应重建后的原生 assistant turn，而不是 Responses 分拆 item 的序号；必须递增、无重复且确实存在。按序恢复旧 turn 后，才核对后一 turn 的完整原生前缀。
- 同角色相邻消息合为同一 Bedrock turn；Responses 的 text/function_call 和并行 tool results 因此有稳定的首轮/续轮打包。结果必须完整关联紧邻 assistant 的 toolUse ID；文本结果保留 `text`，即使它看起来像 JSON，也不重新解析；不发送 AWS ToolResultBlock 不存在的 `name` 字段。
- 可见 text、tool IDs/names/JSON arguments 必须与所带 turn 一致；有显式 Bedrock history 的 Chat 参数不走 HTML 实体修复。畸形参数不能靠旧 `{}` fallback 与伪造 turn 匹配。完整 reasoning、signature、redacted Base64 值及原顺序按 typed turn 原样恢复。
- 重 normalize 的原始公共消息必须仍与当前 canonical policy 投影相同；不把 raw 历史覆盖到策略改写之后。`extra.messages/system`、混合 Vertex history、无绑定旧 turn、跨产品和模型错配均明确拒绝。最终 provider system defaults 也参与摘要；续接不允许旧自定义 path 改投另一模型。
- 单 turn 仍为 4 MiB / 4096 blocks，continuation 有独立有界 allowance；history 总序列化上限 8 MiB、最多 64 项，并在复制/反序列化 turn 前准入。closed schema、union、Base64 及已有流/响应预算继续生效。
- 公共 guard 位于 effective call-flow 与 native/product shortcut 之间，并覆盖 pipeline 专用入口；response-only turn/event/terminal 不能作为请求顶层数据或 provider defaults 转发。历史扩展不会进入 AWS JSON，也不会转发给其他产品。正常 pipeline 在发送准入前执行完整 history plan 校验，避免将本地历史冲突误记为供应商失败。

### 这不是签名验证或权限令牌

`request_sha256` 只是本地一致性 witness，不是 MAC、凭据、授权或 AWS opaque signature 的真实性证明。客户端可以自行计算普通 SHA-256，不能据此获得新权限或绕过正常 Key/route/provider 控制；AWS 最终判断自己的 signature。scope 不绑定 provider ID、账户额度或任意 origin，也不能作为跨账户权益共享证据。旧无 continuation 响应仍能读取，但不能直接用作公共 signed continuation。

不在公共响应复制内部 system/policy 前缀，只返回摘要。类型 Debug 保持 opaque，静态错误不回显 reasoning、signature、工具参数或旧消息。本批没有数据库版本/存储迁移、新秘密、新后台任务或新的重试路径。

## 2. 固定公开依据与红绿

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-public-history-20261107`。

匿名联网取得 AWS [ReasoningTextBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ReasoningTextBlock.html)、[ToolResultBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolResultBlock.html) 及[当前 Converse 指南](https://docs.aws.amazon.com/bedrock/latest/userguide/conversation-inference.html)。原文、HTTP 状态、时间、字节和 SHA-256 保存在 `sources/receipt.json`；旧 conversation-inference-call 地址只是 moved 页面，不拿它作为完整协议依据。

AWS 当前指南明确要求后续请求包含 signature 和全部旧消息，修改旧消息会失败；ReasoningTextBlock 的 signature 字段仍是 optional，不擅自收紧。ToolResultBlock 的成员没有 name，toolUseId 为 1–64 且使用明确字符集。

- `red-history-v1`：4 项真实行为测试 **0 passed / 4 failed**，暴露 reasoning/order 丢失、literal arguments 修复、模型/policy/extra 不拒绝和无绑定旧 turn 可进入续接。三协议循环在首个分支即失败，不虚称三分支均单独红测。
- `green-history-v1`：4 passed，首批行为转绿，所选输入无阶段内漂移。
- `expanded-history-v1`：15 passed / 6 failed。六种 JSON/SSE 起步均已完成三轮实际路由、响应-issued turn 回传和最终 wire 核对，失败在末尾健康断言：三次本地 400 将成功计数 3 重置成 0。这是实现的责任层问题，不是要删除的测试；已将完整历史校验前移到发送准入之前，原断言保留。

## 3. 最终验证

Rust 使用 `cargo test --locked --offline --no-default-features --features runtime-defaults -j 4`，`--test-threads=1`；TEMP/TMP/TMPDIR 指向本批证据目录，没有全局增加 `RUST_MIN_STACK`。新路由合同沿用明确的 16 MiB 专属测试线程，不改变生产栈或 future。代表性关闭组合从 runtime-defaults 中精确去掉 Bedrock family/line，不冒充完整 feature 矩阵。

| 范围 | 结果 | 回执 |
| --- | --- | --- |
| 公共历史、实时/非流式响应及正常 JSON 路由四组 | 54 passed，其中历史21项新增 | `history-final-v1.json` |
| 13组 Bedrock 认证/参数/长模型/native/probe/历史相邻合同 | 93 passed | `adjacent-final-v1.json` |
| 库过滤器 `bedrock` | 30 passed | `library-bedrock-final-v1.json` |
| 库过滤器 `anthropic` | 94 passed | `library-anthropic-final-v1.json` |
| 库过滤器 `responses` | 158 passed / 1 ignored | `library-responses-final-v1.json` |
| 库过滤器 `tool_inject` | 64 passed | `library-tool_inject-final-v1.json` |
| 库过滤器 `vertex_turn` | 12 passed | `library-vertex_turn-final-v1.json` |
| 库过滤器 `openai` | 170 passed / 1 ignored | `library-openai-final-v1.json` |
| 正常生产 lib / 代表性关闭 Bedrock 的生产 lib | 均 exit 0 | `production-final-v1.json`、`disabled-final-v1.json` |
| 25项 Rustfmt / checker tests / ratchet | 均 exit 0；checker31/31，ratchet扫描4816项 | `rustfmt-final-v1.json`、`checker-final-v2.json`、`ratchet-final-v1.json` |

按 harness 与完整测试名去重，最终 **631 passed / 0 failed / 2 ignored**。各过滤器重叠，不能直接相加，也不重复累计中间运行。两个 ignored 分别需要官方状态录制夹具和真实 OpenAI 网络；本批不执行、不计为通过。不是全仓回归、完整 feature 矩阵或正式发布门禁。

13个终态阶段各自冻结的4241项所选 Cargo/Rust/worker MJS 输入与终态逐项一致；不外推所有 UI、环境、依赖或 runtime 数据。25项本批Rust（6新增）、五份文档、固定原文、direct preimages、UTF-8/无BOM、本地链接、范围diff及 `git diff --check` 见 `audit.json`、`test-identities.json` 和 `scoped-diff.patch`。根有效行报告复制入证据后，按本轮直接 preimage 原bytes恢复，保留继承脏改。

25项Rust最大497有效行，无501–700例外或baseline调整：packing200→105、新 messages80、public_history187、continuation80；大 dispatcher496→497，stage_send475→494。ratchet继续报告14项固定 immutable runtime assets，`governed source >700=0`，不是把第三方大文件藏掉。本批源码审阅覆盖输入/秘密、边界预算、复制/序列化、稳定前缀、健康归属、取消/Drop及资源清理；不新增持久状态、外部进程或重试。

## 4. 仍未完成

本子链不等于完整参数/模态、modeled AWS exception、专属完整取消审计、公共 TCP/独立进程/原生包或真实 AWS 权限/模型/额度验收。测试使用正常 router/auth/send 与真实随机 loopback AWS 合成上游；不能冒称独立公共 TCP 客户端或 AWS 账号验收。

累计新增模板仍 17；FreeBuff 公共业务 guard、其他未完渠道、Suno/Luma MCP client/tools、Qwen OAuth 待决和 P50/P60/P70 继续。没有读取实际 `.ng`/凭据、调用真实供应商、修改在线实例、创建 Redis 容器、出包、提交或推送；不重复消费历史 P34 审批。
