# 指定 OAuth 凭证的真实五轮对照

## 授权与实验边界

用户指定本地 CPA Codex OAuth 导出，并明确选择先导入隔离 Gateway 实例。
通过正常 console API 执行 secret grant、validate、带 revision 的 commit；
保留 refresh token、账号 ID、过期时间及 Pro 分组，不修改日常 `.ng` 或原凭证。
真实目标为 `https://chatgpt.com/backend-api/codex`，不是此前失败的 API Key 中转站。
账号实际模型列表包含 `gpt-6-sol`，本次使用该模型。

实验脚本和私密材料位于外部 `linshi/gateway-live-five-turn-20261021`；
目录 ACL 限当前用户和 SYSTEM。认证头、Cookie、turn state 使用同次运行 HMAC，
原始 OAuth token 不写入公开回执。隔离 CLI 的 auth.json 和 Gateway 本地持久数据
仍含必要凭证，不能发布整个实验目录。

原生 Codex CLI 0.155.1 使用 app-server stdio，每组一个进程、一个 thread、
依次五次 turn/start。HTTP/SSE 明确关闭 WebSocket；双方采用相同 MITM 条件，
只转发原始应用层字节，不重序列化。实验 CA 仅进程级信任，真实上游证书仍验证。
MITM 对外经现有回环出口代理，两侧 ALPN 限 HTTP/1.1。
这不是无 MITM 的 TLS 指纹证明，也不是 WS、全部辅助 API 或服务商不可识别证明。

## 已完成基线

- `oauth-http-03`：正常导入；直连五轮成功，Gateway 首轮因走旧 transport 拒绝实验 CA。
- `oauth-http-04`：修正原生通道准入后，直连五轮、Gateway 五轮全部完成；
  两路上游 Authorization HMAC 一致，确认同一指定凭证。
- `oauth-http-04/comparison.json`：五个 Gateway 入口与出口成对请求均发现
  `client_metadata`、`prompt_cache_key` 丢失，`input` 重建；UA/originator 被 preset
  默认值替换成旧 Mac Codex 身份。这些是 Gateway 引入的差异，不是模型回答差异。
- 直连实测还有 `x-openai-internal-codex-responses-lite: true`；当时 Gateway 请求
  白名单不含它。该组 Gateway CLI 入口本身未发送此头，须区分 CLI 模型目录差异
  与 Gateway 透传能力缺口。
- 上游 SSE 实际返回 `x-codex-turn-state`，原 Gateway SSE 响应未保留。
  本次直连后续轮次未观察到重新发送该头，不能据此宣称已证续聊行为受影响。

## 修复范围

1. 已流式的原生 Responses 不需要强制流式聚合，保留 native transport。
2. 同一原生请求计划不再因为只有 responses_path 而执行 minimal pack；实际工具
   注入、协议桥接、模型映射和 provider 配置仍保留原语义。
3. 显式 `codex_cli` profile 不把 preset 默认 UA/originator 当用户覆盖；用户明确
   配置的 identity 仍优先。未显式选该 profile 的旧 preset 行为不改。
4. 精确允许客户端提供 `x-openai-internal-codex-responses-lite: true`，不生成缺省值，
   不开放任何 internal wildcard、认证或 Cookie。
5. 成功 native SSE 的候选局部状态随 TrackedStream 返回，只允许
   `x-codex-turn-state`、`x-codex-routing-hint`，长度与字符受限；重复值拒绝。
   不转发 Set-Cookie、认证、供应商余额或 hop-by-hop 字段，不改变计费/取消回调。

## 最终复测与验收边界

最终证据为 `oauth-https-entry-07`。两组均使用实际账号目录作为 CLI 的
`model_catalog_json`，关闭 apps/plugins，明确使用 HTTP/SSE；Gateway CLI 入口
也经相同测试 CA 的 HTTPS CONNECT 映射到回环 Gateway，未修改 DNS、hosts 或
系统信任库。此前 `oauth-http-05/06` 的 HTTP 回环入口触发 CLI 不同的头序/大小写，
不能把该实验条件差异误报为 Gateway 丢字段；这些历史证据保留不覆盖。

最终运行二进制 SHA256：
`335583f7b15e8d8974aac5bbf819465e307cdbd1bd8445192b537cf4fc93f632`。

- 直连 5/5、经 Gateway 5/5 成功；每组恰好一个 thread，入口/出口各五次 Responses 请求。
- 上游 Authorization HMAC 五次均一致；原始凭证文件 SHA256 未改变。
- Gateway 入口与出口的五个 body SHA256 全部一致：字段、值和原始 JSON 字节均未改变。
- 五次 UA/originator 保留；Responses-lite 头实际到达上游，不再丢失。
- 五次真实上游 `x-codex-turn-state` 在下游 SSE 中具有相同 HMAC。
  `x-codex-routing-hint` 的读取/边界有单测，但本组未观察到上游返回它，不能声称实际复测覆盖。
- 直连与经 Gateway 的上游请求：除 Cookie 外，header 名称、大小写和顺序五次均相同；
  请求体顶层字段集合五次均相同。两段独立会话的 IDs、metadata、prompt_cache_key、input
  和对应 Content-Length 不同；Gateway 入口/出口字节相同证明这些值并非 Gateway 改写。

最终仍存在、没有掩盖的差异：直连五次带 Cookie，Gateway 五次没有；直连还读取
`/backend-api/wham/settings/user`，Gateway 上游只有 Responses。通用 API Key 模式与
ChatGPT OAuth 模式的 CLI 上下文/辅助 API 行为不完全相同。不能直接转发入口 Cookie
或照抄 Set-Cookie：localhost/Gateway 与真实服务商的 cookie 域、身份和跨账号隔离边界
不同。这里未引入新的、按上游凭证隔离的 cookie session owner，不能宣称完全无感。

响应 metadata 改动仅覆盖成功 HTTP SSE，不覆盖 buffered JSON、错误响应或 WS 101；
也不透传供应商余额、认证、Cookie 和 hop-by-hop 头。本次无真实 WS 五轮证据。

验证：native 定向 65、相邻请求计划 13、profile 1、旧 preset 1、stream 生命周期 38，
共 118 项库测试通过；checker 31/31、ratchet、格式检查及 `git diff --check` 通过。
独立 `cargo build --locked --bin gateway` 为 0 errors、25 warnings；不是发布构建或全仓测试。
回执为外部 `final-build-validation.json`，最终抓包汇总为 `comparison.json` 与
`verification.json`。测试实例与 CLI 进程结束；保留隔离持久数据，不改日常实例，未提交/推送。
