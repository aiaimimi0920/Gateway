# 服务商 ID 在线迁移：等待映射内存记账

承接 `2026-10-28-provider-id-wait-policy.md`。本轮不开放迁移入口，不写真实配置／
数据库，不停止、drain、重启或替换 4200。开始核验的 loopback PID 为 42912，
LAN 转发 PID 为 4280，healthz／readyz 均为 HTTP 200。

## 改了哪些内容

- 把身份注册表的条数／ID 字节限制集中为常量。有效 String 经 boxed string
  收紧容量后存入表，不能用一个内容很短但 capacity 巨大的输入绕过估算。
- 本地等待请求在原 canonical 副本估算之外预留 768 KiB，覆盖当前／替换两个
  最大映射快照（256 对，每端 256 bytes，每对另计 1024 bytes 保守树结构开销）。
  这是队列准入估算，不是 allocator 或进程 RSS 的硬保证。
- 即使注册表为空，也预留完整额度；未来等待中出现映射不会超出已登记的身份
  预算。代价是本地请求可能更早触发已有的 64 MiB 等待限制。非本地请求维持原估算。
- 查等待策略的临时映射在策略解析后离开作用域，不跨 sleep／turn 等待保留。
- 继续使用原 lease 的计数、FIFO 和 Drop 释放，不重新入队、不迁移 queue owner。

## 验证

新增三项测试：超大输入 capacity 收紧；空表到满表的固定预算及非本地估算不变；
身份预算确实计入队列上限、超限拒绝与 lease 释放后容量恢复。

实际验证使用 `--locked --offline --jobs 2 -- --test-threads=2`，TEMP/TMP 指向 linshi：

- `--lib pipeline::`：132 passed，1 ignored（独立 Redis 环境用例）。
- `--lib provider_identity::tests`：6 passed。
- `--lib request_wait::`：9 passed，包含既有 FIFO 取消释放回归。
- `--lib local_runtime::`：38 passed。
- scoped rustfmt、checker 31/31、effective-line ratchet、`git diff --check` 通过。

共 185 项 Rust 定向测试通过。未运行全量测试、HTTP/流式集成测试、付费上游测试
或 EXE 构建。本轮没有新增 HTTP／流式迁移成功证据。

有效行：provider_identity 75→84，identity tests 77→91，recovery_wait 260→272，
新增 memory_tests 69。未调整 baseline、未修改其他线程代码。

## 未完成边界

本轮仅补齐等待映射内存记账；旧／新 ID 的 FIFO、provider 限额、压力及熔断身份
仍未合并。等待预算动态缩短、统一发布与恢复协调器、计费审计引用、旧版本并发
保护以及不停服务实例接替仍待完成。没有真实在线迁移、上游调用或新 EXE 交付。
