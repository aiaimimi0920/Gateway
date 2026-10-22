# DSH 普通 Responses 的可选会话兼容

## 修复范围

本记录接续 `2026-10-21-dsh-codex-profile.md`，只处理普通 DSH 调用的 settings/Cookie
初始化与 `session_id` 别名。保留真实消息、工具、推理设置、输出上限和标题生成行为，
不伪造 CLI installation/window/turn metadata、responses-lite 或 beta 能力。

`/v1/responses` 将合法 `session_id` 规范为 `session-id`，同值双名称合并，冲突、重复、
空值和非法值拒绝。此名称兼容不依赖实验开关；缺失名称不生成会话。

自动初始化要求 `GATEWAY_EXPERIMENTAL_CODEX_SESSION=1`、显式 `codex_cli` 官方 OAuth
单候选、standalone owner Bearer 认证、有效 session/model 标签及流式 Responses。
PG/auth-adapter 托管、多候选、WebSocket、compact 不在此次自动兼容范围。
合法原生 CLI UA/originator 对不触发 SDK 自动初始化，保持原 CLI 主动 settings 流程。

入口只分配 scope；真正的 settings GET 在原有认证、过滤、路由和凭证刷新之后发起，
使用与随后 POST 相同的实际凭证绑定。首次成功初始化由 OnceCell 合并并发请求；
成功但没有 Cookie 也算初始化完成，失败或取消可由后续独立请求重试。
非 2xx 或非法 settings JSON 阻止 POST，不盲目重放或切换凭证，不跟随重定向。

Cookie 继续采用既有 origin/path/expiry 和容量限制，不转发调用方 Cookie，不导出上游
Set-Cookie。初始化 lease 持有至 POST 响应头及 Cookie 吸收结束，避免挂起任务跨越
600 秒 idle TTL 后误用新建 jar；取消/完成自动释放，不增加后台线程或持久化 Cookie。

## 证据与验收状态

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-live-five-turn-20261021`。
其中含私有隔离配置，不可整体发布。日常实例、CLI 配置及源 OAuth 文件不在修改范围。

修复前 RED 已实际观察到别名映射、冲突拒绝及共享初始化门闩三项失败。

最终源码 `cargo test --locked --lib native_ -- --test-threads=1`：85 项通过。
另从同一测试二进制运行 headers/request_plan/stream/profile 筛选，去重后合计
1,595 项通过、4 项 ignored；并非全仓测试。31 项 checker tests、ratchet、
`cargo fmt --all -- --check`、`git diff --check` 及 `cargo build --locked --bin gateway` 通过。
本轮 Cargo.toml/Cargo.lock SHA256 与开始时一致，未重新解析依赖；此前锁文件 provenance
限制仍见原生会话报告，不因本次验证而撤销。

本地 `dsh-session-local-01` 使用纯合成凭证，TLS tap 只转回环 fixture，不调用真实上游。
11 组检查通过：错误/混淆认证、别名冲突、非法标签、非流式均不触发上游；六并发
只初始化一次；后续复用、不同 session 隔离；成功无 Cookie 不重复 GET；503 和302
阻止 POST 且后续可重试；无标签保持 stateless。`dsh-session-disabled-01` 验证关闭
开关时 0 GET/1 POST、无 Cookie，别名规范化仍生效。

## 真实网页五轮验收

`dsh-session-live-01`：实际在 `http://127.0.0.1:3080/` 新建默认工作区测试会话，
使用同一指定源 OAuth 凭证及 `gpt-6-sol`，依次得到 `SESSION_OK_1`、
`SESSION_OK_2_17` 至 `SESSION_OK_5_17`。五轮全部完成，未观察到工具调用。

| 接收端抓包检查 | 结果 |
| --- | --- |
| 调用序列 | 先 1 settings GET，随后 6 Responses POST（五主请求加标题辅助） |
| 同一源凭证与实际 Bearer | settings 及全部 POST 均匹配本轮凭证 HMAC；源文件未变 |
| session 别名 | 6/6 从入口 session_id 转为 session-id，无下划线名外发 |
| Cookie 来源 | 6/6 携带此前真实 Set-Cookie 的逐值 HMAC，不复制历史样本 |
| 上游 Cookie 向下游导出 | 无 |
| DSH 入口 JSON 已有字段 | 6/6 逐值保留 |
| Accept / 共同头相对顺序 | 6/6 为 SSE，且与历史 CLI 对照一致 |
| Responses 状态 | 6/6 HTTP 200，五主请求 response.completed |
| 明文 secret 扫描 | 五份回执/比较/抓包文件通过 |

对照继续使用 `oauth-native-session-08` 的 CLI 直连和 CLI 经 Gateway 两份五轮样本，
本轮未重新执行这两组。DSH 的额外标题请求属于允许保留的业务行为，不要求变成五次 POST。
`comparison.json` 和 `session-verification.json` 保存逐项验收；后者验证测试二进制一致性。

最终 `gateway.exe` SHA256：
`174de7ed3d849700092743ae2cd2f0a002916ce369ffcfb01e2a40819f225e59`。

本轮涉及文件有效代码行：alias 84、sdk admission 59、HTTP native_session 204、
responses 93、Cookie sessions 202、upstream native_session 143、binding 45、bootstrap 89、
native_codex 223（本轮前为217）；全部低于250，无新增尺寸例外。

隔离 Gateway 与 tap 已结束，未遗留 gateway.exe。网页已恢复原 Crow 会话及
`deepseek-v4.1-flash`；测试会话和提供商保留，提供商地址指向已停止的临时端口。
未提交、推送、正式打包或部署日常实例。此次证明指定 HTTP 会话差异已修复，
不是完整 CLI 行为模拟，也不证明移除 MITM 后全部 TLS/连接侧特征一致。
