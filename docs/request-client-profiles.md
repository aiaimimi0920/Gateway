# 上游客户端参数配置

`request_client_profile` 选择 Gateway 向上游发送的客户端软件指纹及协议允许的缺省参数。
它与输入 API、上游目标协议、模型映射和账号认证分别管理：选择 Claude CLI 不会把
Responses 接口改成 Messages，也不会把 API Key 变成 Claude 订阅凭证。

## 选择规则与操作入口

| 配置值 | 行为 |
| --- | --- |
| 省略或 `auto` | Claude 模型使用 Claude CLI；GPT 及其余模型使用 Codex CLI |
| `codex_cli` | 无论模型名称，使用 Codex CLI 参数套件 |
| `claude_cli` | 无论模型名称，使用 Claude CLI 参数套件 |
| `workbuddy` | 无论入站客户端或模型名称，使用 WorkBuddy 国际版的软件身份字段 |
| `cpa` | CPA 兼容：Claude 模型使用 Claude CLI，其余使用 CPA 的 Codex TUI 指纹 |
| `cpa_claude_code` | 可选完整 Claude 身份兼容：custom origin 的 Messages API-key 路径 |

自动识别支持 `claude-*`、`anthropic/claude-*`、`anthropic.claude-*` 等模型名称，忽略大小写。
对隐藏厂商名的模型部署别名，也参考请求中原始模型名；显式选择的套件始终优先。
CPA 本身按 executor 处理，并不存在一套适用于所有协议的统一 CPA 请求参数。
这里的 CPA 兼容指纹不等于完整复刻 CPA 的 caller-owned 透传、OAuth 行为或内部状态。

CPA 两种模式现在默认使用内嵌 Rust 传输，无 Go worker 运行依赖。
`GATEWAY_CPA_TRANSPORT=rust` 仍可显式设置；旧 `go` 设置会返回迁移错误，需移除或改为 `rust`。
`cpa_claude_code` 显式启用 credential 派生身份、会话映射、system/billing/cache/beta 兼容；
不改变协议路由/鉴权，不支持借此接入官方 Claude OAuth/CCH。
非 CPA 模式不受此传输选项影响；裁剪掉 `cpa-rust-transport` 的自定义构建在使用 CPA 时明确失败。
详见 [Rust 替代验证与删除状态](progress/cpa-rust-worker-retirement-20261021.md)。

控制台新建服务商的表单，以及已有服务商卡片翻面后的详情，均有“上游客户端参数”选择框。
已有服务商的修改沿用正常配置草稿与保存流程；锁定编辑器时不可修改。
不会绕过现有配置保存流程直接写运行实例文件；不修改凭证内容，也不直接标记账号健康状态。

服务商默认与单独账号覆盖示例（与现有路由配置合并使用）：

```yaml
providers:
  - id: relay
    preset: openai
    base_url: https://relay.example
    request_client_profile: codex_cli
    credentials:
      - id: inherit
        api_key: example-key-1
      - id: claude-style
        api_key: example-key-2
        request_client_profile: claude_cli
      - id: automatic
        api_key: example-key-3
        request_client_profile: auto
```

账号省略字段时继承服务商设置；账号显式 `auto` 则覆盖服务商的固定选择、恢复模型自动判断。
控制台提供服务商级选择，账号级覆盖可通过现有配置文档编辑；未新增账号级专用控件。
旧配置无需迁移；不存在的新枚举值会被后端配置读取拒绝，不会静默应用错误的套件。

## 实际发送的参数和优先级

- Codex CLI：保留成对且有效的入站 `codex_exec` / `codex-tui` / `codex_cli_rs`
  身份；无有效原生身份时采用 Codex 0.155.1 的非交互 `codex_exec` 缺省标识。
  Windows 缺省 UA 从本机注册表只读获取 OS 版本，不再冒充 macOS；不额外生成 `Version` 头。
- Claude CLI：发送 `claude-cli` 的 `User-Agent`、`X-App`、`X-Stainless-*`
  SDK/运行时指纹，以及 `Anthropic-Dangerous-Direct-Browser-Access`。
  当前缺省来自本机 Claude Code 2.1.278 非交互调用采样，OS/架构描述当前平台；
  合法原生 Claude UA + `x-app=cli` 保留其受限 SDK 身份、Accept 和已有 beta，不额外追加能力。
  仅在 Anthropic 目标协议上合并 Claude Code 和 interleaved-thinking beta；
  仅有工具时补充 fine-grained-tool-streaming beta。既有显式 beta 保留且不会重复添加。
- CPA 兼容：非 Claude 模型采用 CPA Codex executor 的 `codex-tui/0.154.0` 身份；
  Claude 模型采用独立固定的 `claude-cli/2.1.280`、MacOS/arm64 软件身份，不随本机 CLI 更新。
  Codex Version 仅保留真实输入，补齐 JSON/SSE、压缩和 keep-alive 协商，清理跨客户端残留，
  不生成缺失会话或 OAuth 身份。协议与认证不变。参考范围和边界见
  [CPA 专项报告](progress/2026-10-21-cpa-client-profile.md)。
- WorkBuddy：参考国际版 5.8.4、内置 CLI 2.161.0 的真实回环请求，发送
  `WorkBuddy/5.8.4 workbuddy-ai/5.8.4 CLI/2.161.0` UA、`X-IDE-*`、
  `X-Product: SaaS`、`x-codebuddy-request: 1`、`X-Requested-With: XMLHttpRequest`
  及实测 SDK 软件字段。选择该套件不会触发 Claude beta 或 Codex Responses 缺省值，
  不替换目标 URL、API 协议或认证头。不虚构缺失的会话/用户 ID、隐私设置或双认证头。
  Codex 原生入站可将真实 session/turn/parent 上下文转换为 WorkBuddy 会话与 agent 字段，
  并为实际请求补齐一致的 W3C/B3 追踪；移除 Codex 专有身份残留，详见
  [Codex 上下文转换](progress/2026-10-21-codex-workbuddy-context.md)。有效原生 WorkBuddy 入站的真实会话、
  agent、隐私声明及关联追踪头通过有界名单保留，仅向显式 WorkBuddy 套件外发，
  不转发调用方认证、Cookie、用户或设备身份。相同 Chat 协议且消息未被策略修改时保留
  消息级 `agent`、`conversationRequestId`；不恢复已变更的消息或覆盖模型映射。
  HTTP/1 头名称与顺序按实测版本组织，使用隔离的请求级 Node TLS 配置而非 Chrome 仿真。
  压缩声明仅包含实际支持的 gzip/deflate/br，不宣称支持 Node 样本中的 `compress`。详见
  [WorkBuddy 风格验证](progress/2026-10-21-workbuddy-client-profile.md)。
  后续三路真实 HTTP 与原始 TLS 修复闭环见
  [WorkBuddy 网络对照](progress/2026-10-21-workbuddy-wire-comparison.md)。

这些软件标识由所选套件统一管理；除有效的原生 Codex 或 Claude 成对身份外，会覆盖同名的软件标识，
清除上一套的 `Originator`、`Version`、`X-App`、`X-Stainless-*` 等残留。
认证、追踪字段、Codex session/turn state 仍由原来的边界管理。
当 adapter 使用自定义认证头且名字与软件标识碰撞时，实际认证头优先保留，不用软件指纹替换凭证。

对于 Codex CLI / CPA 的 OpenAI Responses 请求，仅当字段缺失时补充：

```json
{
  "store": false,
  "include": ["reasoning.encrypted_content"],
  "parallel_tool_calls": true,
  "instructions": ""
}
```

已有请求值、现有生成计划中的显式值及服务商 `extra_body` 缺省设置优先。
不会覆盖用户 system/instructions、采样参数、reasoning effort、工具定义、
`previous_response_id` 或流式开关；不会把这些 Responses 字段塞进 Chat/Messages/Gemini 请求。
Claude 套件不伪造账号 UUID、订阅身份或 `metadata.user_id`，也不注入固定系统提示词。

## 适用范围和边界

通用文本 API adapter 的 Responses、Chat Completions、Completions、Messages 推理请求
共用该规则，包含非流式、SSE、ChatGPT 官方执行入口及 Gemini API 执行入口。
Gateway 本地 realtime bridge 使用同一 HTTP 推理链路；这里没有新增 Codex 上游 WebSocket 协议。
媒体、搜索、向量、管理/配额探测不自动套用。Basispoints、Kiro、浏览器逆向等产品专用通道
保留各自协议和请求参数，不被通用 CLI 指纹覆盖。

切换套件不会改变路由或站点、授予 OAuth scope、自动启用 fast/1m/redact beta、
绕过服务商权限限制或保证模型质量。若服务商依赖完整官方客户端业务语义，仍需该通道的专用适配。

## 参考依据与维护

本次于 2026-10-20 联网核对 CPA 主分支与本地 fork，并参考 sub2api 的身份边界实现：

- [CPA Codex executor](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/runtime/executor/codex_executor_request.go)
- [CPA Claude device profile](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/runtime/executor/helps/claude_device_profile.go)
- [CPA Claude fingerprint policy](https://github.com/router-for-me/CLIProxyAPI/blob/main/internal/runtime/executor/claude_fingerprint_policy.go)
- [sub2api Claude constants](https://github.com/Wei-Shaw/sub2api/blob/main/backend/internal/pkg/claude/constants.go)
- [sub2api Codex identity](https://github.com/Wei-Shaw/sub2api/blob/main/backend/internal/service/openai_codex_identity.go)

当前通用 Codex 快照为 0.155.1、Claude CLI 2.1.278；CPA Codex 兼容套件仍保持自己的 0.154.0 参考值。
Codex 缺省身份集中在 `src/upstream/request_profile/codex_identity.rs`，
CPA 的参考值集中在 `src/upstream/request_profile/headers.rs`，Claude 身份及准入名单在
`src/protocol/claude_wire.rs`；不在运行时联网追随版本。
切换上游 CLI 版本需同步参考代码与 focused 合同测试，而不是无条件复制上游所有行为。

focused 验证命令：

```powershell
cargo test --locked --test request_client_profiles_contract --test request_client_profiles_runtime_contract --lib request_client_profiles -- --test-threads=1
npm --prefix apps/desktop test -- run requestClientProfile.test.tsx usePilotPolicyEditor.test.ts
npm --prefix apps/desktop run typecheck
```

Rust runtime 合同使用本机回环服务器捕获 72 次真实 HTTP 请求，覆盖四套配置、三类模型、
Responses/Chat/Messages 和 buffered/SSE；不使用真实上游凭证，不代表真实服务商验收。

## 原生 Codex Responses 保真通道

`openai_compatible` 的同协议 Responses、Direct HTTP、Codex/非 Claude 自动模式，
现在使用独立的 `reqwest 0.12.28 + native-tls` 通道，不使用 Chrome TLS/HTTP 仿真。
协议桥接、强制流式重打包、Basispoints、浏览器产品通道仍由各自 owner 管理。
详细适用条件、原始字节保留规则与实测边界见 [Codex 网络保真](codex-wire-fidelity.md)。

显式 `codex_cli` 的原生 HTTP Responses 通道另区分普通 SDK 输入：入口存在 UA、但不构成
有效 Codex UA/originator 对时，按流式状态规范化 `Accept`，并按参考 Codex 的共同头顺序
组织现有字段。服务商显式 `Accept` 配置优先；有效原生 Codex 对继续保留入口 Accept/头序。
此规则不应用于 `auto`、compact、WebSocket 或缺 UA 的调用，也不生成缺失的 session/turn
metadata、Cookie 或 beta 能力声明。DSH 实际网页对照见
[DSH Codex profile 验证记录](progress/2026-10-21-dsh-codex-profile.md)。

## 原生 Claude Messages 流式通道

Direct HTTP、同协议 Messages、stream=true 且所选套件使用 Claude 身份时，使用无 Chrome
仿真的 HTTP client。精确匹配已测 2.1.278 缺省身份时，使用独立 HTTP/1.1 客户端与实测
TLS 套件、supported groups、OCSP/SCT 配置；其它版本使用 plain client。
只保留有界 allowlist，入口认证、Cookie、proxy 及内部控制头不外发；
出站凭证仍来自选中服务商。拒绝重定向，不把 API key 发送到重定向目标。

`x-claude-code-session-id` 可原样保留；DSH 的 `session_id`/`session-id` 可转换到此名称，
冲突或重复拒绝，不生成缺失 session/device/account identity。原始 query 仅允许单个 `beta`，
布尔 `beta=true` 不再混入 `anthropic-beta`；与服务商已有 beta query 冲突时拒绝。
未知 query、query 凭证和重复 beta 均拒绝，不使用通配透传。

新 CLI 的 messages 内 system 块可进入正常 canonical policy；未被策略改变的原生 messages
保留原始块字段。只有最终计划与入站完整 JSON 相等时才复用原始 body 字节，真实模型映射和
消息/工具变更不能被原始字节恢复撤销。不复制 CLI 固定提示词，不补造 metadata.user_id。
对于实测 2.1.278 缺省身份，HTTP/1 使用参考头名称大小写和顺序；Connection keep-alive
由出站连接池行为生成，不透传调用方 hop-by-hop 控制。其它版本不假定使用相同头序。
普通 SDK 输入的 Accept-Encoding 按 Gateway 实际启用的 gzip/deflate/br/zstd 解码能力及参考格式
规范化；原生 CLI 的显式值与服务商显式配置保留。缺失会话 ID 不凭空生成。
此通道不声称覆盖非流式、跨协议转换或 Bun TLS 指纹等同，也不新增未经观察的辅助接口调用。
实测记录见 [Claude 请求对照](progress/2026-10-21-claude-wire-comparison.md)。
后续真实原始 TLS 字节对照见 [Claude TLS 修复](progress/2026-10-21-claude-tls-fidelity.md)。
