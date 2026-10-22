# DSH 经 Gateway 的 Codex 请求风格对照

后续更新：下文保留本轮历史边界；settings/Cookie 和 session 别名两项后续已修复并完成
真实网页五轮验证，见 [DSH 会话兼容](2026-10-21-dsh-session-compatibility.md)。

## 授权与范围

用户要求从现有 `http://127.0.0.1:3080/` 网页实际发送，使用此前同一组
OpenAI OAuth 凭证，经 Gateway 转发，并与 CLI 直连、CLI 经 Gateway 的证据比较。
本轮使用独立 Gateway 数据目录，在 DSH 新增 `gateway-codex-wire-test` 提供商，
不修改既有 DeepSeek 提供商。DSH 只持有回环测试 Gateway key，不持有上游 OAuth token。

证据根为 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-live-five-turn-20261021`。
目录包含私有隔离实例数据，不可整体提交或发布。抓包启用上游证书验证，使用进程级 CA，
不修改系统信任库，不改变日常 Gateway 或 CLI 配置。

## 修复前实测

`dsh-responses-01`：从网页在默认工作区新建一段对话，真实发送五轮不使用工具的测试消息，
五轮均显示成功响应 `WIRE_OK_N_17`（第一轮为 `WIRE_OK_1`）。上游捕获六次 Responses
POST：一个含标题生成指令的小请求，加五次主对话请求；不是五次上游 HTTP 请求。
主请求携带 DSH 的 39 个工具定义，但本次没有要求或观察到工具执行。
六个响应 HTTP 状态均为 200；五个主请求有 `response.completed`，辅助请求以
`response.incomplete` 结束。不能将辅助请求的 HTTP 200 当作完整生成成功。

历史比较对象是 `oauth-native-session-08` 的两组已完成 CLI 五轮采样，本轮未重跑历史组。
三组的导入回执中源凭证文件 SHA256 一致；跨轮 HMAC 盐不同，不能直接比较授权头 HMAC。

逐字段比较发现：

- DSH 六个请求的所有入口 JSON 字段逐值保留；仅新增 `include`、`instructions`、
  `parallel_tool_calls` 三个既有 profile 默认字段。
- `User-Agent` 与 `originator` 已转换为 Codex 风格，Node SDK 的 `X-Stainless-*`、
  DSH User-Agent 及浏览器相关默认头未外发。
- `stream: true` 却仍保留 DSH SDK 的 `Accept: application/json`；CLI 为 `text/event-stream`。
- 当前 native sender 对所有入口保留头序，因而把 DSH 头序带到上游；共同头的相对顺序不匹配 CLI。
- DSH 的 `session_id` 与 CLI 的 `session-id` 名称不同，前者不在原生头白名单中。
- DSH 没有 settings GET，普通 `/v1/responses` 不启用原生 session Cookie jar。
- DSH 没有提供 CLI 的 thread/window/turn metadata、responses-lite、beta、client_metadata。
  不可复制历史会话状态、伪造沙箱/安装身份或声明调用方并不具备的 beta 功能。
- `input`、工具定义、`max_output_tokens`、推理/输出配置、辅助生成次数属于调用方语义；
  不能为表面一致而替换 DSH 的真实业务内容。

## 窄修与软件验证

`src/upstream/client/native_codex_profile.rs` 负责普通客户端的请求信封规范化。
仅显式 `codex_cli`、有入口 UA 且不构成有效 Codex UA/originator 对的普通 HTTP Responses
调用生效；缺 UA、`auto`、compact、WebSocket 不进入该规则。服务商显式 Accept 优先。
只重排已经存在的头，不增加入口头白名单、不修改认证边界、不生成会话状态、不增加网络请求。

- 修复前新增测试实际失败 2 项：Accept 和共同头顺序；原生/服务商覆盖测试通过。
- 修复后 native tests 78 项、headers 39 项、request plan 30 项、stream 38 项、profile 1 项通过；
  按测试全名去重共 185 项，不把重叠测试重复计数。
- checker tests、ratchet、官方 formatter check、`git diff --check` 通过。
- 有效代码行：原 `native_codex.rs` 从 213 到 217；新 profile 模块 63；新测试模块 135。
- 未运行全仓所有测试、正式 release/package gates；没有提交、推送或部署日常实例。

## 修复后真实网页复测

`dsh-responses-02` 使用新编译的 `target/debug/gateway.exe`，SHA256：

```text
9747c6a7de346df70817eca089a21c5b361f2b31540c4cb7dd587becd7a0baf4
```

从同一个 DSH 网站新建独立会话并再次发送五轮，页面依次显示 `WIRE_FIXED_1`、
`WIRE_FIXED_2_17` 至 `WIRE_FIXED_5_17`。仍为六个真实上游 POST，五个主请求完成，
辅助生成以 `response.incomplete` 结束。对六个上游请求的验证结果：

- `Accept: text/event-stream`：6/6。
- 与参考 CLI 共同头的相对顺序相同：6/6；这不是头集合完全相同。
- UA 与参考 CLI 一致，`originator=codex_exec`：6/6。
- 所有入口 JSON 字段逐值保留，没有移除或改写；仍仅添加三个既有默认字段：6/6。
- 实际出站 Bearer HMAC 与本轮指定源凭证计算的 HMAC 一致：6/6。
- 源凭证文件与此前 CLI 两组导入时的 SHA256 相同，测试结束后未变化。

辅助请求和主请求可能跨连接并发，比较器按 model/input 内容关联，再检查全部其他字段，
不按两个抓包文件的全局序号强行配对；缺失或有歧义的关联会拒绝得出保真结论。

`comparison.json` 保存三路字段差异；`verification.json` 记录二进制一致性、185 项测试去重、
10 个证据文件的 token/account-id 明文扫描，以及本轮 Cargo.toml/Cargo.lock 未变化的检查。
本轮没有再生成或修改锁文件，不撤销上轮已披露的历史锁文件 provenance 限制。

两次隔离 Gateway 和抓包进程均已结束，未遗留 gateway.exe。网页已返回原来的 Crow 会话，
该会话仍选用 `deepseek-v4.1-flash`。两个测试会话及新增的隔离测试提供商保留，
提供商指向已停止的临时回环端口，不是日常可用服务，勿将其误认为正式部署。

## 尚存边界

本轮修复的是实际可定位的请求风格问题，不是完整 CLI 会话模拟。普通 DSH 路径仍没有
settings GET/Cookie 会话初始化，`session_id` 别名未转换为 `session-id`，也没有生成
CLI 的 thread/window/turn/client metadata、responses-lite 和 beta 功能声明。
这部分不能标记为已解决；若要补齐，需要由显式会话适配层管理真实状态和生命周期，
不能从历史 CLI 请求复制状态或将 profile 默认开关当作完整会话能力。

DSH 的消息、工具、推理设置、输出上限、标题生成等业务差异有意保留。
此次切片证明 HTTP 字段和调用序列，不证明移除 MITM 后全部 TLS/连接指纹一致。
因此结论是“网页链路真实可用、两项请求信封差异已修复复测”，不是“三路网络包完全一致”。
