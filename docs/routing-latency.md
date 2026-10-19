# 延迟感知候选排序

普通候选路由在已有健康、余额、容量和授权规则之后，使用进程内近期成功样本辅助
排序。Local 与 Server 共用同一实现，但 Server 每个实例独立学习，不宣称跨实例
共享；重启丢弃这些建议性样本，不修改持久化健康状态或需要用户迁移数据库。

## 样本与身份

- 非流式在统一 dispatch 成功出口记录该候选的完成耗时，覆盖 JSON、canonical /
  Responses bridge 与 binary。计时不包含先前失败候选、初次 AIMD 排队和 readiness；
  包含本候选成功前的重试及恢复等待。
- 流式沿用 `TrackedStream` 的首个非空**输出 chunk**延迟，仅在成功 Completed 时
  记录。该值不是语义 token 时间；不等待额外语义解析、不缓冲整个响应。失败、
  中断、Drop 取消、无首 chunk 的空流不学习成功样本。
- key 包括 runtime subject（优先 credential ID，否则 account ID）、实际 upstream
  model、protocol profile、adapter、endpoint、streaming 标志。调用方 alias 不污染
  实际模型维度。每个字符串维度最多 256 字节，超限直接不采样，不截断产生碰撞。
- 流式与非流式分别维护 EWMA，`alpha=0.3`。样本在最后一次成功记录后 10 分钟
  到期，读取不续期；未知或过期采用固定 `1000 ms` 中性参考，而不是虚假的 0。
  单次样本钳制到 `[1 ms, 24 h]`，确保有限、非零的排序输入。

## 排序合同

`priority_weighted`（含原有未知策略回退）仅在**未 blocked、有效 weight 相等且
priority 相等**时比较延迟，较低者优先。没有样本时保持旧的 stable ordering。
`round_robin` / `random` 不使用该输入；quota exhausted 仍被剔除，熔断或冷却仍
后移，不会被一条很快的旧样本重新激活。

真实 session / user affinity 交给现有 queue sticky 规则：健康、仍在候选中的
sticky 最后提升到队首，可覆盖普通权重/priority 排序；冷却、熔断、硬过滤或额度
耗尽的候选不能被提升。这也修正旧入口仅调整输入顺序、排序后可能丢失 affinity
的问题。不是新授权来源，不改变显式跨供应商回退 allowlist。

已预约的 `access_catalog` / projected access 路径不参与此次延迟重排，保留访问行、
候选、预扣/预约的成对语义。不能宣传所有路由模式都已切换为延迟负载均衡。

## 生命周期与资源界限

每个 `AppState` 最多 4096 个 key。HashMap 与有序过期索引在同一短锁内维护，
每次热 key 更新先移除旧索引，不留下无界 tombstone；淘汰最旧或过期记录，不在
每次请求扫描全缓存。没有新任务、定时器、网络访问、存储表或依赖，也不持锁 await。
样本只读影响选择，不承担授权、计费、重试预算或恢复探针租约职责。

验证入口为 `tests/reference_routing_contract.rs`、`routing::latency::tests` 与
`pipeline::stage_route::tests::latency`；命令和实际通过证据分开记录在参考采用计划中。
