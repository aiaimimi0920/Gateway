# Gateway Copilot runtime owner

Gateway 的可选本地 Copilot SDK 进程所有权层。固定 SDK `1.0.31`，不分发 CLI，
本 crate 不提供 provider 识别、登录、refresh 或公共推理路由。根工程使用 `copilot-sdk-runtime`
feature 显式启用；默认构建不启用它。

根工程已接入独立的受管理鉴权保护的临时 token 只读目录入口，配置与验证边界见
[管理目录进展](../../docs/progress/2026-11-12-copilot-management-catalog.md)。它不保存凭据，
也不把 GitHub token 发送到通用 HTTP provider。

根工程另支持通过原有 SecretPatch／CAS 保存专用 SDK 身份，并按配置 revision 从已保存
账号读取目录，见 [保存身份与代次校验](../../docs/progress/2026-11-13-copilot-saved-identity.md)。
存储字段不进入通用 HTTP payload，公共推理尚未接通，专用账号仍不可路由。

## 调用边界

- `RuntimeConfig` 的 EXE 和隔离目录必须来自受信任运维配置，使用已存在的绝对路径。
  上层负责每个 owner 的目录隔离及固定、可信的 CLI 版本；不要把用户请求字段映射到 EXE。
- `start` 使用官方 SDK 的 Stdio、Empty，禁已登录账号回退，移除环境 token，禁客户端遥测。
  不自行创建目录、寻找 CLI 或读取用户 profile。
- `prepare_text_session` 接收上层选择且已检查代次的明确 token／model，一个 owner 只准入一个会话。
  先对返回值 `subscribe`，再 `start().await`。内置工具／子代理清空，权限请求统一拒绝。
- `stop().await` 负责正常结束；owner 提前 Drop 时通过 SDK `force_stop` 关闭，
  不让持有 session 的任务延长 child 生命周期。不得将 Drop 当作优雅退出证据。
- 返回的 SDK session 是受信任内部接口，不可直接暴露成公共 RPC。历史导入、工具续轮、
  事件到公共 wire 的转换、用量／计费／取消、全局并发上限由后续 Gateway 接线负责。
  错误原因仅供内部诊断；跨日志／API 边界仍需调用 Gateway 脱敏逻辑。
- `list_models_for_token` 为受信任调用方读取明确账号的模型目录，使用固定 SDK 的 experimental
  `models.list` typed RPC；不用进程账号或 `Client::list_models()` 的全客户端缓存。
  每次调用都显式传入 token，空 token 在发送前拒绝，不做重试或缓存；policy、terms、能力和
  billing 按 SDK 类型保留，缺 billing 仍为未知，目录可见不代表获准推理。调用方负责当前账号
  代次检查、超时／取消、全局并发限制及结果发布权限；本接口不接收 HTTP 用户的裸 token。
  取消目录读取后仍须关闭 owner，不把丢弃 RPC future 当作上游已停止或 child 已退出。

## 构建与验证

`complete_text` 提供消费 owner 的单轮聚合文本执行，正常返回等待 stop，外层取消通过
owner Drop 交由 SDK force_stop。仅返回完整单条根 assistant 文本；最后消息的可选输出
token 数不是整次请求计费证据。它尚未接到公共推理，详见
[文本执行与取消验证](../../docs/progress/2026-11-13-copilot-text-execution.md)。

从 Gateway 仓库根目录执行，`.cargo/config.toml` 强制设置 `COPILOT_SKIP_CLI_DOWNLOAD=1`。
若在其他工作目录或外部仓库使用本 crate，必须自行设置该环境变量；Cargo 不会自动读取依赖仓库
的 `.cargo/config.toml`。仅关闭 bundled-cli 并不能阻止 SDK runtime 构建下载。

```powershell
$env:CARGO_BUILD_JOBS = '2'
$env:GATEWAY_PROBE_ROOT = 'C:/Users/Public/nas_home/AI/GameEditor/linshi/copilot-owner-test-<unique>'
cargo test --locked --offline --manifest-path crates/gateway-copilot-runtime/Cargo.toml --features test-fixture
```

使用未占用的证据目录。`test-fixture` 只构建本仓库的合成 EXE，不是 SDK 的 test-support，
不是生产 CLI；默认不启用该 feature。默认 crate 测试运行配置单测，以上命令另运行真实
child 的 owner 集成测试。`child.pid`／`drop/child.pid` 是本轮 PID，调用方应在测试后只读
确认二者退出；本次 Windows 核对证据见进展文档。

当前不承诺真实 Copilot CLI 的权限隔离、订阅调用或 Server 可用性。
