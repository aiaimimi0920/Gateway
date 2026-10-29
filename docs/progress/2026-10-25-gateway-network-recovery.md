# 出口网络波动保护与服务商手动健康重置

## 交付状态

已完成后端实现及定向开发侧验证。没有生成手测包、替换运行实例、提交或推送代码。
本轮没有对正在运行的 4200 Gateway 执行启动、停止、重启或数据修改操作。

行为及并发边界见 [gateway-network-recovery.md](../gateway-network-recovery.md)。

## 本轮收尾修复

- 集成测试不再引用仅在库内部 `cfg(test)` 可见的清理模块；仅删除本测试创建的 UUID 临时目录。
  本轮测试将 `TEMP` 和 `TMP` 明确设置为 `C:/Users/Public/nas_home/AI/GameEditor/linshi`。
- 服务商开关测试提交脱敏配置时，携带所有已配置秘密的 `SecretPatch::keep`。
  未放宽生产接口的鉴权、Secret Grant 或密钥补丁校验。
- 格式化出口恢复测试的临时目录构造；没有改变其测试语义。

## 已执行验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-network-recovery-20261025`。

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| circuit、出口恢复、Local 时间戳 CAS、provider runtime 定向库测试 | 41 passed，0 failed | `focused-lib-final-r4.log` |
| 服务商开关 Console 合同 | 1 passed，0 failed | `integration-final-r3.log` |
| 模型健康恢复 Console 合同 | 9 passed，0 failed | `integration-final-r3.log` |
| Codex 模型探针 Console 合同 | 1 passed，0 failed | `integration-final-r3.log` |
| `cargo fmt --all -- --check` | 通过 | `format-final-r4.log` |
| effective-line checker 自测 | 31 passed，0 failed | `effective-tests-final-r3.log` |
| effective-line ratchet | 通过 | `effective-ratchet-final-r4.log` |

最终 scoped 源码哈希、UTF-8 无 BOM 检查和差异检查另存于同目录的
`source-final.json`、`encoding-final.json`、`diff-check-final.log`。
这是所列文件的开发验证快照，不是正式发布或 EXE 来源凭据。

测试采用锁定依赖、`devtest` profile、`runtime-defaults`、4 个 Cargo jobs，关闭管理网页嵌入；
HTTP 测试使用 loopback 合成端点，健康存储使用隔离 SQLite。
关键场景包括：出口阻断、恢复前后迟到失败、回退前置等级、保留真实认证／额度拒绝、
CAS 保护并发新状态、手动服务商重置、普通改名不重置、其他服务商和密钥不受影响。

## 未执行及能力边界

- 未实际断开宿主机网络，未调用真实付费供应商，未使用线上凭据。
- 未执行 PostgreSQL／Redis live 验收、全量回归、桌面手测、EXE 打包或正式发布流程。
- 网络窗口属于单个 Gateway 进程，不宣称跨进程或跨重启恢复。
- 通用出口探针可达不代表所有供应商域名均可达。
- 手动重置只移除健康处罚，不修复已失效凭据或补充余额；后续真实拒绝仍会重新记录。
