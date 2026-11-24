# Command Code Server 内部模型探测审计合同

## 所有权与数据边界

定时探测并无公共 API Key、用户凭据或访问 Key。新增 `gateway_internal_model_probe_audits`，承载 Gateway 内部探测主体；不改 Platform 维护的 `001`、公共请求身份约束、公共查询聚合及用户结算。本合同描述专属 Command Code 文本模型探测，手动与定时执行复用同一发送器和生命周期。[SenseNova Token Plan](../progress/2026-11-22-sensenova-server-probe-audit.md) 后续复用同表和生命周期，通过不同产品标识隔离。其他探测、Local SQLite、公开推理、目录请求、Luma 媒体对账保持旧路径。

该表只保存独立审计 ID、部署 namespace、产品、实际 route provider／credential ID、模型、起止时间、终态、已知 HTTP 状态和实际观察到的 token 数。无 API Key、请求或响应正文、原始远端错误、签名 URL、现金金额及用户字段。namespace 用于同一部署的分组，不是租户鉴权凭据。身份按入场时的配置快照保存，不依赖可删除的 Hosted row；独立 route 配置和 Hosted account 不能互相假冒。没有增加公共或普通管理路由。读回由已授权的数据库运维角色按 namespace、产品、provider 过滤，本次以有限查询验证；探测评估仍走现有管理结果入口。

## 持久化契约

1. 在发出生成请求前提交 `running`；表不存在、连接失败或无权限时返回明确的探测审计不可用，不发送，不把存储失败写成模型故障。Server 必须有池，不能当作无数据库的测试配置继续。
2. 复用 `ProbeAudit` 有界审计许可和取消所有权。完成、失败和取消终态只可从同一记录的 `running` 写入一次；缺少确定用量保留 NULL，不估算、不重试生成。终态存储失败不能回报探测通过；随后不重复提交同一生成。强制退出留下的 `running` 表示未知，不构成成功或恢复重发的凭证。
3. 运行时不创建表、不自动清理审计。数据库 owner 管理授权、备份、使用量与保留期。混部时只有新版副本提供这项完整审计；完成滚动升级前不能声称全流量覆盖。

## 迁移与回退

`007-gateway-internal-model-probe-audits.sql` 是独立 additive 扩展。新 standalone 卷由官方 initdb 加载；已有库必须备份，先由部署 owner（共享部署由 Platform schema owner）评审表、权限和登记，再事务性应用并核验，之后升级二进制。运行角色需 INSERT、UPDATE 及 PostgreSQL 为更新谓词要求的 SELECT；不授予未授权消费者读表。降级可直接回退二进制并保留扩展表，不回填公共审计、不删除原有记录或卷。

本仓库实现与隔离库验证不构成实际环境迁移审批。验收应覆盖未迁移零发送、成功/失败/取消的终态、精确作用域与实际用量、双调度周期/重建、公共 Hosted 请求审计和原身份拒绝。未验证项需明确保留。
