# Copilot 公共请求在途禁用账号

本块仅新增集成测试，复用已有身份复核与停止机制，没有修改生产逻辑。
测试使用合成 CLI，不访问真实订阅账号，不修改运行实例。

## 验证路径

从管理 Secret Grant／SecretPatch／CAS 保存配置开始，以真实 Local Key 调用公共
router；请求到达合成 runtime barrier 后，经管理 PUT 禁用 credential，再释放 barrier。
不是手造 PreparedAttempt，也不是直接改测试内存身份。

- Chat 启动阶段：观察 connect 后禁用，公共请求返回 503、不发布答案。独立 child
  RPC 记录确认 sessionCreates=0、sends=0、shutdowns=1，启动后未进入生成。
- Responses 生成阶段：观察 session.sendAndWait 后禁用，公共请求返回 503、不发布
  答案。独立记录确认 sessionCreates=1、sends=1、shutdowns=1。不能撤回已经发送的请求。
- 两项均先预扣消息额度 2→1，失败后退还为 2；后续同 Key 请求返回 403、额度不变。
  403 是本夹具 providerIds 授权与空候选共同作用的结果，不推广为所有无候选场景。
- 每项两条审计均非 completed，usage／上游状态保持未知，attempt 为 1／0；workspace
  清空，Windows 进程核验确认两个 child 均已退出。这是确认 stop 路径，不是取消隔离路径。

## 实际回执

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-public-revocation-20261113`。

- `startup.log`：1 passed / 0 failed / 0 ignored，编译 14m01s，测试 0.35s。
- `generation.log`：复用同一测试 EXE，1 passed / 0 failed / 0 ignored，测试 0.61s。
- `startup-process-exit.json`、`generation-process-exit.json`：独立 RPC 计数及退出证据；
  主线程另行断言上述计数。这些计数不是 Rust 测试内置断言，复跑验收需同时检查 probe。
- `binaries.json`：测试 EXE 与合成 CLI 的 SHA256。不是产品包或原生用户验收。
- checker 31 项与 ratchet 通过；本块源码 scoped rustfmt／diff 检查通过。
  public_enabled 为 192 有效行，新增 public_revocation 为 145，有效行及哈希见 source-final.json。

两项 opt-in 测试须各自使用新 probe／workspace 根，在独立测试进程中执行；没有将 ignored
计为通过。未重跑未变化的历史全套测试。编译仍有既存 warnings，不声称零警告。

## 保留边界

显式 SDK feature 的受限公共路由保持开放条件，默认构建仍禁用。本块不证明 Key revoke、
凭据轮换、公共断连取消、真实 CLI／订阅账号、Server 或桌面产品包。没有提交、推送、
出包或部署，不增加整卡完成数。
