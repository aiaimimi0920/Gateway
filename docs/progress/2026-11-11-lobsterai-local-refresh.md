# LobsterAI 本地持久刷新接线

本批目标是让已有协议进入 Gateway 共享凭据 owner。总体仍为 partial，范围仍有 67 个未完整关闭的工作项。

## 实现范围

- 可恢复个人身份上下文绑定安装 UUID、官方用户标识和 exact token pair；本地 generation 区分没有 expiry 且返回相同 token 的后继。
- 原始响应保留为有大小限制的 ReceivedTokens，个人/企业和 token/identity 准入在 durable receipt 确认后执行。
- 共享 GrantResult 的已知寿命使用 Some，旧 JSON 数字/字符串形状保持；None 仅允许明确的 Lobster 类型，旧厂商仍需合法正数寿命和 RFC3339 到期时间。
- 共享 runtime 支持未知到期时间，不加入默认六小时。只有 exact access + generation + 401 的信号使当前会话到期；403/429/5xx 不触发。
- single-flight guard 贯穿 Sending reservation、receipt、Result、配置发布与 Published；写盘失败保留 pending，重试存储。
- 本地重启先只读检查历史记录，无记录不预约、不发送。有 receipt/result 则按 exact revision 恢复；仅 Sending 或 revision 不匹配时要求重授权，不重放网络。
- Secret Keep 保护安装、身份、token 绑定和代次；模型配置编辑仍可保留旧密钥。

## 兼容与恢复边界

没有修改现有数据或 storage version；旧 generic/Nous 等历史记录保留读取和 ID 合同。新 Lobster 数据依赖本批 reader，旧二进制不提供此产品支持；回退前应停用该新增产品，保留 grant 历史，不修改或删除凭据数据。

自动恢复必须匹配原配置 revision。这保留既有防止删除后重新添加同名授权的约束，不宣称跨任意 revision 恢复。

Server Redis cache owner 当前要求 expiry，本批在请求前排除 Lobster Server refresh，没有伪造 TTL，也不宣称 Server 支持。此处仅实现本地 owner；尚未接管理授权会话/UI、推理请求中的 401 信号、公共 SSE/非流式/工具与结算，也未进行真实账户、打包或部署验收。

## 验证状态

Cargo r1/r2 编译失败均保留回执。r1 是旧测试结构体缺字段、uuid 未启用 serde 和 Option deadline 测试解包；r2 是另一处旧测试结构体缺字段。已经分别修正，不升级依赖。r3 最终 **219 passed / 0 failed / 4 ignored**，其中 LobsterAI **34** 项。完整命令、耗时和日志保留在证据目录。

独立只读审查没有发现新的生命周期缺陷，确认了 exact revision 恢复边界。所有新增测试使用 synthetic material、非转发 loopback peer 与 linshi 下的隔离 TEMP，不访问真实 .ng。

定向 rustfmt、checker 测试 31 项、最终 effective-line ratchet 和 scoped diff 检查通过。未修改 baseline 或 exception，没有升级依赖。全工作区仍有继承 dirty edits；证据只覆盖本批 scope，不声明 whole-working-tree provenance。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-refresh-20261111`，包含 `cargo-r1/r2/r3.json/.log`、`source-r3.json`、`source-final.json`、`scoped.diff`、`changed-files.json`、`final-validation.json`。没有提交、推送、出包、部署或重启实例。
