# 2026-11-21：Server MCP 管理任务 TCP 强杀恢复

补齐 [Server 持久管理任务](2026-11-21-luma-mcp-server-journal.md) 的进程边界验收（L1／L6），不改生产逻辑、数据格式或以往证据。长期入口为 `tools/luma/check-postgres.ps1 -McpRestart -Output <新目录>`，和 `-McpCreation`、公共 Agents 的 `-ServerRestart` 互斥。

测试用独立 PostgreSQL／Redis、随机 loopback 端口和合成 MCP peer。正常管理 TCP 走 `/readyz`、管理令牌、secret grant 和 `/v1/internal/gateway/console/luma-mcp-creation`。每个场景在上游收到指定操作、但还未返回 JSON 回执时强杀第一个测试进程，等待该进程退出；新进程重用原配置目录和数据库，以原管理令牌重新申请 grant。活动租约期间读回的原 task 与 PostgreSQL 版本不变，且没有新增上游请求；由数据库时钟自然推进 90 秒租约，不改时钟或任务记录。

| 强杀窗口 | 重启后的结果 | 实际上游业务调用 |
| --- | --- | --- |
| 已落盘 `s1`／`r1`，首次 `Await` 挂起 | 同一 task/run 恢复到 `completed`；PG 保存原会话 `9 credits`，`settled=true` | `Open → Generate → Await → Await → Usage`；两次 `Await` 都是 `r1` |
| `Generate` 已发出，尚未收到 run ID | PG 从 `submitting` 变为 `unconfirmed`，不猜测 run ID；只读原会话 `9 credits`，`settled=false` | `Open → Generate → Usage`；不执行 `Await` |

两条路径在再次提交同一 task 时均返回 `409`，没有第二次 Open 或 Generate。合成费用为会话总账，不是某次未知 run 的实际费用，也不写入公共 Agents、现金或余额账本。媒体输出为合成测试所用的空列表，本次不证明媒体读取或播放。

显式 TCP 入口 **1 passed**，内含上述两个场景；创作定向回归 **37 passed / 3 ignored**。首轮测试宿主编译因 `error[E0597]` 未运行：`root` 未满足关闭 future 的生命周期要求。修复测试关闭信号的所有权后复测通过。定向 Rust 格式、PowerShell 语法及互斥参数拒绝、有效行检查器 **31 passed**、ratchet 和 `git diff --check` 通过。两组被强杀及正常退出的测试进程、两批 PostgreSQL／Redis 容器均已确认退出。

[验证回执与源码 SHA-256](../evidence/luma/gateway-luma-mcp-restart-20261121/verification.json)。这是测试二进制启动的单机 TCP 进程证据，不是新包、跨机器、原生应用点击或真实供应商生成／远端取消。Web 本轮未取得当前登录态只读合同，MCP 分层仍缺实际布局返回；不重新登录、不执行真实收费操作，未触碰 4200 实例，未提交、推送或部署。
