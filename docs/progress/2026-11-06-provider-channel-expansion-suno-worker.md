# Suno 本地 browser worker 生命周期子批

日期：2026-11-06。目录：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
这是 R15 的本地执行资源子链，不是 Suno 整卡、有效音乐、OAuth/MCP、P50 或总计划完成。

## 1. 本批改了什么

- `src/upstream/suno/browser_worker.rs` 复用既有进程 supervisor、16 个共享进程 slot、Windows JobObject / Unix process group、取消信号和 terminate/reap owner，不复制另一套回收实现。
- Suno 使用独立 metadata 预算：stdin 1 MiB、stdout 128 KiB、stderr 16 KiB。输入在 spawn 前拒绝；输出两管道与 stdin/wait 并发处理，超量或非零退出不接受成功 JSON。
- 本地外层 timeout 包含 slot 排队、启动及 stdin 写入；caller Drop 通知 supervisor，由它继续负责自有进程/子孙回收。Suno 超时投影为 504；进程错误用 Suno 代码与静态安全信息，不外露 Producer 身份或原始管道正文。
- browser-backed 调用给本地/远端 worker 与最终响应共用一个 deadline，执行错误统一禁止重试与 fallback；本地 worker 的成功 JSON 不覆盖非零 exit，坏 JSON 不回显原始 stdout。
- JS 不再 `process.exit(0)`。生成返回结果后先关闭 owned browser 或断开本次 CDP 连接，再尝试释放 EasyBrowser lease，最后 await stdout write callback，保留 CLI 的 exit 0 + 结构化业务错误合同。
- 关闭失败仍尝试释放 lease；release 有 5 秒本地等待预算并拒绝非成功 HTTP，必要清理失败不能伪装成功。输入有 1 MiB 上限，非法 target 在获取 lease/浏览器/生成之前拒绝。
- 共享进程文件原 491 有效行，增加预算入口前将现有测试原样移至子模块；不增加超限例外、不改其他服务商的默认预算或进程语义。

## 2. 验证与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-worker-20261106/`。
最终状态以该目录的 `closeout.json`、`audit.json` 及各阶段日志为准，不把编译、模拟 complete 或 URL 当有效音频验收。

- Node 首次 fixture 因 Windows `--import` 文件路径写法失败；改成 file URL 后真实红测 0 passed / 3 failed，明确观测到仅 emit、没有 close/release。失败日志均保留。
- Rust 在不改变旧 transport 行为的显式 launch seam 上真实红测 0 passed / 3 failed，复现非零退出仍成功、输出无预算以及业务错误可重试。该阶段 3313 项选定 Rust/Cargo 输入无漂移。
- Node 最终定向 18/18 通过，包含实际 Chromium + 新隔离 profile + 回环页面/HTTP/EasyBrowser：成功和 challenge 两次都释放 lease，借用浏览器/原页仍可用且不导航/增页，生成计数没有增加第二次。不是实际 Suno、真实音乐、TLS 指纹或用户浏览器验收。
- Node v2 阶段 18/18；随后给 native 测试自身的子进程增加 AbortSignal 超时，再定向重跑该项 1/1，按名去重仍为 18 而非 19。两阶段各冻结 1188 项 scripts 输入无漂移，跨阶段差异仅该 native 测试及独立 Rust fixture，其余 17 项实际依赖相同；不声称所有历史阶段覆盖整个仓库。
- Rust 正常 Suno 146/146（本批新增 9）、共享 supervisor/security/原 Producer 相邻 30/30 通过，合计按完整名去重 176；超时与 caller Drop 两项实际 Node 子孙进程通过随机回环 TCP probe 的 EOF/Reset 验证退出。默认线程栈，不设置 `RUST_MIN_STACK`。正常 `runtime-defaults` 生产非测试库与 `cargo fmt --all -- --check` 通过；三个最终 Rust 阶段各 3314 项选定输入无漂移。
- checker 31/31、ratchet 4694 文件通过，14 项 immutable runtime artifacts 单列，governed source >700 为 0。根 `artifacts/effective-code-lines.json` 直接备份并按 SHA256 逐字节恢复；没有改 baseline/exception。
- 13 项源码最大 403 有效行，7 项新增最大 212。共享进程文件 491→290、独立旧测试 212；Suno owner 258→245。两份漏备份的测试 preimage 仅撤去本批精确一行，并分别与前批最终来源/本批较早 Node 来源 SHA256 相同，登记为重建而非直接备份。
- 安全/资源审阅覆盖显式无 shell 启动、输入/管道准入、秘密正文省略、共享 semaphore/JobObject/drop、非零退出、取消/join、借用浏览器不关闭及正常清理的错误语义。未新增依赖、秘密 store 或另一套进程 owner；下节未完边界不因静态审阅而抹去。
- 最终 18 项源码/文档 UTF-8 无 BOM、来源与 scope diff、248 个本地链接审计通过；共享旧测试移出后去空白 token 一致。Rust runtime fixture 的补充快照在编译期间、实际执行前采集，不冒称包含在启动时的 3314 项 Rust/Cargo 快照中。收尾未见本批匹配的 Node/Chromium/调查进程残留，在线旧 Gateway/UI 未操作。

## 3. 仍未完成及不能推出的结论

- **强制终止/调用取消不能保证 JS 清理代码运行，也不保证远端 EasyBrowser lease 已释放。** 本地回收与正常完成/业务错误清理分开证明；硬终止后的远端 lease 对账/过期回收未实现，不能声称通用 lease 生命周期已闭合。
- 本批 stdout/stderr 预算不是 Playwright `.text()` 内存预算。browser worker 现有页面响应读取、部分 feed/失败 clip/等待超时的结果语义仍待与严格原 ID 全量终态对齐；没有把直连上一批的保证外推到浏览器分支。
- 远端 executor 的具体子进程回收、lease、跨进程/Server/原生包未由本批本地测试证明。browser-backed 最终图片下载的完整安全与内存预算也不是本批结论。
- 当前 Web 模型/feed wire、持久 device/Clerk、challenge/刷新、有效音频读取/播放、精确媒体 probe/费用/UI/公共结算与官方 OAuth 客户端/工具/生命周期继续接力。
- Unix process-group 仅复用现有实现，本机测试是 Windows；不报跨平台与完整 feature 矩阵绿。上一批仅 Suno 最小组合的 Gemini Web 缺符号失败保留，不接管 S06。
- 无凭据开发继续；模板仍 16，无新增整卡 `offline_ready`。P50 全范围集成/新 devtest、P60 集中录制、P70 真实逐渠道验收未完成。

没有提交、推送、打包、改在线实例或使用真实凭据/平台生成。P34 Redis 授权此前已消费并清理，本批不再复用，也没有创建容器。
