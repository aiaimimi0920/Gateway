# GitHub Copilot：独立客户端资格与原生合同调查

2026-11-08 匿名联网核对。仅源码／文档调查，尚未实现，不代表登录、目录或推理验收。
没有读取本机 GitHub/Copilot profile、登录材料或真实 token，也没有执行推理请求。

## 官方支持的自己的 App 路线

- [Copilot SDK 的 GitHub OAuth](https://docs.github.com/en/copilot/how-tos/copilot-sdk/setup/github-oauth)
  允许自己的 OAuth App／GitHub App 取得用户 token，显式交给 Copilot SDK；按用户订阅使用。
  官方示例使用 `gitHubToken` 与 `useLoggedInUser:false`，不需要借用编辑器 App。
- [GitHub Device Flow](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)
  要求自己的 App 启用 Device Flow，使用自己的 client ID；`interval` 是最小轮询间隔，
  `slow_down` 增加 5 秒，pending／拒绝／到期分开处理。
- [SDK 身份验证](https://docs.github.com/en/copilot/how-tos/copilot-sdk/auth/authenticate)
  当前列出 `gho_`、`ghu_`、fine-grained `github_pat_`，不支持 classic `ghp_`。
  SDK 的环境／已登录用户自动发现不是 Gateway 隐式读取客户端材料的授权。
- [创建 OAuth App](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/creating-an-oauth-app)
  已包含用户 token 到期选项；不能把自己的 App token 一概假设为永久有效。

官方支持自己的 App + SDK，**不等于**公开承诺任意 App 都能使用
`copilot_internal/v2/token`，更不等于允许 Gateway 声称自己是 `vscode-chat`。
后续实现须保留这个边界，不把公开常量或社区调用成功当作产品资格。

## 固定 Magpie 源码中的关键细节

来源固定到 [627fd1a3](https://github.com/yetone/magpie/tree/627fd1a3b78c65e9f3b7a838dcd2b24bb368c93d)。

- `internal/provider/copilot_signin.go:31-37,61-137`：device grant；首次轮询先等 interval，
  pending 继续、slow_down 加 5 秒，连续网络错误有上限并支持取消。原代码硬编码编辑器 App，
  **不复制该 client ID**。
- `internal/provider/account.go:1641-1675`：GitHub OAuth token 换短期 Copilot session，
  读取 `token`、`expires_at`、`endpoints.api`；session 剩余超过 2 分钟才复用。
  401 与 403 的账号／权益含义不同。GitHub token 不是 inference token，也不是一次性 refresh token。
- `internal/provider/account.go:1752-1857`：账号级 `/models` 包含 `supported_endpoints`、
  `model_picker_enabled`、capabilities、premium billing 与 policy/terms；目录不能退化成静态名称表。
  `:1678-1689` 在缺少 `supported_endpoints` 时只准入 Chat，不能猜 Responses 或 Anthropic。
- `internal/provider/account.go:1516-1543`：推理目标来自 session，Bearer 使用短期 token；
  initiator 与 vision 有独立语义。原编辑器身份头不是 Gateway 获得使用许可的证据。

社区固定包 `93dacd738f2c35981cf0364ad1bfbfe7b427a78a/packages/copilot` 只作为额外参考：
其立即且无限轮询、共享 client ID、未固定推理目标／redirect，以及成本强置 0 的做法不移植。
没有因为源码将 token 放在 `refresh` 字段，就把 GitHub token 接入单次轮换 owner。

## Gateway 后续责任层

本轮读取时 Gateway `src` 尚无 Copilot owner。需要独立的产品／App 配置、授权 session、
账号级目录／协议准入／权益状态，以及正常 Local Key 的公共请求验证；不是替换 OpenAI base URL。
以下区分两条路线：优先的官方 SDK 路线由 SDK/runtime 管理上游会话，Gateway 管理明确的
GitHub 凭据、进程／会话归属和事件适配；只有另获资格的 raw API 路线才需要 Gateway 自行
维护短期推理 session 的 typed exchange/cache、最终目的地及身份校验，不能把两者叠成双 owner。

可复用现有 Kiro/Grok 单步 device poll、管理 registry、SecretPatch/CAS、共享协议转换、
正常 Local Key／SQLite audit 测试；不能用单次 OAuth refresh 的 durable successor 语义
冒充可重复的短期 session exchange。真实 App／SDK 或获准 raw API 的资格与联网验收另列。

## 官方 SDK 路线的补充核验

同日匿名联网固定 [Copilot SDK d1e6021a](https://github.com/github/copilot-sdk/tree/d1e6021a43b26e87acfcd43b37988eff09dd8aaf)。
该源码的 Node package 为 `0.0.0-dev`、声明 CLI `1.0.108-1`；仓库自己说明发布时替换版本，
不能把 HEAD 占位号当成已发布 npm 版本。未安装包、运行 CLI 或发推理请求。

- 已有官方 Rust SDK，`rust/Cargo.toml:7,15` 要求 Rust 1.94.0、SDK 为 MIT；
  `rust/README.md:46-62` 说明 stdio Content-Length JSON-RPC 和 shutdown 后最多等待 10 秒的进程清理。
  Gateway 当前 manifest 声明 Rust 1.98.0，语言版本满足这个下限，不代表依赖已集成或编译通过。
- `nodejs/src/types.ts:573-586` 的 `gitHubToken` 与 `useLoggedInUser:false` 支持显式账号，
  禁止读取已登录用户或环境里的账号来源；`client.ts:3954-3963,4021-4026` 将 token 放专用环境变量，
  argv 仅传变量名。`mode:"empty"`、独立 `baseDirectory` 可隔离配置与缓存。
- `client.ts:3420-3456` 通过 `models.list` 读取账号目录；`types.ts:4319-4384` 包含能力、
  policy/terms、context/output 限制与 billing。SDK 不要求 Gateway 再直接调用 internal 目录。
- `session.ts:836-883` 的 send 只返回接纳后的 user message ID，结果由事件提供；
  `assistant.message_delta`、最终 message、reasoning 和 usage 需要 Gateway 自己转换。
  `generated/session-events.ts:8610` 明确缺 billing 代表 unavailable，而非零费用。
- 声明式外部工具会产生 `external_tool.requested`；
  `session.tools.handlePendingToolCall` 可由 host 返回结果，但 raw RPC 标为 experimental。
  `mode:"empty"` 要求明确 `availableTools`，不能让上游 runtime 默认执行本机 shell、文件或 MCP 工具。
  `abort`、`detach`、`stop` 分别是取消执行、分离会话、关闭 owned runtime，不互相替代。

**重要语义缺口**：已读的正式 send 只接收 prompt/attachments；experimental `session.sendMessages`
也只声明多个 user prompts，尚未找到稳定的任意 assistant/tool 历史导入合同。不能将历史拼成一个大 prompt，
然后宣称完整等价于 Gateway 的无状态 Chat/Messages/Responses。工具跨公共 HTTP 请求续接还需要
精确保有原账号、session、pending request 和 call ID 的对应关系，以及有界 TTL、并发和断连取消。
这不是简单替换 base URL 的接入，也不能以纯文本演示冒充完整工具回合验收。

[CLI 许可证固定 2e935d36](https://github.com/github/copilot-cli/blob/2e935d3612dcfac71c3a76842794e8c7d93c5fd3/LICENSE.md)
允许在有实质额外功能的应用/服务中附带原样 runtime，但要求许可、归属与独立产品边界，
禁止修改/衍生 CLI。SDK 的 MIT 不自动覆盖 CLI；CLI 分发许可也不授予服务或订阅访问权。
最终路线应优先官方 SDK/runtime，不以这些材料为 raw internal API 或编辑器身份的许可依据。

2026-11-12 已补官方 Rust SDK 的会话级身份、runtime 依赖与隔离边界，见
[SDK 原生接入切片](copilot-sdk-runtime-contract-2026-11-12.md)。这仍是合同与设计证据，
不是已实现／已编译／真实订阅验收。
