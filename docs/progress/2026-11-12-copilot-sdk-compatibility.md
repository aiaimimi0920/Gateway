# Copilot 发布版 Rust SDK 兼容性实验

日期：2026-11-12。状态：`sdk_probe_pass / gateway_runtime_not_implemented`。
本轮从合同阅读推进到真实编译／合成协议执行，不代表 Copilot 产品或公共推理已接通。

## 版本及证据

从 crates.io 查询并固定 [`github-copilot-sdk 1.0.31`](https://crates.io/crates/github-copilot-sdk/1.0.31)。
下载 crate 的 SHA256 已与 registry 返回的 checksum 一致；版本元数据、原始 crate、解包源码保留。
GitHub latest release API 返回 `403: rate limit exceeded`，没有将该请求算成功，也未继续重试。
本轮不把 main 的 `0.0.0-dev` 当依赖版本。

实验全部位于：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-sdk-compat-20261112`。
`probe/Cargo.toml` 固定 SDK 版本且 `default-features=false`；独立 Cargo.lock 保留。
这是一次性实验工程，不是 Gateway 第二套生产构建，不修改 Gateway Cargo.toml/Cargo.lock。
只下载编译依赖，没有下载、安装或启动 Copilot CLI。

## 已实际执行

- 首次临时目录构建选中环境默认 Rust 1.95.0：2 项握手测试通过。不能将它当 Gateway 工具链验证。
- 随后显式 `cargo +1.98.0 test --locked --manifest-path probe/Cargo.toml`，2 项通过。
- 增加事件实验后，第一次 2 passed / 1 failed：夹具误写 `githubToken`，官方 wire.rs 明确字段为
  `gitHubToken`。只修复实验断言，不修改 SDK，不降低身份断言；失败日志保留。
- 最终 `cargo +1.98.0 test --locked --offline --manifest-path probe/Cargo.toml`：
  **3 passed / 0 failed / 0 ignored**。stdout/stderr 与 exitCode／耗时分开保存；Cargo 并发为 2。
- 实验源码执行官方 `cargo fmt`，最终检查 UTF-8 无 BOM、格式、源文件 hash 与锁文件。

3 项分别验证：支持版本的 `connect` 成功，不支持版本拒绝且关闭流，以及 `prepare_session`
预先订阅能接到 `session.create` 响应之前的 delta/idle 事件。第三项还断言会话 token、session
绑定及空工具列表的真实序列化字段。所有流由 Tokio duplex 合成，无 TCP、真实账号或推理。
关闭验证是 `force_stop` 后对端 EOF，**不是**真实 child graceful shutdown/reap 验收。

## 阻止直接照搬实验的两个发现

1. 发布版 `build/in_process.rs:56-88,106-113`：`runtime` 并不等于禁下载；即使关闭 bundled-cli，
   构建仍可走 runtime 下载／缓存。显式 CLI 路径部署还必须固定构建禁下载策略。
   `COPILOT_SKIP_CLI_DOWNLOAD` 是源码明确支持的 hard opt-out；本轮后续命令设置此变量，
   但没有启用 runtime feature，故不能宣称已验证 runtime feature 的完整构建。
2. `src/lib.rs:2465` 的正式 `from_streams` 使用默认模式；`src/mode.rs:30-35` 默认是 CopilotCli。
   可指定 Empty 的 `from_streams_with_mode_for_test` 位于 test-support/test 门控，不能拿它当生产接口。
   本轮使用普通 streams 只证明 SDK 协议／事件可用，不证明 Empty 或真实工具权限隔离。

因此下一生产适配应评估正式 `Client::start` + 显式 CLI 路径 + Empty 配置与禁止构建下载，
先用合成 child 验证进程管理和配置，再接 Gateway 的凭据／公共请求层。
不为绕过 API 限制启用 test-support、不复制整份 SDK、不手写平行 RPC owner。

## 未完成边界

实际 CLI 启动／退出、Empty 隔离、权限拒绝、两账号并发、目录、公共 Chat/SSE、计费／取消、
管理 UI、历史导入与工具续轮均未在本轮实现或验收；真实订阅、Server 和原生包也未验证。
未运行 Gateway 全量测试／有效行 ratchet，因为仓库只改文档；没有触碰生产代码、依赖、
真实凭据、运行实例或 S06。未提交、推送或出包，67 项未完全关闭的数量保持。
