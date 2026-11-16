# Luma OAuth 单次刷新协议底座

本轮仅实现 refresh 请求／响应底座，未启用账号刷新、未新增持久秘密、未申请离线权限，也未调用真实刷新或生成服务。

## 联网依据

2026-11-14 匿名读取 [Luma OAuth 元数据](https://agents.lumalabs.ai/.well-known/oauth-authorization-server)，观察到：

- issuer：`https://agents.lumalabs.ai`；token endpoint：`https://auth.lumalabs.ai/oauth2/token`。
- grant types 包含 `authorization_code`、`refresh_token`、device code、token exchange。
- scopes 包含 `email`、`offline_access`、`openid`、`profile`；token endpoint auth methods 包含 `none`。
- 同时读取 [RFC 6749 第 6 节](https://www.rfc-editor.org/rfc/rfc6749.html#section-6)：刷新请求不应扩展原授权 scope；新 refresh token 应替换旧值。没有新 refresh token 时保留原值，不把 malformed 字段视为省略。

公开 discovery 只证明服务器声明支持，不证明当前已登记 client 或账号已获得 refresh 权限。当前 Gateway 注册只声明 authorization_code；不能通过修改 scope 就假定旧 client 自动支持刷新。

## 代码范围

- 新增非 Clone、消费自身的 `RefreshGrant`，接收经过验证的固定 Luma context 和显式 refresh secret。只发固定 endpoint/resource 的 public-client form，无 client secret、Authorization 默认头或新增 scope。
- 单次总预算最多 15 秒；响应最多 64 KiB 解码数据；不跟随重定向、不重试、不创建后台任务。过期 access metadata 仍可用于显式 refresh；refresh secret 本身不从 access expiry 推导寿命。
- 新 refresh 值替换旧值；仅字段省略才保留原值。null、空、非字符串、重复或非法 refresh 字段拒绝，不降级成功。结果对象不 Serialize，Debug 脱敏；只提供显式消费结果的接口。
- access 继续复用原 `TokenMaterial` 的严格校验及发送时间锚定，未知 expiry 保持未知。抽取授权码／刷新共用的有界响应读取，保留旧授权码交换的行为和原测试。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-refresh-core-20261114`。

- `red.log`：先以未实现刷新发送的占位方法运行成功轮换场景，按预期失败，再实现完整请求；这是新增 API 的测试先行，不冒充旧生产入口已支持 refresh。
- 新增 8 项定向测试，覆盖过期 access 下刷新、新旧 refresh 选择、精确 public-client form、默认凭据隔离、秘密脱敏、非法材料、可达跳转目标、错误不重试、响应上限、超时和取消。
- `tests.log`：新增 8 项刷新测试全部通过。较宽 Luma 回归为 **150 passed、1 failed、2 ignored**；失败项是既有 Agents caller-drop 测试，固定等 200 ms 后要求已到达下一 HTTP 阶段，实测只有 1 次请求而预期 2 次。
- 针对该已证同步缺陷，仅将 `upstream/luma_agents/pipeline_failures.rs` 中的取消触发改为：持续 poll 原调用，最多 3 秒等真实请求计数到达目标阶段；提前完成或不到达仍失败。原精确请求数、仅一次 POST、退款、取消 audit、健康和并发释放断言不变，不修改生产时限或重试语义。独立只读复核未发现该改动掩盖业务错误。
- checker 31 项、ratchet、scoped rustfmt 及原 5 项源码哈希／UTF-8／空白检查通过；所有本批代码文件均低于 250 有效行。修正测试后用 `final-scope-hashes.json` 固定 6 项源码，不改 baseline／policy／例外。
- `cancellation-recheck.log`：修正后的取消用例 **1 passed、0 failed**，执行 0.56 秒。未变化且已通过的其余测试没有重复运行；不把首轮宽回归改写成一次全绿运行。
- `production-check.log`：`cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` exit 0。仍有 warnings，不声称零警告；构建时间包含共享锁等待，不作为纯编译耗时或提速数据。
- 最终 6 项源码哈希、UTF-8 无 BOM、范围空白与文档链接复核通过。未提交、推送、出包或修改真实凭据、数据库和运行实例。

## 后续接线与边界

1. 明确用户同意的 offline 授权及 client 注册 grant 能力，保留旧 client／回执兼容性，不能擅自扩大当前 openid 授权。
2. 登录材料通过正常 `console/luma_oauth/persistence.rs`、SecretPatch 保存；只按明确合同调整 `routing/config/luma_mcp.rs`，不放开独立 Agents API 的闸门。
3. 复用现有 `console/runtime/refresh_grants.rs`、`token_refresh/grant.rs` 的永久 Sending 占用和 received/result 发布机制；把 Luma context 纳入精确材料绑定及轮换发布。协议对象单次消费不替代跨进程防重放。
4. 当前 disabled MCP 账号不会进入后台自动刷新；不能简单启用账号来绕开此边界。未知 expiry 的 Result／Server cache 合同也须明确，不填造寿命。

本轮未改注册 grant_types、登录 scope、账号保存或后台调度。因此不会让现有账号自行刷新，不作为正常管理入口、真实供应商、Server 或原生包验收；不减少整产品未完成数量。
