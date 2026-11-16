# Nous 控制台管理测试入口验收

后续已补自动调度与本地存储重开的定向证据，见 [自动模型测试调度验收](2026-11-12-nous-automatic-probes.md)；下文边界为本批当时状态。

在上一批专属 sender 与严格回复校验之上，本批补真实管理路由的本地合同测试。只新增测试、注册测试模块并扩大已有合成 credential 构造函数的测试模块可见性，没有修改生产逻辑。

## 已验证

- Subscription／Welcome 分别通过单次 prompt、临时策略、命名计划、账号保存策略四种来源；每个目标恰好发送四次 Chat 请求。
- 模型使用配置别名，实际请求映射到 `org/model`；每次 Authorization 为所选 `stable` 账号的合成 access token。
- 未取得 console secret grant 返回 403，外来 credential ID 返回 400，均无上游调用。
- 无保存策略的默认探测、禁用账号及错误模型返回 unsupported，不发送。禁用账号使用独立 typed credential，不复制单次 refresh token 链。
- 每个目标四条 LocalRuntime 审计均 completed、total_tokens 为 5、realCredentialRef 为 stable；管理结果查询包含命名计划，响应不包含 access token。

## 验证证据

命令：`cargo test --locked --offline --lib -- nous_console_probe_management nous_console_probe_disabled --test-threads=2`

**2 passed / 0 failed / 0 ignored**；测试耗时 0.64 秒，完整命令 890.47 秒（包含共享 Cargo 锁等待与编译，非纯编译时间）。上一批 64 项没有因本批测试扩展无故重跑，不能将两批报告成一次 66 项运行。

有效行 checker 31 项、ratchet、scoped rustfmt、git diff --check 均通过。新测试模块 166 有效行；fixture 保持 199，模块注册文件 181 → 182。三个 Rust 文件编译前最终快照与验证后 SHA256 一致、UTF-8 无 BOM；`scoped.diff` 以本批修改前快照为基线，未把继承 dirty edits 计入本批。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-management-probe-20261112`，包含命令日志与 receipt、源码哈希、修改前快照和 `final-validation.json`。

## 边界

测试使用已有 ConsoleStateFixture、真实管理 router、LocalRuntime 以及隔离 loopback 上游。不证明真实账号、client 资格、订阅额度或现金费用；没有运行浏览器 UI、原生 EXE、Server／Redis、自动调度、并发授权轮换或进程重启恢复。durable refresh owner 未修改。结果查询验证不等于重启后结果恢复验收。

本批补齐 Nous 管理测试入口的上述本地证据，不关闭 Nous 整卡；全范围仍为 67 个工作项未完全关闭。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。
