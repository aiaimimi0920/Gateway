# LobsterAI 管理策略保存与运行生效

## 本批闭环

此前管理与自动测试采用夹具预置策略，本批补实际管理保存路径：通过 `PUT /v1/internal/gateway/console/route-config` 提交脱敏 document、原 revision 和 `SecretPatch::keep`，不直接替换内存路由或写入测试策略文件。

新增一个完整流程测试：

1. 从无测试策略的隔离 LobsterAI 账号启动，获取管理 Secret Grant。
2. 通过真实管理 router 保存账号自动策略和命名计划；确认 committed、revision 更新，access／refresh／产品 context／catalog 保持原值，响应不泄漏合成凭据。
3. 使用旧 revision 再提交返回 CONFLICT，已保存路由文件字节不变；两次保存请求均未触发上游生成。
4. 运行既有 scheduler sweep，两个保存目标各调用一次；自动结果独立保存、plan ID 正确、标准答案评分通过。上游收到实际保存的 prompt、配置模型别名及精确账号。
5. 通过同一管理 API 关闭两项自动策略；立即 sweep 的 scheduled_count 为零。关闭并从原文件重建 AppState 后仍为零，证明关闭状态已落盘而非只改草稿。
6. 全程仅两次推理，完成审计有真实 5 tokens；原凭证材料与普通 Key 余额保持不变，最后正常关闭运行时。

现有生产链路通过上述检查，本批未修改生产逻辑。新增 `inference_tests/policy_save.rs`（135 有效行），父模块注册 202→203。

## 验证

`cargo test --locked --offline --lib -- lobsterai_policy_save --test-threads=2`：**1 passed / 0 failed / 0 ignored**，测试 0.52 秒，完整命令 1030.88 秒，主要为编译；本次日志没有共享构建锁等待记录。

官方有效行 checker、ratchet、scoped rustfmt、差异／空白检查通过；本批源码测试前后 SHA256 一致，UTF-8 无 BOM，未修改 baseline 或例外。未改变共用夹具及生产逻辑，因此不重复运行上一批已通过的 93 项集合。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-policy-save-20261112`，含 before、命令日志与回执、scoped.diff、源码哈希及最终检查。

## 边界

本批证明管理 API 保存到运行时和本地持久化，不是桌面浏览器编辑／自动保存 UI 验收。关闭自动策略影响后续准入，不承诺撤回在途请求。仅合成凭据和不转发 loopback 上游；真实账号、费用、自动调用与授权轮换组合、Server/Redis、原生包仍开放。

全范围仍有 67 个工作项未完全关闭，不关闭整卡。未读取真实 `.ng`，未提交、推送、出包、部署或修改运行实例。
