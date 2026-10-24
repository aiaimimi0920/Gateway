# 模型硬熔断第一阶段开发与验证

这是第一阶段的历史检查点；多方案/认证/UI 的后续成果见
[第二阶段](2026-10-23-site-health-profiles-phase2.md)，下文的“尚未完成”只描述当时停点。

## 实际完成范围

已接入单实例的服务商 × 凭证组 × 实际上游模型硬熔断，不再只有软性扣分。

- 60 秒内连续 3 次合格失败，按 2/8/32/60 分钟冷却；更长的上游等待不会被缩短。
- 到期单次试放、两次相隔至少 30 秒的完整成功恢复，稳定成功 30 分钟清退避级别。
- 取消、本地预算、参数/上下文/内容拒绝不污染计数；鉴权失败保留已有凭据状态流程。
- 流式反馈等待真实终态；失败只记一次，迟到回包不能解除较新熔断，Drop 释放试放资格。
- 硬过滤接入 YAML 池内选择、多目标映射、普通路由、access-catalog 成对候选、发送前复核。
- 本地公开模型目录使用只读存在性判断，不为查询目录消耗 round-robin、随机选模型或复制秘密。
- 管理站点健康响应增加 `model_circuits`；命名空间区分本地组和分组外凭证身份。

原有人工开关、凭据状态和监控结果没有被写入或清除。没有改 live `.ng`、数据库或运行实例，没有真实模型/付费上游调用，没有提交、推送或生成新的手测交付包。

## 定向验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-health-v2-20261023`。

| 检查 | 实际结果 |
| --- | --- |
| 相关 Rust 库、pipeline/health/smart-routing 测试目标编译 | 通过 |
| `routing::model_circuit` | 11/11 |
| `routing::config`（含只读目录、映射和人工禁用） | 86/86 |
| `pipeline::stage_rate_limit` | 2/2 |
| `pipeline_send_runtime_contract` | 48/48 |
| `site_health_contract` | 5/5 |
| `site_health_runtime_contract` | 4/4 |
| `smart_routing_contract` | 6/6 |
| effective-line checker 自测 | 31/31 |
| effective-line ratchet | 通过，受治理源码无 >700 行 |
| Desktop TypeScript 类型检查 | 通过 |
| 改动范围 Rustfmt、`git diff --check` | 通过 |

相关 Rust 测试合计 162 项。批量运行记录为 `circuit-checks.json` 与同目录日志；诊断键命名空间隔离后再次编译并通过 3 项新增熔断 HTTP 场景，再完整复跑 48 项发送链路，记录为 `final-pipeline-send.log`。源码最后仅调整一条与实际行为不符的注释。

新增 HTTP 场景证明：同一目标连续 3 次已识别 HTML 403 后第 4 次不再发送；Plus 被隔离后 Pro 仍能在池内选中；流在交付后中断仍累计故障，不把响应头当成功。测试构造器以独立 boxed future 控制 debug 测试栈，无需增大 `RUST_MIN_STACK`。

行数：`stage_send.rs` 从本阶段开始的 497 降为 492；`stage_route.rs` 434→468；熔断状态机 263、尝试生命周期 55、错误分类 49、状态机测试 180，均未跨 500 行。

## 尚未完成，不作为全功能交付

1. 多检测方案按本地组绑定/共享、方案级观测缓存及自动恢复。
2. 手动适配器、监控专用凭据的安全存储/回显/身份重排，以及相应管理 UI。
3. New API 精确远端分组、sub2api V2 矩阵适配及相关离线契约。
4. PostgreSQL 公开模型目录的实时硬准入一致性（实际路由已有过滤）。

`src/site_health/policy.rs` 仍未注册，只是后续准备代码。当前熔断状态不跨重启、不跨进程共享；没有用此结果宣称 cluster/restart 保护。EXE 和原生界面验收留待后续功能接入及用户的出包要求，不把本次测试构建冒充手测交付。
