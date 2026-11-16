# Copilot 显式账号模型目录

2026-11-12，本块实现 runtime 内部目录读取；管理 API、凭据持久化和公共推理仍未连接，
不把该块计作 Copilot 整卡完成，67 个工作项未完全关闭。

## 实现与边界

- `CopilotRuntime::list_models_for_token` 经正式 owned stdio runtime，向 SDK typed RPC
  `models.list` 显式传入 `gitHubToken`。不使用进程账号、不读取本机登录、不增加 raw HTTP owner。
- 固定发布版 SDK 1.0.31 的 `src/lib.rs:3931-3977` 明确：普通 `Client::list_models()` 使用
  进程身份并终身缓存，而会话 token 不会改变它；官方建议显式账号走 `list_with_params`。
  本次依据已归档的发布版源码实现，不宣称重新联网核实了最新版本。该 typed RPC 属于
  experimental，升级 SDK／CLI 时需重新验证，不能把它暴露成任意公共 RPC。
- 返回完整 SDK Model 类型，不压成静态名称列表；保留 policy／terms、能力限制和可选 billing。
  不自动接受条款／启用被禁模型，不把缺价格当作零，也不把目录可见当作推理授权。
- 空 token 在发送前拒绝；失败保留内部 SDK source，外层 Display 固定，不把错误变成空目录。
  不重试、不同账号不共享 SDK 目录缓存，不消费生成会话名额，不触发 `session.send`。
- 账号选择、代次校验、并发上限、HTTP 授权与目录持久化仍属于待接线的 Gateway 上层；
  调用方负责超时／取消并关闭 owner。取消 future 不等于上游取消，关闭仍由 SDK owner 负责。

## 验证

先以普通 `Client::list_models()` 实现运行回归，合成对端因缺少明确身份返回拒绝，
得到 **1 failed**；改为显式 token 的 typed RPC 后仓库 crate **7 passed / 0 failed / 0 ignored**，
其中新增 2 项目录集成测试，其余 5 项覆盖共同修改的 owner／合成进程路径。

新增测试经正式 Client::start 启动合成 EXE，验证：

1. 同一 runtime 并发读取 A／B、再次读取 A，各自目录正确且实际发出三次 RPC，不复用全局缓存。
2. policy disabled／terms、输出限制和价格倍数保留，B 缺 billing 不补零。
3. 空 token 不发送；合法空目录成功，RPC 拒绝与畸形模型失败，安全 Display 不透出内部原因。
4. 目录读取后仍能创建唯一生成会话，但没有发送生成请求；日志只记录方法名，不落 token。
5. 挂起目录由调用方超时取消，只发送一次；随后 runtime shutdown／EOF 正常完成。

红灯和绿灯的共 5 个合成 child PID 都由 Windows OpenProcess／WaitForSingleObject 核对已退出，
包含正常目录、取消目录、既有 stop／Drop 和红灯失败退出，不只依据 Rust future 返回。

有效行：owner **66 → 88**，合成主程序 **80 → 92**；新目录测试 **94**、目录夹具 **22**。
无新增依赖，不改 baseline；没有真实账号、真实 CLI、费用或原生／Server 验收。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-catalog-20261112-224244`。
`red.log/json`、`green.log/json`、`process-exit.json` 保留失败／成功及进程证据。
根 feature 编译（原 target、jobs=2、锁定离线）及 crate formatter 检查通过，checker 测试
31 passed；effective-line ratchet 通过，扫描 5490 文件，governed source >700=0。
最初给 checker 的 `--json` 传了绝对证据路径，被其 repository-relative 参数合同拒绝（exit 2），
未改 checker；随后使用官方 `npm run check:effective-lines --prefix scripts` 成功（exit 0）。
`gates.json` 保留原失败，`ratchet-corrected.log/json` 保留纠正后的独立回执。
本轮源码／文档 UTF-8 无 BOM、scoped diff 检查通过，根 Cargo.lock 与本轮 before 完全一致。
没有重复前批不受影响的 28 项 HTTP 回归，没有提交、推送、部署或出包。
