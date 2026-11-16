# 服务商分渠道扩展：Udio Rust worker 生命周期与禁止重放

日期：2026-11-06。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批承接 R16 的 [浏览器任务子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-udio-tasks.md)，不代表完整 Udio、P50 或总目标完成。模板仍为 16 个，没有新增整卡 `offline_ready`。

## 1. 本批实际修改

### 本地进程复用既有资源 owner

[Udio owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/udio.rs) 不再自行 spawn / write / `wait_with_output`，委派给新增 [browser_worker](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/udio/browser_worker.rs)，复用原 `execute_browser_worker_process_with_limits`、16 个共享 slots、Windows JobObject / Unix process group 和 cancellation/reaper。没有新增另一套进程监管器，也未修改共享进程实现或 Gemini/S06 源码。

- stdin 序列化结果最大 1 MiB，超限在启动前拒绝；stdout 128 KiB、stderr 16 KiB，由既有并发 pipe reader 限制。不是序列化之前的对象、整个 heap 或远端响应的统一预算。
- 外层 timeout 包住共享 slot 等待、输入写入与进程等待，不再只限定 `child.wait`。非零退出即使 stdout 含 success JSON 也失败；非法 UTF-8、空或坏 JSON 不回显原始输出。
- 共享 Producer 的进程错误投影成 Udio code/provider；任意 OS / pipe / stderr 诊断不直接作为公开错误正文。结构化业务错误保留现有安全分类。
- 超时或 caller Drop 向既有 supervisor 发送取消，由它保留进程树和 permit 直到清理；**不保证调用者收到错误之前清理已经结束**。本批验证的是 Windows 下实际 Node 子孙进程的最终退出，不是 Unix 或全部 Chromium 强杀场景。

### 本地与远端调用均不因不确定失败再次生成

[远端 dispatch](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/browser_executor.rs) 对已配置的 Udio executor 禁止跟随重定向；HTTP / 网络 / 解码 / 缺 result / 结构化失败不返回 `None` 以请求本地 fallback。未配置远端且显式允许本地执行时，原本地入口继续可用。其他服务商策略未扩大；既有 Gemini media 仍沿原 no-replay 条件。

`execute_udio_media` 对 worker / remote dispatch 和现有最终响应共用一个异步 timeout，执行内错误统一 `retryable=false`、`FallbackHint::Abort`、provider `udio_compatible`。正常 retry helper 不再重试这些错误，失败也不表示允许换号或换服务商重提。超时返回 504；既有 request-timeout 预算计算保持原样，没有在本批修改 challenge / wait 参数产品策略。

这是**本次应用调用边界的禁止自动重放**，不是跨请求、进程崩溃或外部客户端重发的全局 exactly-once 保证；没有增加持久操作 ID 或补偿协议。关闭本地连接和停止等待不保证撤回远端任务、取消供应商扣额或退款；远端 executor 的取消传播及其真实进程/lease 回收仍未验。

## 2. 测试与可复验证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-rust-20261106/`。

先将旧进程代码原语义提取到显式 launch seam，再运行红测，避免测试期间改全局 Node 环境变量。真实 Rust 红测 **0 passed / 4 failed**，分别复现非零退出被当成功、stdout 无预算、业务错误可重试和远端失败转本地；不是编译失败冒充业务红测。记录在 `rust-red.json / rust-red.log`。

修复后的 `cargo test --locked --offline --no-default-features --features runtime-defaults --jobs 4 --lib udio -- --nocapture` 为 **187 passed / 0 failed / 0 ignored**，新增 16 项。筛选串 `udio` 同时匹配 AIStudio，187 不是纯 Udio 测试数量。相邻 `producer_browser_worker / browser_executor / suno::tests::worker / pipeline::stage_send::tests::policy` 组合为 **155 passed / 0 failed / 4 ignored**；按完整名去重共 **335 passed**。4 项 ignored 为既有 Redis lease-release opt-in，不计通过、不复用 P34 授权创建容器。正常 `runtime-defaults` 生产非测试库 `cargo check --locked --offline --no-default-features --features runtime-defaults --jobs 4 --lib` 与格式检查通过。完整命令及输入以 `audit.json` / 各阶段回执为准；不把多阶段重叠测试直接相加。

新增覆盖本地非零退出、两路输出上限、输入启动前准入、spawn failure、空/坏 JSON 与坏 UTF-8、正常 pending 身份、结构化错误 no-retry，以及超时和 caller Drop 后实际 Node 子孙的随机回环 TCP probe EOF/Reset。远端夹具覆盖三种媒体 endpoint 的 HTTP / 坏 JSON / 缺结果 / 业务错误、连接失败、307/308 各只一次 POST、无远端时本地策略，以及 `execute_udio_media` 在远端 success 但结果解析失败后的 Abort。

本批没有浏览器或平台调用。前批 139 项 Node（含九场景真实隔离 Chromium/CDP）的输入未改，本批不重复运行并累计它们。本批新增 `.mjs` 仅为测试夹具，位于既有排除出包的 `scripts/tests/fixtures`，不是新增运行 payload。

8 项源码最大 196 有效行，5 项新增最大 167；`udio.rs` 212→196，shared remote owner 167→168。checker **31/31**、ratchet **4715 files** 与 Rustfmt/diff 通过，无新例外、无 baseline 改动；根 effective-lines 报告直接备份后按字节恢复。Rust 各阶段冻结的是 2632 项选定 Rust/Cargo/fixture 输入，不宣称覆盖整个仓库或所有构建环境。

## 3. 下一步仍须接的内容

1. **local / remote 成功结果准入**：当前旧 parser 只做有限结构及非空 trackIds 校验。需按原 ID 完整集合、唯一/合法 ID、目标资产和等待策略验证，不能让旧/异常远端返回绕过 JS 已补的终态校验。
2. 正常公共音乐 pipeline、精确媒体 probe/逐次费用确认/UI、usage/audit/本地结算与 Keep 全链。已有图片最终下载的 URL/读取/解码预算也不是本批安全验收。
3. 当前模型/参数/生成与查询 wire、Supabase session/refresh/受众/challenge、真实有效音乐与适用播放/嵌入权益仍缺；见 [Udio 渠道合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-udio-channel-contract.md)。前批联网记录的下载限制保留，不实现绕行。
4. 远端 executor 实例的版本/资源/取消/lease、跨进程和 Server、原生包及完整 feature 矩阵仍缺；本批回环响应不能证明这些已完成。
5. 其他原始渠道尤其 Luma Web / FreeBuff 公共业务、P50 全范围无凭据集成与 devtest、P60 集中录制、P70 真实逐渠道验收继续，不以该元数据子链代替总目标。

本批无依赖升级、真实凭据/平台生成/费用、在线 Gateway 或 UI / Redis / 容器操作、出包、提交或推送；P34 已消费的单容器授权不复用。
