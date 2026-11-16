# GitHub Copilot SDK：Rust runtime 接入边界

2026-11-12 匿名联网核实，状态 `contract_reviewed / runtime_not_implemented`。
不减少 Magpie 未完成项；未安装 SDK/CLI、运行下载代码、读取本机凭据或调用真实推理。

## 固定来源与本轮增量

官方仓库固定提交：
[`540dc4e1ccd4aa174e7602465099a2aa93fab176`](https://github.com/github/copilot-sdk/tree/540dc4e1ccd4aa174e7602465099a2aa93fab176)。
GitHub recursive tree 返回 `truncated=false`；8 份公开源码／文档及 URL、SHA256 保存在
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-sdk-20261112`。
这是本次取得的 main 快照，不是已发布版本选择；`rust/Cargo.toml` 的 `0.0.0-dev` 不作版本承诺。

此前已确认自有 App + SDK 路线及历史导入缺口，本轮不重复认领。新增核实如下：

- `rust/src/types.rs:2672-2683`：`SessionConfig.github_token` 决定会话的身份、模型路由与额度，
  不同于 `ClientOptions.github_token` 的进程身份。会话级 `github_token_provider` 支持轮换，
  与静态 token 互斥；回调接收 host、可选 session ID 和获取原因。
- `docs/setup/multi-tenancy.md:552-605`：目录缓存按会话隔离；应用仍负责 session 归属验证，
  包括 resume/delete。`mode: empty` 禁用环境默认工具，不是 OS 文件系统沙箱。
  共享 runtime 的 host filesystem 在启用 host tools 后仍共享。
- `rust/src/types.rs:2443-2470`：built-in agents 的空 allowlist 与 None 不等价，重启恢复时
  需要重新传入；Empty 默认排除 bundled skills，MCP OAuth 存储默认为内存。
  这些默认值不替代 Gateway 的权限拒绝、无 MCP／工具注册和账号边界。
- `docs/setup/multi-tenancy.md:378-429`：`baseDirectory` 隔离进程配置和数据；使用 sessionFs
  时，approval metadata 的 namespace 仍由应用负责。不得把不同用户放进同一 namespace
  后仅靠不同 session ID 声称永久授权隔离。
- `rust/Cargo.toml:32-39`：默认 `bundled-cli` 包括 runtime 和 network-forwarding。
  `rust/README.md:68-85` 明确无默认 feature 可用外部 streams，且不包含 runtime 下载／发现／启动；
  `runtime` 是独立的进程管理选择。未检查最终锁文件与构建脚本前，不直接启用默认依赖。
  后续实际检查发布版 1.0.31 发现：仅关闭 `bundled-cli`、保留 `runtime` 仍可能下载／提取 CLI；
  必须显式禁止构建下载，不能把“不嵌入”当作“不联网”，见下方兼容性实验。
- `rust/README.md:60-66`：`stop()` 的 shutdown RPC、进程退出等待及最终 reap 有分开的边界，
  不能把整个停止链误写成总共 10 秒；外部 runtime 不具有 owned child 的同等关闭责任。
- `docs/setup/backend-services.md:3,72-80`：远程 TCP 是明文 JSON-RPC，不是 HTTPS；
  非 loopback 服务要求 connection token，但该 token 不提供 TLS。首切片不开放远程 runtime。

## 首个实现切片的明确范围

这是待实现方案，不是当前已有功能，也不缩减最终 Copilot 卡片范围。

1. 复用 Gateway 的显式凭据管理与 CAS；使用操作者自己的 App 或显式合法 token。
   不读取 GitHub/Copilot 本机配置，不自行消费另一个客户端的 refresh token。
2. 优先采用官方 Rust SDK 的本地 owned stdio runtime，不新增一套平行 JSON-RPC 实现。
   CLI 路径及版本显式固定，依赖与分发许可分别核对，不自动下载或升级 runtime。
3. runtime 的配置／工作目录隔离；会话使用 Empty、无内置工具／子代理／MCP、拒绝权限请求。
   token 只进入受控身份通道，不出现在 argv、日志或普通错误响应中。
4. 会话绑定 Local Key、provider/credential ID、凭据代次、模型和随机内部 session ID。
   不接受用户提交的裸 SDK session ID 直接 resume；发送前沿用 Gateway 当前身份校验。
5. 第一条公共路径仅准入已证实可表达的单轮纯文本，明确拒绝历史 assistant/tool、媒体和
   未证明可保真的参数；不能悄悄拼 prompt。事件完成／失败／取消及未知用量遵循公共协议和审计。
6. runtime/会话数量、排队和存活时间有界；请求断连取消执行，随后关闭会话；shutdown 等待 owned
   child 回收。并发请求不得共用可变对话；不把 detach 当取消，不隐藏重放付费请求。

## 实现前后必须拿到的证据

- 固定可交付 SDK/CLI 版本、许可与构建行为；先验证 SDK 适配，不以下载 main 源码代替依赖验证。
- 使用合成 stdio peer 覆盖 handshake、事件终态、错误、取消、超限与退出回收；不发真实账号请求。
- 两账号并发验证会话 token/目录/用量不串；旧代次发送拒绝；本机环境 token 不被拾取。
- 管理入口到公共文本请求的完整本地测试，而非只给一个孤立 parser 或 preset 打通过标记。
- 工具跨 HTTP 续轮及任意历史导入合同另行实现／验证；本轮没有确认稳定的历史导入 API，
  也没有证明其永远不可实现。真实订阅、费用、原生 EXE 与 Server 验收继续单列。

## 本轮检查

仅变更本合同、原合同责任边界和工作板；未更改生产代码、依赖或完成数量。
源码归档 SHA256、文档 UTF-8 无 BOM／尾随空白与本轮 scoped diff 作为本地检查；
未运行 Rust 测试或编译，不以文档检查代替实现验收。

后续同日已执行独立 Rust SDK 编译和 3 项合成流实验，见
[发布版兼容性证据](../progress/2026-11-12-copilot-sdk-compatibility.md)。实验不改变上述
Gateway runtime 未实现状态；本节“未运行”仅描述前一合同批。

同日后续已启用发布版 `runtime` feature 并显式禁下载，通过正式 `Client::start` 完成
合成 owned EXE 的 Empty 配置、隔离目录、会话创建、shutdown／stdin EOF 与 Windows 退出核对，
最终实验 5 项通过，见 [owned runtime 实验](../progress/2026-11-12-copilot-owned-runtime.md)。
没有使用 test-support 替代正式 API；真实 CLI 与 Gateway 接线仍未实现／验收。
