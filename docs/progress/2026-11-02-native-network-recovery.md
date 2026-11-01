# Native 网络恢复与并发容量修复

## 范围与现场

用户确认重启的是 Gateway，并要求不稳定代理网络的短时故障在 Gateway 内恢复。
只读现场显示 4200 health／ready 正常；历史审计存在 Native send error 和流取消。
没有取得历史代理切换证据，不能据此认定是旧代理配置导致历史故障。
本轮不停止在线实例、不改凭据、不调用真实模型、不打包或提交推送。

## 改动

- AIMD feedback／容量转移串行化，避免多个失败按同一旧 limit 重复扣减许可。
  available 改读真实 semaphore。旧实现两轮各 10,000 次并发失败中分别出现
  2,037／1,811 次容量归零；隔离候选两轮均未出现。
- Native reqwest source 链仅映射到固定诊断类别，不记录 URL、query、代理认证或
  raw source。首次 hop 的 connect failure 才具有可信 pre-origin recovery evidence。
  同源 redirect 的 request-local 记录阻止把后续 hop 连接失败误当作未发送。
- 同账号重试通过原 ProviderAttemptGate、审计和 RequestBudget：每请求最多 4 次
  额外连接资格、30 秒恢复准入窗口、原 deadline／取消继续有效。不会给成功连接
  后正在等模型的请求硬加 30 秒总 timeout。持续故障明确失败；不惩罚账号／模型。
- 本地 502／504 不冒充供应商 HTTP 响应；审计 upstream status 留空，trace 分类
  使用 networkCategory，attempt 指标使用 native_network。finalize 健康持久化
  同样排除 typed Native 故障，避免只保护 send 分支却在收尾时误记账号 cooldown。
- 区分取消、deadline、shutdown 与上游中断；保留既有退款／usage settlement。
  明确 caller cancellation 才写已有 disconnect 字段，无数据库版本变更。
  Local／PostgreSQL 均 first-terminal-wins。

## 验证

测试回执与脱敏现场证据位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-native-diagnosis-20261102/`。
生产源码 AIMD 的 10,000 次并发 harness：mismatches=0、zero_capacity=0。
第一组扩展定向测试 128 passed，包含真实 loopback 代理恢复、工具原始 wire、
持续故障有界、取消、原 deadline、redirect／未知发送负对照、407 代理认证、
原 Native Codex body/header 合同。测试曾暴露底层库使用
`proxy authorization required` 而非 `proxy authentication`，已按真实错误修复。
补充最终 finalize 健康／审计误归因保护后的最终回执：

- `source-focused-final.log`：139 passed／0 failed，包括最终健康持久化保护测试。
- `source-check-final.log`：默认 features 下非测试 `cargo check --lib` 通过。
- `cargo fmt --all -- --check`、`git diff --check` 通过。
- effective-line checker 自测 31 passed／0 failed，ratchet 通过；未重建 adoption baseline。
- 本轮 46 个相关文本文件 UTF-8 无 BOM 校验通过。
- 4200 health／ready 均 200；UI PID 54332、后端 PID 58504 及启动时间未改变。

构建仍有非阻断 warnings，未做无关警告清理。没有把上述合成上游测试称为真实
Google／OpenAI 整链或 PostgreSQL 实库验收。脱敏汇总见证据目录的
`source-fix-final.safe.json`。这份回执只证明源码和定向测试，不是 EXE 发布凭据。

未执行：真实上游／付费模型、生产替换、EXE 打包、PostgreSQL 在线验收、正式发布。
覆盖边界：新连接恢复仅普通 Native Codex HTTP；opt-in native session 和 WebSocket
不新增此资格。handoff 后的 body 中断不重放，保留既有 stream failure 处理。
