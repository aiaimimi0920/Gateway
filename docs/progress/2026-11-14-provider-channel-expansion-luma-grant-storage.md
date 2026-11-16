# Luma 刷新发送意图的持久防重放登记

日期：2026-11-14。范围是已有持久 grant 所有者的 Luma Sending 支持，不是正常账号刷新已完成。

## 实现

- 从 offline context 派生固定 endpoint／client／expiry，复用协议的 access／refresh 格式校验；已过期 access 可登记，未知 expiry 不伪造。
- 独立材料准入拒绝跨产品混入、未明确授权、固定 endpoint 下缺少 context、秘密畸形及 expiry 冲突。opaque refresh 的 ASCII space 规则与协议保持一致，不套用通用 trim 规则。
- 新增可选 `lumaOauth`，缺省省略以保留旧 JSON、material ID 与版本；Luma context 纳入 material ID 和等价比较，family ID 保持原 endpoint／client／refresh 定义。
- 复用本地文件存储、writer lock 与权威 revision。重新创建 runtime／文件所有者后仍恢复同一 Sending；重复 reserve 为 Existing，修改 access 后同 family 为 Blocked，旧 revision 为 Stale。
- `permits()` 双向禁止 Luma successor，普通 result() 和伪造 Result／Published 均不能越过尚未完成的接收回执支持。未更改 receipt schema 或旧结果寿命规则。
- Accio 数据库快照只补新增字段的 None；未开放 Luma 数据库刷新、worker、自动刷新、UI 或任何真实网络请求。

## 验证

- 测试先失败：正常 offline Luma 的 GrantRequest 派生返回 `console_recovery_required`，确认原实现缺少此接线。
- 只读审查已完成，修正测试兄弟模块可见性，并补足相同 endpoint 下剥离 context 的封口断言。
- 新增定向测试 5 项通过；本次刚构建的测试程序执行 Luma 相关回归，162 passed、0 failed、2 ignored（包含上述 5 项，跳过项不计通过）。
- 原有 `console_refresh_grant_contract` 集成测试：14 passed、0 failed、6 ignored；6 项 Redis 场景要求显式可丢弃 Redis 地址，本轮未执行。
- 非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过。保留仓库既有 warning，没有扩大为无关清理。
- 有效行检查器测试 31 项、最终 ratchet、scoped rustfmt 与 git diff --check 通过。9 项源文件已验证 UTF-8 无 BOM，冻结哈希在定向回归后保持一致，见 `source-hashes.json`。最大的改动文件为 374 有效行，保持单一材料身份职责；未增长超过 700 行的遗留文件。
- 证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-grant-storage-20261114`。测试 TEMP/TMP 指向该目录下的 temp，保留合成数据，不清理外部目录。

## 尚未完成

收到刷新响应后的持久保存、延迟解释／隔离、一次性轮换结果发布与恢复仍待实现。此次本地 runtime 重建测试不等于实际进程崩溃或多节点验收；没有真实 Luma 授权、刷新、生成、下载、Redis／PG 验收或 EXE 出包，没有提交／推送，也未修改真实数据和运行实例。
