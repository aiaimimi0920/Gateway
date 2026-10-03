# CI 分层与编译反馈

## 日常 PR / main

Windows 与 Linux 默认产品验证仍运行既有 Rust 检查和全量测试、前端测试、
类型检查、构建及 Tauri 检查。测试线程策略和安全扫描没有放宽。
Windows 的 44 条 feature 线路不再串在默认产品测试之前。

`tools/plan-gateway-line-matrix.py` 根据事件的 base/before 与实际 checkout SHA
做完整 Git diff，不依赖 GitHub 文件列表分页或路径筛选的文件数上限。
只有已知 UI/文档文件集合可跳过 feature 矩阵；默认产品验证仍必须运行。
Rust、依赖、manifest、构建脚本、工作流以及未知路径均触发完整矩阵。
比较失败、缺失历史、空 diff 也执行完整矩阵；重命名同时检查旧路径和新路径。

完整矩阵在独立 Windows job 中分为 6 片，每片约 7–8 条线路，最多 6 片并行。
各片使用现有 verifier 和完全相同的测试筛选条件；不将失败忽略为成功。
默认产品测试与分片同时进行，耗时不再相加。实际排队时间受账户并发额度限制。
`Windows product validation` 保留原检查名称，聚合产品测试、计划和必要的全部分片；
失败、取消或非预期跳过均不能通过该检查。Linux 检查名称不变。

## 缓存与资源

矩阵的 `SharedCargoTargetDir` 与 rust-cache 都指向 `target/line-matrix`，
按分片、Rust 源码/manifest 指纹及 action 自带的工具链/依赖指纹区分缓存。
启用 workspace crate 缓存；仅 main 写入，PR 只读取可访问缓存。
矩阵专用 dev/test profile 不生成 debug info，减少链接与缓存体积，
不关闭 debug assertions、不修改发布 profile。默认产品测试的调试配置不变。
缓存失效时仍执行真实编译与测试，不把 cache hit 当作测试通过。

每个产品/矩阵 job 最长 120 分钟。相同事件、PR/ref 的新 CI 自动取消旧 CI，
不同 PR、手动、定时任务互不取消。该设置不取消独立的发布工作流。

## 完整验证与发布边界

每周日 03:17 UTC 的 CI 和手动 `workflow_dispatch` 强制运行所有线路。
现有 `verify-gateway-release-candidate.ps1` 的默认全量线路验证保持不变；
`build-windows.yml`、`release-tag.yml` 及安全、签名、打包流程未在本次调整中修改。
这次优化不代表 release 已通过，也不新增或声称已有跨工作流的发布依赖。

本地原有 `verify-gateway-line.ps1 -All` 仍执行所有线路。可用
`-All -ShardIndex 0 -ShardCount 6` 运行第 0 片；索引从 0 开始，所有片必须完成
才能称为全矩阵验证。`-ListOnly -AsJson` 可检查分配；空片和非法参数会失败。

## 验证与效果口径

聚焦回归：`test_gateway_standalone_ci_plan.py`、`test_verify_gateway_line_shards.py`，
以及现有 verifier execution/list-only、standalone CI、release candidate 契约。
必须验证 6 片无遗漏、无重复、数量均衡，以及错误比较和未知路径不会静默跳过。

调整前 Windows job 为约 310 分钟，其中线路矩阵 259 分钟。
本次移除了普通 UI/文档 PR 的该项成本；全量改动将该成本分片并与产品测试重叠。
并行减少墙钟等待，不保证减少总 runner 分钟；首轮冷缓存、排队和缓存驱逐仍影响时间。
实际提速需以新提交的 GitHub job 耗时验证，不将理论除以 6 当成已测结果。
