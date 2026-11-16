# 服务商分渠道扩展：Udio 结果准入与公共音乐元数据结算

日期：2026-11-06。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批承接 [Rust worker 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-udio-worker.md)，闭合限定成功结果准入和正常公共音乐元数据的本地结算子链，不代表完整 Udio、P50 或总计划完成。模板仍 16 个，无新增整卡 `offline_ready`。

## 1. 实际修改

### 原始结果先校验，再解码

新增 [admission](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/udio/admission.rs)，在正常执行的 local/remote 合流处，以及 executor service 返回 success envelope 前复用同一校验：

- 原 track IDs 必须 1–16 个，唯一、非空、无首尾空白、每个不超过 128 UTF-8 字节；songs 最多 16 项，逐项为对象、ID 合法唯一且属于原集合，不允许旧 decoder 静默筛掉坏项。
- `status`、`state` 分别检查类型、空白、64 字节上限及失败状态；任一为 error/failed/canceled/cancelled，不能被 finished 或其他状态掩盖。
- 完成须同一快照含全部原 ID，每首含目标资产，并有原始布尔 `finished=true` 或终态 status/state。字符串 `"true"`、early URL、readyToStream 不能单独证明完成；资产别名保持 JS 的首个非 null 优先级，错误类型不能被后续 alias 绕过。
- `completed` 必须与实际完整终态一致；默认等待不允许 pending 成功。显式 no-wait 仍允许合法空/部分 snapshot，并按原 ID 顺序返回全部任务，缺项补 pending；若提交时已经全部完成，no-wait 也可返回 completed=true。
- 新增拒绝统一 502、Udio provider、禁止 retry/fallback，错误不回显原始 metadata。保留既有 legacy helper 的独立合同，未把整个 protocol decoder 一并重写。

### 公共链发现并修复 Cookie 身份覆盖

真实公共 pipeline 测试发现：共享 header builder 先生成的账号 session Cookie 会被入站 caller Cookie 覆盖。现仅在 [Udio 会话准备](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/udio_response_helpers.rs)隔离 caller extra headers，Cookie/UA/Origin/Referer/语言继续来自选中账号的 session auth/custom headers 或 browser runtime；prompt、模型、参数和等待设置仍来自正常请求。没有改其他服务商的全局 header 策略。caller Cookie 也不能凭空创建账号 runtime。

### 远端只接有界 metadata

[远端响应 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/browser_executor_response.rs)对 Udio 使用 128 KiB 解码正文上限，复用 [既有 bounded reader](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/upstream_body.rs)的声明长度和实际 chunk 追加前检查。其他服务商保留原 64 MiB 默认合同；不是整个 heap、wire、累计请求或媒体下载预算。已配置远端后的失败继续禁止转本地重提。

## 2. 正常公共链证明了什么

[公共 music 测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/tests/udio_pipeline.rs)使用生产 Local bootstrap、正常管理 API 创建普通 Key/次数余额、正常 `/v1/music/generations` router oneshot、真实 loopback HTTP executor 和真实 SQLite：

- 配置无关模型账号、精确账号、disabled 账号及备用 provider，验证请求别名/解析模型/真实 credential ID、一次远端提交、账号 Cookie 与 executor Bearer；不混入 caller Cookie/Origin。
- 完整终态 metadata 和显式 pending metadata 均按正常请求成功结算一次，余额 2→1；pending 的 audit completed 表示这次 HTTP 调用完成，不等于音乐任务已终态。
- HTTP 不确定失败、部分/外来歌曲、默认等待 pending 不作为成功，正常 refund 2→2，不换号、不 fallback。未知 token usage 在响应和 audit 中保留未知，不补零。
- caller Drop 在远端已收到请求后执行；有界 Notify 同步而非固定 sleep。验证 cancelled audit、预占退款、并发归零、账号 failure_count 不增加和远端响应 source Drop。只退款 Gateway 本地预占，不承诺供应商退款或远端任务撤回。

这是正常公共业务的**音乐元数据/Local 结算子集**，不是 UI→真实入站 TCP、独立进程、Server/PG、远端 executor service 的实际 browser 进程、当前平台或真实媒体验收。`https://asset.invalid/not-audio` 是合成假资产，未下载、播放或绕过官方限制。

## 3. 验证、失败与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-admission-20261106/`；最终来源和范围见 `audit.json / closeout.json / passed-test-names.json / new-test-names.json / scoped.diff`。

- 首次 fixture 路径导致 formatter/编译失败，保留 `rust-red`，不当作业务红测。修正后 `rust-red-v2` **0 passed / 4 failed**，真实复现部分 complete、外来歌曲、failed+finished 和默认等待 pending 假成功。
- 首轮组合 `rust-green` **203 passed / 4 failed**：1 项为 gzip fixture 的 OddLength，3 项公共测试暴露真实 caller Cookie 覆盖；两种责任分别修复，未放宽断言。最终 `rust-green-v2` **208 passed / 0 failed**，其中 **21 项新增**；`udio` 筛选同时匹配 AIStudio，208 不是纯 Udio 数量。
- 相邻 reader **12/12**、executor **83 passed / 4 ignored**、Suno worker **9/9**、retry policy **25/25**；与上述 208 按完整测试名去重 **327 passed**。4 项 ignored 为既有 Redis lease opt-in，未运行、不计通过、不复用 P34 授权。前批未变 JS 测试未重跑，不重复累计。
- 正常 `runtime-defaults` 生产非测试 lib `cargo check`、Rustfmt、checker **31/31**、ratchet **4723 files**、UTF-8 无 BOM 与 diff 检查通过。Rust 七个最终阶段各 **2642** 项选定 Rust/Cargo/build/fixture 输入无漂移；不是全仓/全 feature 矩阵证明。
- 13 项审阅源码最大 **271** 有效行，8 项新增最大 212；`udio.rs` 196→201，header 准备 owner 127→127，共享 reader 247→271。无 baseline/exception 改动；根 effective-lines 报告直接备份并逐字节恢复。header owner 是发现 Cookie 缺陷后补做直接备份，SHA256 与此前红测及首轮组合 snapshot 一致，回执为 `late-backup.json`。

## 4. 接下来仍须完成

当前平台模型/参数/生成及查询 wire、Supabase session/refresh/受众/challenge、有效音乐与适用播放/嵌入、精确媒体 probe/逐次费用/UI/Keep 整链，以及原 image 下载路径的 URL/读取/解码准入仍未闭合。远端取消/lease、跨请求 exactly-once、跨进程/Server/完整 feature 矩阵/原生包继续；官方限制见 [Udio 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-udio-channel-contract.md)，不增加下载绕行。

其他原渠道尤其 Luma Web、FreeBuff 公共业务、Suno 公共链和 OAuth 候选、Kiro/Qwen OAuth 未完项，以及 P50/P60/P70 保持原范围。无真实凭据/平台生成/费用、在线实例/数据库变更、Redis/容器、出包、提交或推送；历史 P34 单容器授权不复用。
