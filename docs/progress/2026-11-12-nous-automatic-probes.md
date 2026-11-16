# Nous 自动模型测试调度验收

后续已修复并验证发布轮换／禁用后的旧快照发送边界，见 [发送前凭据轮换保护](2026-11-12-nous-probe-rotation.md)；发送已开始后的轮换仍未独立验收。

## 本批完成内容

在专属 sender 与管理入口已验证的基础上，新增 Nous Subscription／Welcome 的自动测试调度合同测试。复用生产 `start_provider_credential_probe_scheduler`、现有管理测试夹具、LocalRuntime 和隔离 loopback 上游；没有新增调度器，也未修改生产刷新或推理逻辑。

- 两个目标各配置 active、disabled、关闭自动策略的 off 账号，以及绑定 active 的 named 计划；每个账号均使用独立合成 typed OAuth 凭据。
- active 的账号策略与 named 计划分别保存结果，mode 为 automatic，policySource 分别为 account／plan:named；模型别名解析到 `org/model`，真实 sender 保留 Nous 身份头和精确 access token。
- 实际启动原有 60 秒定时循环，首轮四次调用、到期第二轮再四次；期间立即 sweep 不重复执行。没有暂停时钟、手动调用替代第二轮 timer 或修改数据库 next-due 时间。
- 关闭并重开同一路径的 LocalRuntime 后，四份结果仍可读取、持久执行间隔仍生效，不立即补发。这是存储／状态重开证据，不是原生 EXE 进程重启验收。
- 401 时四个到期计划分别只发送一次，保存 failed 结果，审计用量保持未知，不触发隐式 refresh 或 inference 重放；同一执行间隔内再次 sweep 不发送。
- 关闭所有自动策略后 scheduled_count 为 0；启动前已请求 shutdown 时调度器及时退出，无上游请求或审计。disabled／off 账号无自动测试结果。

## 验证结果

`cargo test --locked --offline --lib -- nous_scheduler --test-threads=2`

**4 passed / 0 failed / 0 ignored**：其中本批新增调度测试 3 项，另命中已有 `token_refresh::tests::nous_lifecycle::nous_scheduler_preserves_receipt_before_publishing_the_exact_successor` 1 项，不计作新增。

测试执行 60.81 秒；完整命令 1065.22 秒，包含共享 Cargo 锁等待与编译，不能当作纯编译耗时。上一批管理 2 项与更早 Nous 64 项的结果分别保留，未把不同批次合计成一次运行。

有效行 checker 31 项、ratchet、scoped rustfmt、git diff --check 均通过。调度模块仅新增测试注册，174 → 176 有效行；新测试 202 行、夹具 135 行。三个 Rust 文件测试前后 SHA256 一致、UTF-8 无 BOM；scoped.diff 使用本轮修改前快照，未将继承 dirty edits 纳入本批。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-scheduler-20261112`。保留命令日志、receipt、源码快照、哈希和 `final-validation.json`。

## 仍未验证

真实独立 client 资格／账号／订阅／费用、调度期间并发授权轮换、原生 UI／EXE、真实进程重启、Server／Redis 仍开放。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。

本批关闭自动模型测试上述本地证据缺口，不等于 Nous 整卡完成；全范围仍为 67 个工作项未完全关闭。
