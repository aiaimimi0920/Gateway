# Copilot 缺失 CLI 的额度收尾与公共接通边界

本块只新增测试，覆盖显式 CLI 路径已失效、尚未启动 child 时的收费请求收尾。
不读取真实凭据、不自动发现或下载 CLI，不改变公共禁用状态。

## 验收路径

Chat／Responses × message_prepaid／token_prepaid，共四种情况。先通过真实 stage_filter
预扣额度，再让 sender 使用不存在的绝对 CLI 路径。要求返回终止错误、不重试／替换，
失败审计保留未知 usage 和上游状态，额度退还；随后成功切换为 unlimited，证明 Local
pending hold 已释放。workspace 保持空目录，probe 根没有任何 child receipt。

这里 routeAttemptCount 为 1：gate 记录一次执行准入，并不证明 SDK 已生成或已经发送。
本测试不将计数 1 误称为真实调用；实际没有启动 child。它只覆盖启动前路径预检失败，
不是已 spawn 后握手失败／超时或服务重启恢复验收。

## 下一公共接通切片

只读审查确认当前直接阻断是 `routing/config/provider_compilation.rs` 对 Copilot
credential 的无条件 disabled，而非 sender 尚未实现。下一生产切片应以以下结果为终点：

1. 默认构建仍禁用；显式 SDK feature 构建仅允许明确启用且有专用 token 的凭据。
2. 管理 SecretPatch／CAS 保存到公共 Chat／Responses buffered 请求，经过真实候选选择、
   eligibility、prepare 与 sender；验证一次合成执行、真实 Local 审计与额度收尾。
3. 缺 CLI／workspace、Server／非 Local Key、cash、streaming 及单轮范围外请求在 SDK
   generation 前拒绝；不把已有内部可信 gate 测试当作公共链已通过。
4. 只有公开链实际发现 call-flow／keepalive／工具准备缺陷时才改对应 owner，不预先
   泛化重写。管理存储、身份冻结、单轮准入、现金拒绝等已有机制继续复用。

以上是后续最小接通范围，不是本块已实现或已解禁的功能。真实 CLI／订阅账号、原生 EXE、
Server 和取消后自动恢复仍单列；保守槽隔离不等于优雅关闭或可靠回收能力。

## 回执

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-prepaid-startup-20261113`。
`startup.log`：1 passed / 0 failed / 0 ignored，覆盖四种协议／额度组合。构建 15m31s，
包含共享 build 锁等待和原生依赖编译；测试执行 0.45s。退款、额度模式切换、审计字段、
空 workspace 和无 child receipt 断言均通过。此场景未启动合成 child，因此不伪造
子进程退出回执。

checker 31 项、ratchet、本轮源码 rustfmt、scoped diff 检查通过。UTF-8 无 BOM；
prepaid 注册文件有效行数 194 → 196，新增 startup 测试 75。源码／测试 EXE 哈希见
`source-final.json` 和 `binaries-final.json`；未改 baseline。
没有提交、推送、出包、部署或真实上游请求，不增加整卡完成数。
