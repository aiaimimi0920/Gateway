# Gateway 文档导航

日常使用从下表进入。产品合同说明当前行为；验收回执说明某一版本实际验证过什么；历史计划不是当前待办。

| 要做什么 | 从哪里开始 |
| --- | --- |
| 安装、运行、Docker | [项目 README](../README.zh-CN.md)、[Docker 部署与数据库迁移](../deploy/README.md)、[运维手册](operations-manual.md) |
| 配置账号与模型 | [账号发现](account-discovery.md)、[账号模型选择](account-model-selection.md)、[账号分组](default-account-group.md)、[本地数据](local-data-storage.md)、[访问密钥](local-access-keys.md) |
| 调整控制台、网络或备份 | [设置面板](settings.md)、[管理密钥](management-key-slots.md)、[备份与恢复](sync-backup.md) |
| 查请求路由与产品协议 | [服务商调用流程](provider-call-flow.md)、[首批三家交付与验收](providers/first-three-closure-2026-11-13.md)、[跨项目集成合同](../INTEGRATION_CONTRACT.md) |
| 处理运行问题 | [网络恢复](gateway-network-recovery.md)、[模型健康与恢复](model-health-recovery.md)、[官方站点健康](official-site-health.md) |
| 构建或发布 | [日常手测包](devtest-build.md)、[发布版本形态](release-editions.md)、[安全发布门槛](security-release-gates.md)、[CI 验证](ci-validation.md) |
| 继续未完成的工作 | [服务商扩展清单](providers/magpie-remaining-2026-11-13.md)、[计划与历史交接](plan/README.md) |

## 历史与验收

- `progress/` 是分批开发及验证回执；平时不必逐篇阅读，按现行合同或收口页的链接核对具体能力。
- `status/` 是历史检查点，不是当前部署状态。`plan/parallel-lanes/` 是分片交接记录，不是现行产品手册。
- [证据目录](evidence/README.md) 保留独立快照和原始材料。不要用源码回归、隔离手测包或合成上游结果代替真实账号验收。
