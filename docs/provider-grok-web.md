# Grok Web：当前 WebSocket 文本子链

日期：2026-11-06。仅适用 `adapter: grok_compatible` 且
`call_flow.special_adapter: grok_web` 的正常 Web 模板。xAI API Key、Grok Build
OAuth 和 Web SSO 仍是不同产品，不共享材料、不自动跨渠道兜底、不推断额度互通。

## 合同来源与置信边界

本批联网调查固定了 [chenyme/grok2api 的 commit](https://github.com/chenyme/grok2api/tree/e2fd7ab481a30a1965bb61395d437154cf247c92)。
其当前文本 `openChat` 直接调用 `openGatewayChat`，不是旧 POST NDJSON fallback。
关键参考：

- [gateway.go](https://github.com/chenyme/grok2api/blob/e2fd7ab481a30a1965bb61395d437154cf247c92/backend/internal/infra/provider/web/gateway.go)：WS 路径、头、会话/turn/心跳及响应字段。
- [sessionidentity/session.go](https://github.com/chenyme/grok2api/blob/e2fd7ab481a30a1965bb61395d437154cf247c92/backend/internal/infra/provider/sessionidentity/session.go)：身份 GET 与 UUID 来源。
- [catalog.go](https://github.com/chenyme/grok2api/blob/e2fd7ab481a30a1965bb61395d437154cf247c92/backend/internal/infra/provider/web/catalog.go)：`fast / auto / expert / heavy` 模式。
- [protocol_test.go](https://github.com/chenyme/grok2api/blob/e2fd7ab481a30a1965bb61395d437154cf247c92/backend/internal/infra/provider/web/protocol_test.go)：合成 WS 会话和响应帧，**不是平台真实录制**。

本地完整 ZIP、逐文件 SHA256 与来源清单在
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-web-20261106-contract`。
当前参考 ZIP SHA256 为 `75527a3ffa8852fe6372fda52cb188414dd4f173ed801760d76f67a11d051176`。
历史 POST 版本也已固定，只用于说明迁移，不作为当前平台仍支持旧协议的证据。

这是一份第三方公开实现合同，不是官方稳定 API，也没有真实平台响应、权益或费用保证。
参考实现要求 session ID 与 conversation ID 相同；目前仅源码和 mock 支持这一假设。
未找到可信的中间帧 response ID、多候选选择或空 terminal status 表示成功的真实录制。
因此不会强制发明每帧必有 response ID，但遇到已出现的 ID 变化会拒绝；成功必须有
`response.done.response.status: completed` 和非空答案。

## 本批明确支持的范围

2026-11-12 [普通余额Key真实TCP/WS联合](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-grok-prepaid-tcp.md) 已连接正常Key/SQLite、真实HTTP下游与合成GET/WS：成功次数扣一次或token保留预扣，异常与增量后断连退款并关闭WS，audit用量保持未知。此处不是仅oneshot，但仍非真实平台、现金或成品验收。

2026-11-12 [精确阈值与重叠准入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-web-token-competing.md) 已验证：余额恰好等于预扣时可调用，两个重叠请求只允许一个使用最后额度，被拒绝请求不误退预扣，取消后只恢复一次。限单实例本地合成合同，非跨进程/PG压力验收。

2026-11-12 后续 [非流式结算与不足准入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-web-token-admission.md) 已补齐 buffered 成功保留预扣且usage未知，并验证0/1 token不足余额在普通/流式入口均429且零上游调用。未覆盖并发或真实收费。

2026-11-12 补充 [token 预付费合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-web-token-prepaid.md)：成功流在 usage 未知时保留既有预扣，audit 不伪造 tokens；失败或增量后取消释放预扣。仅本地合成合同，不代表真实用量、现金定价或供应商扣额。

2026-11-12 补充：普通 Gateway Key 的 `message_prepaid` 已以生产本地状态和合成上游验证成功扣一次、失败退款、增量后取消释放预扣；token usage 仍未知。仅按次数合同，不代表现金定价或真实供应商扣额验证。详见 [本批回执](progress/2026-11-12-provider-channel-expansion-grok-prepaid.md)。

- 公共 `/v1/chat/completions` 与 `/v1/responses` 的单轮、一个 user、一个纯文本 part。
- `grok-chat-fast / grok-chat-auto / grok-chat-expert / grok-chat-heavy`，也接受对应短模式名。
  这些是**模式**，不是固定模型版本或账号权益证据。旧 `grok-3` 等名称不会偷偷映射。
- buffered 聚合，或 pull-driven 真增量 Chat SSE，再复用原 Responses 转换和终态/计费 owner。
- 显式管理模型测试，严格按已选择的 credential/model 执行同一 dispatcher，不重新路由。
  默认连通度检查不会发送生成。保存 Grant、禁用账号和模型归属仍由原管理边界处理。

多轮、system/history、附件、工具、JSON schema、generation 参数、续聊和公共 Messages
当前明确拒绝，不拼接、不静默丢弃。未知响应类型、搜索/工具/citation、媒体附件同样不能
当成完整文本功能成功。ANALYSIS/REASONING 不混进正文；本批不宣称推理过程保真。

## 发送与资源 ownership

1. 验证同一精确 payload 的 audience 与 Cookie；只允许固定 `https://grok.com/`。
2. 沿现有 Chrome136 wreq client/代理，GET `/api/auth/session` 取稳定 UUID；不按邮箱猜身份。
3. 同一 client 与账号 Cookie/UA，HTTP/1.1 Upgrade `/ws/mgw/?uid=<uuid>`；验证 nonce、
   Connection、Upgrade、Accept，拒绝未请求的 extension/subprotocol，不跟 redirect。
4. 只发送一次 session.create；created/attached 匹配后才发送一个 user item 和 response.create。
5. 明确请求 `enable_image_generation: false`、`enable_side_by_side: false`。这是本地文本-only
   策略；字段名见参考源码，**false 的平台接受性与实际禁用效果仍待真实验收**，不宣称服务端零媒体成本。
6. 每 25 秒应用层 ping，所有写在同一 socket owner 中串行；没有后台转发任务或无界队列。
7. 只有完整成功终态才能发成功 finish/[DONE]；提前 close/EOF、坏 JSON、错误帧和未知终态均为 typed error。

总预算至多 120 秒（包含身份 GET、握手和响应），身份 GET 至多 15 秒，握手至多 20 秒，
单写至多 15 秒且受总预算约束。身份 body 64 KiB，WS message/frame 1 MiB，累计文本帧
8 MiB/16384 帧，正文 2 MiB。Drop 释放 socket，不自动重提或转投旧 HTTP；未证明远端
`response.cancel`，因此不承诺取消立即停止平台计费。背压期间不建独立 relay/心跳任务。

完整 Cookie（包括 clearance）保留；`sso` 必须与当前账号 token 一致，重复/坏 Cookie 拒绝。
caller Authorization/Cookie/UA 不进入本链。传输错误只输出固定 code，不回显上游 raw body、
连接 URL 或会话 ID。未获真实 usage 时保持 `None`/省略，不用本地字符估值或伪零量冒充真实扣额。
最终配额与审计仍由原 pipeline 处理；未知 usage 也不意味着实际免费。

## 兼容与未完成项

旧的非 typed `grok_compatible` 自定义配置继续走其历史兼容实现，未自动迁移或宣称重验。
正常 typed 模板不能调用 raw request-plan/JSON/binary passthrough 退回旧 POST。
feature-off 的正常执行返回 `grok_web_line_disabled`，不触网兜底。

还缺完整多轮/模型目录/额度/订阅、搜索引用和其他声明能力、SSO/challenge 的录制与恢复、
原生 devtest 包、真实平台/费用/权益逐渠道验收。当前文本子批不等于 R03 整卡
`offline_ready`，也不等于 P50/P60/P70 完成。验证结果见[本批接力回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-grok-web.md)。

2026-11-12 后续已补 [公共真实 TCP / WS 合成验收](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-grok-public-tcp.md)：两公共入口的成功、提前关闭、坏 JSON、未知终态与客户端断开均有真实 socket 证据。此限定子链不替代正常管理网页联合、独立进程/包或真实平台验收。

2026-11-12 已进一步补 [正常管理网页联合验收](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-grok-probe-ui.md)：取消导入无写入，正常 Grant/CAS 保存合成 SSO/Cookie/UA 后精确账号一次 probe，实际身份 GET/WS 使用新材料，页面结果与唯一脱敏审计一致；未知 usage/额度保持未知。未涵盖重启恢复、真实平台或应用包，不能据此宣称整卡完成。

2026-11-12 [本地会话材料恢复合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-web-session-restart.md)补齐生产 new_local 的旧材料读取、轮换、移除与发布前后中断恢复；这是 runtime/store 重建和磁盘阶段注入，不是独立应用进程或真实平台验收。

2026-11-12 [管理 TCP 与本地恢复联合](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-web-session-management.md)进一步验证正常 Grant/secretPatches/CAS 保存后关闭旧管理状态、新服务从磁盘恢复，脱敏读取和路由材料一致，旧 Grant/旧 revision 不得覆盖；仍非完整应用进程或真实平台验收。

2026-11-12 [正常网页跨进程联合](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-12-provider-channel-expansion-grok-process-ui.md)已实际完成同一材料的网页保存、首宿主退出、不同PID从同根本地authority恢复、网页精确probe及合成WS出站。此处是测试宿主进程重启，不是成品EXE/全启动/真实平台验收；其他边界保持。

## 参考许可

本批用上述公开字段合同在 Gateway 框架内独立实现 Rust owner，没有 vendored 或执行参考项目。
保留该参考项目的 MIT 许可文本如下：

```text
MIT License

Copyright (c) 2026 Chenyme

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
