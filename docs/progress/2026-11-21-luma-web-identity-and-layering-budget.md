# Luma Web 身份传递与分层预算复核

日期：2026-11-21（Asia/Shanghai）。本批有一个已修复的 Web 实现缺陷，以及新的真实接口／原浏览器证据；没有完成 Web 自动续期，也没有实现 MCP 分层。

## 改了哪些内容

账号配置中的 `User-Agent` 原先没有进入 Rust 的 Luma 浏览器输入，虽然 JavaScript worker 已支持该字段。本批将它贯通到本机 worker stdin、远端 executor 请求及远端重新构造的 worker stdin。读取配置时兼容 HTTP 头名称大小写；未配置该头的旧账号仍使用浏览器默认值。

改动位于 `src/upstream/lumalabs_response_helpers.rs`、`src/upstream/lumalabs_runtime_helpers.rs` 和 `src/upstream/browser_worker_types/media_workers.rs`。新增的 `tests/browser_identity.rs` 从真实请求计划开始验证两条传递路径与旧账号行为；已有 `tools/luma/web-worker-check.mjs` 增加浏览器实际网络请求的 User-Agent 断言。没有新增未接入产品的 Cookie 解析器或旁路凭据写入。

## 验证

- 修复前定向回归：**1 failed**，worker JSON 的 `userAgent` 实际为 `Null`，期望为合成账号的浏览器标识。最初一次使用短名和 `--exact` 导致 0 项匹配，未计作红／绿证据；随后以完整测试名重跑并得到上述失败。
- 修复后 `upstream::lumalabs` 定向 Rust：**60 passed / 0 failed**。包含依赖编译，耗时 671.55 秒；没有修改 Cargo profile 配置或升级依赖。
- 生产 worker 的真实 Edge loopback：**2 passed / 0 failed**。正常产物和 POST 307 拒绝都保留指定 User-Agent；仅原任务产物、单次提交、SSE 关闭断言继续通过。所有请求指向本机合成服务，不使用个人浏览器 profile 或真实 credits。
- 有效行 checker：**31 passed**；ratchet、`cargo fmt --all -- --check`、`git diff --check` 通过。受治理源码 >700 的文件数为 0。
- `lumalabs_runtime_helpers.rs` 有效行 470 → 486，`lumalabs_response_helpers.rs` 143 → 150，`media_workers.rs` 209 → 211；新增回归 60 有效行，浏览器验收工具 115 有效行。没有触及规模例外。

## 原 Edge 的只读轮换证据

继续读取用户本机原有的 DevTools Network 录制，选中 `GET /api/vespa/teams/<team>/connectors`，核对到 `200 OK`。请求和响应各有一条 `wos-session`，只在内存中比较值，确认发生变化。响应 Domain 为 `app.lumalabs.ai`、Path 为 `/`、HttpOnly、Secure、SameSite=Lax，Cookie 保存期为 400 天。

本批没有为此发起新的官网请求，没有导出 Cookie／HAR／真实 team ID。400 天不是内部 access/refresh token 生命周期。该证据只证明普通 GET 可以携带轮换后的会话，不能证明过期恢复或 Gateway 持久写回。脱敏记录见 `docs/evidence/luma/gateway-luma-web-identity-20261121/web-get-rotation.json`。

## MCP 目录、余额和分层核实

初次直接目录读取返回 HTTP 401；正常管理接口返回 `luma_mcp_material_unavailable`。随后通过 Gateway 已有显式刷新流程刷新并保存唯一的已存 MCP 账号，状态为 `succeeded`。正常管理 API 的目录／模型读取均为 200，26 个工具与 2026-11-20 归档逐项相同。`RunAssetOutput` 仍没有 layer 布局字段，不能拿 REST 输出 schema 代替它。

真实 MCP `luma_usage` 的账号余额为 **5 credits**。按用户已有的付费测试授权，使用不含私人数据的合成几何 PNG，创建预算上限 5 credits 的独立会话并调用一次 `luma_generate_layering`。调用返回 JSON-RPC 错误，临时调查工具没有保留原始错误明细，未得到 run ID；不得将这个错误直接写成 402 或额度拒绝，也不重发该提交。

只读补查原会话没有事件，`total_credits=0`、`unresolved_event_count=0`、`jobs=[]`。结束这一个调查会话的调用返回 `closing`，随后账号余额仍为 5。再次只读确认结束状态时，访问凭据已返回 401，因此没有把 `closing` 改写成已结束，也没有再刷新或重新提交。

随后联网读取[官方分层指南](https://docs.agents.lumalabs.ai/guides/images/layering/)：`1k` 75 credits／次，`2k` 105 credits／次，均与层数无关，页面说明是可能变化的 introductory rates。官方 [MCP 与 API 对照](https://docs.agents.lumalabs.ai/agents/mcp-and-the-api/)说明 MCP 使用同一套 sessions、runs 和 apps。当前 5 credits 低于公开最低分层价格，无法取得成功分层样本；按用户允许额度不足部分暂不开发的范围，将该项暂缓。价格依据不是此次 JSON-RPC 错误原因的证明，也不是 MCP 分层已实现／已验收的证明。

## 仍需完成的工作

按当前额度豁免，仍有 **1 个必须继续实现的功能包：Web 自动续期与轮换持久化**。原本两个未完功能中的 MCP 分层保留为暂缓，产品能力没有凭空增加。

Web 尚缺：从正常 browser worker 接收轮换材料、绑定原账号／目的地／realm／User-Agent 后持久保存、防止旧响应覆盖新材料，以及保存失败／重启时的恢复。这些是同一功能包的必要环节。现有 keepalive 成功 probe 会丢弃 Set-Cookie，通用 runtime material 写回只涉及 Redis，不能将其当作 Local／Server 完整持久化。现有 OAuth grant/receipt/publication 有恢复机制，但还没有适配 Web Cookie；本批没有用伪造 refresh token 或 Cookie 保存期冒充 OAuth 生命周期。

本批未构建新 EXE、未替换运行程序、未提交／推送 Git。此前 `gateway-devtest-20261121-luma-complete-gaps` 保留，**不包含本批 User-Agent 修复**。源码测试回执与源文件哈希见 `docs/evidence/luma/gateway-luma-web-identity-20261121/verification.json`。
