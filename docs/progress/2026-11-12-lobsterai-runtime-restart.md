# LobsterAI 调度运行时重建恢复

## 已完成

在隔离 LocalRuntime 中执行账号策略与命名计划首轮后，完整关闭旧 refresh worker、SQLite 连接和 AppState，使用原 Config 与原文件再次调用 `runtime::build_app_state`。不重写路由，不重新创建普通 Key，不复制测试结果或到期槽，不修改 SQL 到期时间。重新启动现有 refresh owner 并等待正常 dispatch readiness。

新增测试确认：

- 旧 AppState 的 Weak 引用无法升级，旧 owner 已释放。
- 原 Key ID 和余额保留，路由文件字节不变。
- 账号和命名计划的完整序列化结果均与关闭前一致；两条持久到期槽的 ID／next 值完全一致。
- 重建后立即 sweep，两个目标均因未到期跳过，仍只有首轮两次上游请求。
- 启动真实 scheduler，等待原到期时间与真实 60 秒 timer，第二轮正常执行，两个槽保持原 ID 并更新 next。
- 总共四次固定产品路径、精确账号与配置模型调用，四条完成审计均有真实 5 tokens，普通 Key 余额保持不变；最后请求停机并等待调度器结束。

本批只修改测试：夹具提取原有刷新就绪逻辑供首次启动和 restart 共用，并增加 restart 方法；生产逻辑、持久化格式、调度规则未改。

## 验证

因涉及共用夹具，运行相关完整集合：`cargo test --locked --offline --lib -- lobsterai --test-threads=2`。

**93 passed / 0 failed / 0 ignored，其中本批新增 1 项**。测试 107.19 秒，包含真实 timer 等待；完整命令 1344.67 秒，包含共享构建锁等待和编译，不能当作纯编译时间。

官方有效行 checker、ratchet、scoped rustfmt、差异／空白检查通过。夹具 226→247 有效行，scheduler 测试模块 179→180，新测试 93。源码测试前后 SHA256 一致，UTF-8 无 BOM；未修改 baseline 或例外。before、scoped.diff、命令回执和最终来源检查在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-restart-20261112`。

## 边界

这是同一测试进程中的完整运行时关闭／重建，不是原生 EXE 退出重启或进程崩溃恢复。仅合成凭据与不转发的 loopback 上游，不等同真实供应商、费用或身份头接受性验收。运行中修改策略／禁用账号、自动调用与授权轮换组合、Server/Redis 和原生包仍开放。

全范围仍有 67 个工作项未完全关闭，不以本地恢复测试关闭 LobsterAI 整卡。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。
