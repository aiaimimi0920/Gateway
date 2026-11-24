# 2026-11-21：MCP 创作正常管理网页联合验收

按 L1／L4／L6 先做、依赖已登录 Web 的 L2 后做的顺序，本批补上 L1／L6 的正常管理网页验收。只新增可重复测试，不改创作、费用或存储的生产行为；复用已有 PostgreSQL／Redis 强杀证据，不重复跑数据库场景。

- 正常 main 构建经独立 Edge 登录和自动 secret grant，进入已保存 MCP 账号。登录、打开面板和准备任务均没有 MCP 请求；不确认费用时“提交生成”禁用。确认后只生成一次。
- 关闭、重开面板不会重发；输入原任务 ID 查询，页面显示“上游已完成”、`9 credits · 已结算` 和合成资产链接。未打开该链接。三次管理请求都绑定同一精确账号及配置 revision；提交和查询使用同一任务 ID。
- 宿主从 SQLite 的同一审计行独立读回原 `r1`、完成状态、链接和会话费用，配置 revision 不变。MCP 顺序为 `Open → Generate → Await → View → Usage`：15 次传输 POST、5 次 transport DELETE；DELETE 是运输会话收尾，不是验证真实业务取消。管理授权不进入 MCP 请求。

显式宿主 **1 passed**、正常 main 构建、Node 语法、定向 Rust 格式、有效行检查器 **31 passed**、ratchet 和 `git diff --check` 通过。首轮浏览器夹具错误地等待准备动作再次申请 grant，超时；调整为等待正常登录的自动 grant 后通过，无生产代码改动。已核对本轮独立浏览器、代理和 Rust 宿主退出。

[可重复执行入口](../../tools/luma/README.md#mcp-创作正常网页联合验证l1l6) · [验证回执与范围源码 SHA-256](../evidence/luma/gateway-luma-mcp-creation-browser-20261121/verification.json)

L4 的 SQLite／PostgreSQL 原任务补账和换 Key 拒绝以[既有回执](2026-11-21-luma-hosted-reconciliation.md)为准。L1 的 MCP 分层布局仍缺上游返回证据；L6 的真实媒体、远端取消、账单和原生点击仍未验。没有访问真实 Luma、读取个人浏览器、消费真实 credits、改动运行实例、重新打包、提交、推送或部署。
