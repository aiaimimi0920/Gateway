# 2026-11-21：Luma Agents 托管凭据强杀补账

接续[四个剩余工作包](2026-11-21-luma-four-continuation.md)，本批补上 L4／L6 的 PostgreSQL 托管凭据路径，不重做 L3／L5。真实收费生成仍暂停；未触碰现有 4200 实例、真实数据或凭据。

## 改了什么

- 普通公共请求使用 PostgreSQL 路由挑选出具体 provider 和 credential 后，首次取得已验证任务 ID 时将恢复标记写入原审计。标记 v2 以 `GATEWAY_API_KEY_SECRET` 的专用派生 HMAC 绑定原审计 ID、provider ID、credential ID、官方 Agents 端点和发送时用的 Key；审计不保存 Key，没有数据库迁移。旧路由文档标记 v1 及其 revision 规则不变。
- 重启后只读取原 provider 和 credential，不重新选账号。账号必须仍可用、未归档，且实际 Key 和签名密钥必须与原绑定匹配；不匹配则阻止自动补账，不退回路由文档，不重发生成。没有签名密钥时，托管请求照常执行，但不会写入可跨进程补账的 v2 标记。
- 成功补账只更新原任务实付 credits，不改原请求状态、媒体校验事实、本地余额或 USD/cash。尚未取得可验证任务 ID 的受理窗口不在恢复范围内。

## 实测

普通 `POST /v1/images/generations`、实际 Server 路由、隔离 PostgreSQL／Redis 和 loopback 合成上游。测试在首次 GET 保持挂起时从操作系统终止第一个 Gateway 测试进程，再启动第二个进程。托管用例的路由文档为空，由 PostgreSQL 里的 alias、provider account 和 credential 完成路由。

| 场景 | 上游调用 | 同一审计行 |
| --- | --- | --- |
| 原路由文档凭据 | `POST → GET → GET` | `settled`，0 → 12.5 credits |
| PostgreSQL 托管原凭据 | `POST → GET → GET`，恢复使用原 Key 和原任务 ID | `settled`，0 → 12.5 credits |
| 强杀后换托管 Key | `POST → GET`，重启后没有上游调用 | `blocked`，仍是 0 credits |

三个场景的原审计仍为 `running`，原任务事实仍为 `queued`，`mediaVerified` 仍为 `false`。旧源码运行托管场景时已到首次 GET，但审计的 `route_trace` 为 SQL NULL（旧夹具日志为 `ColumnDecode ... UnexpectedNullError`）；修复后二进制的复合强杀用例全过。测试只推进 worker 的注入时钟，不假称实际等待 15 分钟、实际后台定时器或打包 EXE 已验。

本批 Rust 定向 13 项通过、3 项默认忽略；另显式执行 PostgreSQL CAS 1 项与上面的复合强杀测试 1 项，均通过。关闭功能编译、Rust 格式、有效行检查器 31 项及 ratchet 通过。

[验证回执与范围源码 SHA-256](../evidence/luma/gateway-luma-hosted-20261121/verification.json)；持续入口见 [Luma 开发工具](../../tools/luma/README.md)。

## 仍未关闭

- L1：Server MCP 是管理创作任务，不是本次公共 Agents 请求。现有管理授权没有可直接写入 PostgreSQL 公共请求审计的主体；下一步需明确管理任务的持久 owner、密钥换代规则和存储契约，不使用伪造公共 Key 或租户 ID。MCP 分层仍缺布局字段的实际返回合同，远端业务取消尚未验证。
- L2：仍需当前已登录 Web 的脱敏只读会话、board 和任务记录；已归档公开资料与 MCP/Agents 接口不能代替。无需重新登录，不提供真实 Cookie、Authorization 或带签名的资产 URL。
- L6：此次只证明公共 Agents 的合成 Server 进程边界。真实媒体、真实供应商费用、Web worker 和远端取消仍分别待验。未重新打包、提交、推送或部署。
