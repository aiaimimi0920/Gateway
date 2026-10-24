# 健康适配器接口证据与实现边界

2026-10-23 开发调查已联网读取以下官方仓库源码。链接指向分支，不是不可变 commit；后续接口变化需重新核对。本文是调查依据；实际接入与验证见 [第二阶段](../progress/2026-10-23-site-health-profiles-phase2.md)。

## New API

官方来源：

- <https://raw.githubusercontent.com/QuantumNous/new-api/main/controller/perf_metrics.go>
- <https://raw.githubusercontent.com/QuantumNous/new-api/main/pkg/perf_metrics/types.go>
- <https://raw.githubusercontent.com/QuantumNous/new-api/main/pkg/perf_metrics/metrics.go>

`GET /api/perf-metrics/summary?hours=24` 返回 `success:true`、`data.models[].model_name/success_rate`，比例为 0–100。摘要跨活动组聚合，不能把它当作 Plus/Pro 独立健康结果。摘要不保证暴露请求计数字段，不能强制要求 `request_count`。

远端精确分类应使用 `GET /api/perf-metrics?model=<model>&group=<group>&hours=24`，核对 `data.model_name` 和 `data.groups[].group`。`summary:null,groups:[]` 属于未知/无样本，不是测得 0%。逐模型详情必须有总请求预算，不能对完整模型目录无界展开请求。

## sub2api

官方来源：

- <https://raw.githubusercontent.com/Wei-Shaw/sub2api/main/backend/internal/service/channel_monitor_v2.go>
- <https://raw.githubusercontent.com/Wei-Shaw/sub2api/main/backend/internal/handler/channel_monitor_v2_handler.go>
- <https://raw.githubusercontent.com/Wei-Shaw/sub2api/main/backend/internal/repository/channel_monitor_v2_repo.go>
- <https://raw.githubusercontent.com/Wei-Shaw/sub2api/main/backend/internal/server/routes/user.go>

用户监控接口需面板 JWT Bearer，不是业务模型 API key。分组模式使用 `GET /api/v1/channel-monitor-v2/matrix?range=24h&group_by=platform_group_model`，`group_id` 为正整数，可重复指定。响应 envelope 为 `code:0,data`，矩阵行含 `platform/group_id/model/metrics/health/buckets`。

`metrics.success_rate` 比例为 0–1，需乘 100；不能用 `1-error_rate` 替代。普通用户计数可能脱敏为 0，不应据此判定无样本；`health.overall:"unknown"` 才应保留未知语义。应保留 coverage 不完整和聚合滞后信息，而不是输出伪精确健康率。无分组的 `models` 接口不能冒充分组矩阵。

`/health` 的 `status:ok` 只是存活信号，模型/分组目录也不是成功率证据。

## One API 及原兼容模板

`songquanpeng/one-api` 官方的 `/api/status` 主要是站点配置/存活信息，不能宣称提供模型健康率。原有 `model-status-all/channel-health/model-status-embed/model-health-slot` 模板保留兼容用途，但本轮没有确认它们是官方 One API 的通用接口，不应以官方保证宣传。

## 认证边界

自动发现只尝试有限公开健康模板。配置监控认证时须手动选择适配器，避免向所有候选模板广播密钥。监控密钥不能借用业务凭据；只允许固定同 origin 请求、禁止重定向，并继续遵守已有 DNS 固定、私网许可、响应体大小和总超时限制。

401/403 监控认证失败属于未知，不得转换成模型 0% 并自动禁用。适配器、远端分类或本地组绑定变更后，旧快照必须失效。
