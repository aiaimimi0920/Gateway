# 相似网关参考特性后续闭环

继续沿用 [首批研究](2026-10-19-reference-gateway-adoption.md) 的 2026-10-19
官方 GitHub 固定 SHA，不把这次实现日期当作再次追踪上游新提交。只参考可观察行为，
使用本仓库 Rust、pipeline、AppState、LocalRuntime / PostgreSQL owners 独立实现。

## 已选择的范围

1. gpt-load 的明确 `Retry-After` 模型冷却，接通 typed evidence、失败候选、两类
   存储 writer、reader 和候选排序。不是仅改等待公式或管理列表中的一个显示值。
2. AxonHub 的流式/非流式分开延迟学习，接通真实成功采样与普通候选路由消费，
   使用有界、过期、冷启动中性的进程级 owner，不另起负载均衡/指标系统。
3. 直接相关的 sticky correctness：blocked 不被提升，真实 affinity 不被新增排序
   覆盖。健康 sticky 采用 queue 已有的队首提升合同，包含跨权重/priority 的提升。

详细运行合同见 [等待策略](../retry-wait-policy.md) 与
[延迟排序](../routing-latency.md)。不新增依赖、schema、存储版本、基线或有效行例外。

## 实际定位与修复的接线问题

- 初始真实 loopback 测试显示最终失败没有 winning selection，旧 finalize 不写模型
  行。新增 dispatch 候选限流记录，保持 fallback 归属，避免只在人工构造 context 下
  可见。正常最终失败经过 finalize 的计数和 fallback 成功时的 48 小时 deadline 均
  有保护断言。
- ID 缺失而 payload 有 credential ref 时，原 Local/PG reader 使用空 ref，与 writer
  key 不同。两种 reader 都采用 payload ref fallback，复用原 identity 派生，不改历史
  ID；共用合同同时覆盖 ID 与 ref-only 两种身份。
- 原 affinity 只改 `all_candidates` 顺序，未传给 queue；新增延迟 tie-break 可覆盖它。
  现在过滤/模型健康之后由 queue 提升健康 sticky，真实 Local affinity 路径有回归测试。

## 本批明确不采用的方案

模型到期后确实缺少跨进程单探针，但当前冷却是允许末尾兜底的软语义。只有一把
进程锁或短 Redis TTL，既不能阻止其他实例发送，也不能隔离过期旧 owner 的结果。
完整实现需要硬发送 gate、可回收租约、generation/token fencing、取消/流终态处理与
启动迁移，且改变“全部候选冷却时仍可尝试”的可用性合同。本批不收编这种半实现，
也不把它列为已完成能力。SSE 裸 CR 和拼接 JSON 的前轮边界同样没有在此批放宽。

不搬入支付、用户中心、ensemble、独立执行器，不接管 S06 Gemini 业务实现。共享
`GatewayError` 新可选字段只要求其一个 cfg(test) initializer 机械补 `None`，不表示
S06 内部 HTTP 头已经接入。没有真实供应商调用、提交、推送、部署或发行包替换。

## 验证记录

执行证据保存在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-reference-followup-20261019/`。
最终结果如下；编译、实际执行、显式 opt-in 和发行验收分别报告。

初始实际运行负例已分别触发：冷却失败行缺失、冷却 sticky 被提升、成功样本未被
真实 route 消费。初轮夹具在断言 panic 时又遇到 Windows SQLite 句柄清理冲突，
导致进程 abort；后续先关闭 fixture 再断言，并在多断言存储合同外捕获/清理后重抛。
不将 abort 或修复前的编译失败当作 green。

全库首轮另发现新增 affinity 夹具目录名不符合既有 `gateway-local-` 清理保护前缀，
修正夹具命名后重新编译、单项执行并重跑全库。发送合同并发执行时曾有一次 Windows
SQLite 清理 `os error 32`；未修改受保护旧夹具，随后完整串行重跑 29 项通过。
这些失败日志均保留，没有改写成成功记录。

| 最终检查 | 实际结果 | 日志 |
| --- | --- | --- |
| 全部 library tests，4 threads | 3448 passed，0 failed，36 ignored | `final-lib-rerun.log` |
| 新参考路由合同默认执行 | 14 passed，0 failed，PG 1 ignored | `final-reference-routing.log` |
| 隔离 PostgreSQL 显式执行 ignored 合同 | 1 passed，0 failed | `final-reference-routing-pg.log` |
| 原发送运行时合同，串行 | 29 passed，0 failed | `final-pipeline-send-serial.log` |
| SSE / Responses 等另外 5 个合同 target | 32 passed，0 failed | `final-reference_stream*`、`final-responses_*`、`final-sse_*` |
| 重试等待、预算、日志与 HTTP 分类 4 targets | 14 passed，0 failed | `final-retry-contracts.log` |
| `cargo check --locked --all-targets` | exit 0 | `final-all-targets-check.log` |
| `cargo fmt --all -- --check` | exit 0 | `final-fmt-check.log` |
| checker tests | 31 passed，0 failed | `final-checker-tests.log` |
| effective-line ratchet / strict | 均 exit 0；2981 files；14 immutable runtime artifacts；governed source >700 = 0 | `final-ratchet.log`、`final-strict.log` |
| line manifests | 44 checked，0 errors | `final-manifests.log` |
| `git diff --check` | exit 0 | `final-diff-check.log` |

合计实际执行了 11 个 integration targets 中的 90 个测试；PG 项不是仅保持 ignored
而未验证。它使用本批独占的 loopback `25439`、专用数据库与 run guard、新 UUID
schema 和 tmpfs PostgreSQL 16 容器。测试结束后按 ID、label、mount、port 核实身份，
只停止这个容器，没有删除容器、volume 或接触已有 `5432` / `6379` 服务。

Rust 测试均把 `TEMP` / `TMP` 指向上述证据根的 `temp/`，路由 library tests 的 Redis
地址显式指向不可用的 loopback `1` 端口，不借用宿主业务 Redis。`--no-run` 编译日志
列出实际 test EXE；全库使用修正夹具后重新编译的 EXE，而不是前轮旧包。

本批 45 个 Rust 文件的有效行最大值为 469；新 latency owner 124，失败候选 health
owner 45。主要旧文件为 send 401→438、route 459→441、queue 387→425、PG model
states 393→403，无需例外或 baseline 重建。安全/资源复核确认：typed evidence 不
公开序列化；持久化沿用脱敏 owner；latency key 与条目数有界，无新后台任务、持锁
await、全缓存逐请求扫描；健康与授权仍在延迟建议之上。两次独立只读复核分别覆盖
ref-only writer/reader identity 与真实 affinity 接线，实际执行结果以上述日志为准。

最终源码回执 `final-source-receipt.json` 记录本批 48 个文件的 SHA256、UTF-8 无 BOM，
以及开始时已有 47 个 dirty / untracked / missing 项的字节或缺失状态保持不变。
HEAD 保持 `a8740888e9f9a6720f9dfc389ec6424af5be27dd`。

这不是全仓所有 integration / ignored fixtures、发行构建、打包运行、桌面手测、真实
供应商或线上验收。本批达到上述源码功能与本地回归范围；没有提交、推送、部署或
更新用户正在运行的程序。
