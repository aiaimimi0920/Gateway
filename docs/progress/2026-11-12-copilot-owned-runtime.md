# Copilot SDK：正式启动入口与合成子进程生命周期

2026-11-12，状态 `owned_process_probe_pass / gateway_runtime_not_implemented`。
承接 [stream-only 兼容性实验](2026-11-12-copilot-sdk-compatibility.md)，本轮不再使用测试专用
模式构造器，实际调用官方 SDK 的 `Client::start`、`create_session` 和 `stop`。

## 本轮实际改动

仍在独立临时实验工程中，固定 `github-copilot-sdk = 1.0.31`，关闭默认 feature，新增 `runtime`。
启用 feature 时只更新临时工程的 Cargo.lock，不修改 Gateway 的依赖或锁文件。
构建进程显式设置 `COPILOT_SKIP_CLI_DOWNLOAD=1`；没有获取或运行真实 Copilot CLI。
测试还断言 `HAS_BUNDLED_CLI=false`。首次获取新增 Rust 依赖后，最终测试锁定且离线执行。

新增合成 Rust EXE 和集成实验：

- `probe/src/bin/synthetic_runtime.rs`：有界 Content-Length 请求读取；只响应预期 RPC，
  不实现工具／网络／账号读取；只记录方法名，不将 token 写到日志。
- `probe/tests/owned_runtime.rs`：显式 EXE／Stdio／Empty、禁止已登录身份回退、隔离工作目录和
  COPILOT_HOME、关闭客户端遥测，并从 child 环境移除三个常见 GitHub token 变量。
- `run_runtime.py`：固定 Rust 1.98.0、Cargo jobs=2、禁下载环境、UTF-8 日志／回执；
  每次使用新证据目录，并通过 Windows 进程 API 只读核对本轮 child PID 退出。

## 可观察到的链路

1. 正式启动指定合成 EXE，child 核对 `--stdio`、`--no-auto-login`、隔离 COPILOT_HOME，
   token 不在 argv，移除的环境变量没有出现在 child。
2. `connect` 握手通过后创建显式账号会话，核对合成 token 与空工具 allowlist。
3. SDK 的 Empty 模式真实发送 `session.options.update`；实验断言 `includedBuiltinSkills=[]`
   和 `installedPlugins=[]`。原始 options.json 也保留 custom instructions／schedule 等设置。
4. 即使创建失败，实验仍先执行 `stop`；成功链收到 `session.detach` 和 `runtime.shutdown`。
   child 回复 shutdown 后不立即退出，而是等待 stdin EOF，写入 graceful-exit 标记后退出。
5. 外层 Windows `OpenProcess`／`WaitForSingleObject` 检查 PID 已终止或不存在；
   不是只看 SDK 返回成功，也不是仅依赖 SDK 内部 PID 字段。
6. 显式不存在的 EXE 路径返回失败，不回退自动发现，不触碰真实 Copilot 安装。

这证明官方正式启动接口可用于所选配置，**不证明真实 CLI 执行了所有隔离语义**。
环境变量移除与参数交付可观察；真实账号授权、文件访问和工具权限仍需后续 runtime 验收。

## 测试结果与失败分类

最终命令：`cargo test --locked --offline --manifest-path probe/Cargo.toml`，环境固定 Rust 1.98.0。
结果 **5 passed / 0 failed / 0 ignored**：原合成流 3 项，加 owned child 和缺失路径 2 项。
最终整条 Cargo 命令约 9.09 秒，含增量编译，不作为 Gateway 构建提速结论。
Windows 核对 `childExited=true`，本轮 PID 62172。

此前失败证据全部保留：

- r1：options.update 夹具回复缺必需 `success`；核对官方 `SessionUpdateOptionsResult` 后修正。
- r2：detach 夹具回复缺必需 `success`；核对官方 `SessionDetachResponse` 后修正。
  r2 虽断言失败，仍已先停止 child，Windows 检查确认退出。
- r3：完整 5 项通过，不修改 SDK 或放宽协议断言。无网络故障伪装为实现失败。

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-sdk-compat-20261112`。
`runtime-r3.log`、`runtime-r3-receipt.json`、`runtime-r3/rpc.jsonl`、`options.json`、
`graceful-exit` 保留；源码／锁文件 hash、格式与本轮文档差异另存校验回执。

## 尚未完成

本轮没有 Gateway 生产代码／依赖修改；管理凭据、账号代次、模型目录、公共文本请求和
事件转换、用量审计及取消还未接线。没有真实 CLI、订阅、权限拒绝、工具续轮、Server 或原生包验收。
未测 hung-child 强制终止和后代进程回收，不能拿正常退出用例代替这些证据。
下一写入范围应是 Gateway 的 SDK 配置／owned runtime 适配及对应仓库测试，而非继续把
临时实验当生产实现。保留 67 项未完全关闭，不提交、推送、部署或重启运行实例。
