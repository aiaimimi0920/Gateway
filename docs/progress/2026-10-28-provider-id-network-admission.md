# 服务商 ID 在线迁移前置回归：出口故障与等待准入

本轮关闭 `2026-10-28-provider-id-wait-authorization.md` 中遗留的网络故障回归。
仅修改源码和隔离测试；4200 不停止、不 drain、不重启、不替换，真实数据库不迁移。

后续候选刷新进展见 `2026-10-28-provider-id-wait-candidate.md`；本文保留本轮网络回归记录。

## 原因与最小修复

关闭等待时，`subject_queueable` 原先直接使用 `subject_allowed`。全局出口故障令
所有 circuit unavailable，候选在授权/发送之前消失，最终错误可能成为“无服务商”，
而不是 `gateway_network_unstable`。开启等待时，旧测试却仍假设路由阶段立即失败。

`src/routing/admission.rs` 现在只在网络正常且等待关闭时使用原来的即时筛选。
网络故障期间保留符合队列准入条件的候选，让候选继续经过账号组、Key、协议等检查，
由真实发送准入负责精确拒绝或等待。余额等不可恢复限制与站点健康限制仍然生效；
没有新增候选，没有将网络故障当成权限来源，没有移除发送前的两层网络检查。

- 等待关闭：正常走到发送阶段立即返回 `gateway_network_unstable`，实际发送数为 0。
- 等待开启：仍由原请求预算约束；截止时间到达后结束，实际发送数为 0。
- 不存在的账号组和空路由仍返回原来的路由/权限错误，不被全局网络错误覆盖。

## 测试契约

原健康恢复测试现在显式关闭等待，并从完整 route→send 路径断言精确网络错误和
发送次数为 0，而不是要求 route 阶段抛错。健康字段恢复、故障计数、真实 401
保持 blocked 的原断言全部保留。新增开启等待与无授权路由的两项回归。

首次编译因新增测试误用 `RouteConfigYaml: Default` 失败。按真实 schema 改为
反序列化 `{"providers":[]}`，未修改生产 schema 或添加无关 derive。

最终验证（TEMP/TMP 为 linshi，Cargo locked/offline、jobs=2）：

- `cargo test --locked --offline --lib pipeline:: --jobs 2 -- --test-threads=2`：
  118 passed / 0 failed / 1 ignored。唯一 ignored 项需要独立 Redis，本轮未配置。
- `cargo test --locked --offline --lib routing::model_circuit:: --jobs 2 -- --test-threads=2`：33/33。
- `cargo test --locked --offline --test access_key_groups_contract recovery_wait --jobs 2 -- --test-threads=2`：
  5/5，覆盖本地回环 JSON/SSE、截止时间、禁用/账号组撤销和真实 HTTP 断连。
- checker 31/31、ratchet、scoped rustfmt、`git diff --check` 通过。
- 有效行：admission 172→173；egress tests 346→410。工作区原有 warnings 未自动修复。

## 未完成范围

这只关闭网络准入回归，不等于在线 ID 迁移完成。等待中的同账号新旧 provider ID
匹配、路由/映射统一发布、审计/价格/压力/refill 引用、恢复协调器和部署/降级保护
仍须接齐。迁移入口未开放，没有新 EXE 交付，没有真实上游调用。
