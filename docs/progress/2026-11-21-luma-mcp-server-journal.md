# L1：Server MCP 创作管理任务持久化

Server 复用 Local 的创作、会话审阅、素材复用和账号连续性校验。在 PostgreSQL 增加独立的 `gateway_luma_mcp_creation_tasks` 表，不往公共请求审计写伪造的项目／Key，也不改变现有 SQLite 任务的接口和落盘格式。正常账号面板可用任务 ID 读取原任务；Server 任务表不自动进入公共运维请求列表，也不改变 Agents 费用补账机制。

每次调用先校验实际的管理令牌和当前请求的 secret grant。任务用签名密钥及当前令牌指纹按独立用途派生的 HMAC 标识绑定：不保存令牌、原始指纹或 grant，也不把客户端 IP／Host 当持久身份。同一共享管理令牌仍是同一操作者；不同令牌、签名密钥变更或公开 passwordless marker 不能接管原任务。原 revision 不同时仍要求证明账号连续性。跨节点没有所需的旧 revision 归档时拒绝，不猜测账号归属。

准备不调用上游。PostgreSQL 以数据库时钟、租约和版本 CAS 裁决竞争；提交前意图先落盘。过期的 `opening`、`submitting`、`responding` 保留为 `unconfirmed`，不会重发。在供应商返回可验证的会话或 run ID 前强制中断，不能据此得知上游是否受理。账目为 MCP session credits，不映射到现金或公共 Key 余额。

部署需要先按 [部署文档](../../deploy/README.md#quick-start) 审核迁移及备份；本批不改现有实例。应用不自动创建表。持久数据包含请求／答复摘要、任务和费用事实，以及展示所需的供应商签名素材链接、审阅和会话内容（上游内容可能回显用户输入），必须按敏感数据保护。上游操作有时间上限；Server 在客户端断开后仍可继续处理，但未加入 Local finalizer 等待链；进程终止时以已持久的阶段和租约为准。

## 验证边界

隔离运行入口为 `tools/luma/check-postgres.ps1 -McpCreation`，经过正常管理路由、独立 Server runtime／连接池、真实 PostgreSQL／Redis 及合成 MCP 上游。完整 Server 用例 **1 passed**：准备为零上游调用，两 runtime 并发只开一个会话、提交一次生成；原令牌在新 runtime 下读取原 `r1` 与 `9 credits`；不同有效管理令牌、错误 grant、缺签名密钥、缺表均拒绝，过期 `submitting` 标为 `unconfirmed` 且不能重提；原任务不出现在公共请求审计。测试容器均已收尾。基线红测命中仅 Local 的 400；第一次修复后测试读回了任务与费用，但夹具期望错写 `luma_session_usage`。根据现有 wire 合同改为 `luma_usage` 后整项通过，生产代码未因此改动。

创作定向回归 **37 passed**（单独执行的 Server 测试在普通过滤器下标记 ignored）；feature-off 编译、有效行检查器 **31 passed**、ratchet、定向 Rust 格式及 `git diff --check` 均通过。

这个入口不是实际 TCP、OS 强杀、真实账号或原生应用验收。Web 登录协议和 MCP 分层布局仍需要脱敏的实际返回；未运行真实取消、媒体或收费生成，未重新打包。

[验证回执及源码 SHA-256](../evidence/luma/gateway-luma-mcp-server-20261121/verification.json)。
