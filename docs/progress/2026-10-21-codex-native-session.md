# Codex Cookie 与账号设置兼容模式

后续更新：可选模式现已扩展到满足准入条件的普通 `/v1/responses` SDK 请求；下文为
原生 CLI 阶段记录，新增行为见 [DSH 会话兼容](2026-10-21-dsh-session-compatibility.md)。

## 范围与状态

用户授权先在隔离实例验证可选原生会话模式，不改日常 CLI 配置、日常实例或原始 OAuth 凭证。
最终版本已通过 165 项不重复的相关 Rust 测试、31 项 checker 测试、ratchet、格式及 diff 检查，
并完成指定凭证的真实直连 5 轮＋Gateway 5 轮。结论限于下述实验模式与 HTTP SSE 路径，
不是正式部署、全仓测试或全部 Codex backend 接口的验收。

## 真实根因

额外一次真实直连单轮确认：CLI 在 OAuth 模式下先请求
`GET /backend-api/wham/settings/user`，收到 `__oailb`、`__cf_bm`、`__cflb`，
随后将它们带入 Responses。两个接口使用不同 TCP 连接，因此连接局部 Cookie jar 不足以兼容。
探测只保存名称、作用域属性和随机 HMAC，不保存 Cookie 明文。

已联网核验 Codex 固定 commit `be2951ea34f0d295ed0becf97079f92fa5f6950e`：
`chatgpt_base_url` 控制 backend client；`get_user_settings` 是带认证的真实 GET，
`commit_attribution_enabled` 影响 git attribution policy，不是无语义的连通性探针。

## 可选入口

仅当启动环境设置 `GATEWAY_EXPERIMENTAL_CODEX_SESSION=1` 时注册：

- `GET /codex/session/:session/:model/backend-api/wham/settings/user`
- `POST /codex/session/:session/:model/backend-api/codex/responses`

`:session` 是隔离标签，不是认证凭证；隔离 CLI 为每次新会话生成独立 UUID。
CLI 的 `chatgpt_base_url` 指向前缀中的 `/backend-api`，模型 provider `base_url`
指向其 `/codex` 子路径，保持 `requires_openai_auth=true`。
测试 HOME 内使用 Gateway owner bearer，绝不将上游 access token 作为 Gateway 认证传入。
测试 refresh token 是不可用占位值，不允许本地实验触发真实账号刷新。
不要直接覆盖日常 `auth.json`；完整配置由隔离实验脚本准备。

本版刻意限于独立实例 owner shared key、一个明确候选、官方 Codex OAuth Bearer/account
凭证及显式 `codex_cli` profile。拒绝 PG/auth-adapter 托管场景、普通 access key、
非官方 origin、非 Bearer 模式及缺少 account 的配置；这不是新的多租户凭证授权接口。
不开放 WebSocket、compact 或非流式请求；原有 `/v1/responses` 不受实验开关影响。

## 安全与生命周期

- settings 使用公共认证 owner，但不伪造推理请求，不执行推理扣费流程；成功与失败状态、
  实际响应体来自真实 settings API，不伪造设置值。settings body 上限 1 MiB、超时 10 秒。
- Responses 保留原完整管线；不匹配路径 model、不是 SSE，或候选切换到翻译 transport 时拒绝。
- Cookie 状态属于当前 UpstreamClient runtime，不是进程全局 jar；scope 绑定实际 owner bearer
  与 session label，凭证绑定包含实际上游认证、account、credential ID、operator headers 和 origin。
- 已有 session 换凭证或 origin 时拒绝，要求新 session，不把旧设置/Cookie 用到另一个账号。
- 使用 `cookie_store 0.22.0` 解析 Domain、Path、Secure、Max-Age、Expires、替换与删除。
  仅接受本 origin 的 host-only 或相同域 Cookie，拒绝父域和异域；不支持浏览器跨站导航。
- 每 Cookie 4 KiB、每 session 32 个/16 KiB、出站 Cookie header 8 KiB；最多 128 个 session，
  600 秒空闲逻辑过期，后续访问惰性清除。没有后台任务或磁盘 Cookie 持久化。
- 不复制入口 Cookie，不向客户端导出上游 Set-Cookie；显式 operator Cookie 优先。
  专用 session HTTP client 禁止重定向，防止人工附加的 Cookie 被带到另一条路径或 origin。
- 两组独立真实会话的 Cookie 值可以不同；验收检查名称、来源、更新链和作用域，不要求随机值相同。

## 验证证据目录

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-live-five-turn-20261021/`

该目录包含隔离认证文件，不得整体发布。`cookie-scope-probe-01` 是根因探测；
`cookie-unit` 直接编译同一生产 Cookie 源文件，先观察 4 项失败，再实现并验证 6 项基础测试。
`native-session-tests.log` 是首次整仓 72/72；最终版本以独立的 final 日志和真实对照回执为准。

这次工作不包含提交、推送、正式打包、日常部署或“对上游完全不可识别”的保证。

## 最终真实验证

最终目录：`oauth-native-session-08`。同一 CLI、同一指定上游凭证、`gpt-6-sol`，
每组一个 thread 连续五轮；两组均 5/5 成功。

| 检查 | 结果 |
| --- | --- |
| 两路上游调用序列 | 均为一次 settings GET，再五次 Responses POST |
| 六个请求的 header 名称、大小写和顺序 | 6/6 一致，包括 Cookie 字段 |
| Responses header / body 顶层字段缺失或额外添加 | 5/5 均无 |
| 两路使用同一真实上游认证 | settings 及五次 Responses 的 HMAC 均一致 |
| 三个实际 Cookie 的发送 | 两路五轮均有 `__oailb`、`__cf_bm`、`__cflb` |
| Gateway Cookie 值来源与更新 | 5/5 对应此前真实上游 Set-Cookie 的逐值 HMAC |
| Gateway 入口至出口 body SHA256 | 5/5 一致 |
| UA/originator 和成功 SSE turn-state 保持 | 5/5 一致，turn-state 真实观察到 |
| settings 响应体 | Gateway 上游至下游 SHA256 一致；两路 JSON 字段集合一致 |
| 上游 Cookie 向下游导出 | 无 |

仍有明确边界：Cookie 的随机值、session/thread/request/window ID、turn metadata、
prompt_cache_key、独立会话历史及相应 Content-Length 不同，不是 Gateway 改写入口 body。
首轮 Cookie 头内部键值对的顺序不同，其余四轮顺序相同；直连前一次探测的首轮顺序也不同。
未强行排序来匹配一次样本，也未证明 Cookie 内部顺序生成策略完全相同。
`x-codex-routing-hint` 未从这组真实上游观察到；此前单测不冒充真实覆盖。
MITM 证明 HTTP 字段与会话行为，不证明去掉 MITM 后的全部 TLS/连接侧信号。

本轮额外隔离检查：六个拒绝案例全部符合预期，且未产生上游请求：无认证、错误 bearer
配正确 API-key 别名均 401；settings 写入和未开放的 WS GET 均 405；模型不匹配和非 SSE 均 400。
关闭开关后的独立实例对兼容 settings 路径返回 404，普通 `/v1/models` 仍为 200。

相关 Rust 测试：native 74、request plan 13、headers 39、stream 38、profile 1，共 165 个不重复测试。
`cargo build --locked --bin gateway` 成功；24 个既有风格/未用项警告中没有新增 session/Cookie 模块警告。
本轮未执行全仓全部测试、正式 release/package 门禁或真实 WS 五轮。
新增源文件 effective lines 为 198、179、61、73、106，均低于 250，无新增行数例外。

验证使用 `target/debug/gateway.exe`，SHA256：
`a67a15096f4d7352d69f16205b371c28fbd172905acb76cf4b525d18fb423450`。
`final-safety-verification.json` 记录八个证据文件未含明文凭证、原凭证与上一轮 hash 一致、
实际测试 binary hash 与当前文件一致。测试 Gateway 与 capture CLI 已退出；未改系统信任库。

## 依赖处理记录

新增 `cookie_store = 0.22.0`，关闭默认功能，仅启用 `preserve_order`；关联新增 cookie、
document-features、litrs。处理期间一次 `generate-lockfile` 曾导致无关包升级；没有保留该
宽范围解析结果，随后以 Git 原锁文件和当前 manifest 重建最小必要解析，再加入 Cookie 依赖。
最终版本已完成上述编译、回归和真实调用。由于操作前未单独备份当时未提交的 Cargo.lock，
不能证明最终锁文件与本轮开始时的未提交版本逐字节等同；发布前仍需复核该锁文件差异。
解析中间证据保存在实验目录的 `lock-recovery`，它不是操作前锁文件的恢复副本。
