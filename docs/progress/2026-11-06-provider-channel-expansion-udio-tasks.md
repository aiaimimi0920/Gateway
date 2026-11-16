# 服务商分渠道扩展：Udio 浏览器任务终态与单次提交

日期：2026-11-06。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批推进 R16 的原浏览器入口，不是有效音乐、完整 Udio、P50 或总目标完成。模板仍为 16 个，没有新增整卡 `offline_ready`。

## 1. 先核实的产品限制

本次实际联网读取两篇官方帮助与匿名 create 页面：官方目前仍说明没有公共 API，audio/video/stems 下载已停用，匿名 create 重定向登录。来源和精确限制见 [Udio 渠道合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-udio-channel-contract.md)。

保留 `udio-web`，不把第三方代理 Key 当官方 API，不虚构 OAuth，不增加下载绕行。旧 `/api/generate-proxy`、`/api/songs` wire 继续明确标为未完成当前平台核实，不能把回环夹具当在线生成或播放证据。

## 2. 本批实际修改

### 一次提交与完整原 ID 集

新增 [generation owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/udio-browser/generation.mjs)，由原 [worker](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/udio-browser-worker.mjs) 接线；保留原字段和普通输出 envelope。

- 严格验证 1–16 个原始 track ID：非空字符串、无首尾空白、UTF-8≤128 bytes、唯一；显式错误数组不再静默筛选/回退，冲突 alias 拒绝。
- songs 最多 16 项，坏对象/ID、重复 ID、非原歌曲、坏 status 和明确失败/取消终态不伪装成功；查询结果恢复原始 ID 顺序。
- 必须同一快照包含全部原始歌曲，且每首有完成状态/finished 与目标资产字段；early URL 或 readyToStream 不是最终完成。不合并 disjoint partial snapshots。
- no-wait 保留完整 pending trackIds，`completed=false`；不能跳过坏项/失败校验。默认等待超时返回 504，不输出成功的 incomplete 结果。
- worker 只调用一次 submit；challenge/HTTP/transport/坏结果不在此 owner 内自动重提。原 native UI 的 challenge-cycle 再次 click、外层 challenge 后再次 submit 均已移除。
- 从输入处理后建立同一 deadline，提交/查询前后检查余量；不再用 `Math.max(60_000, remaining)` 扩大预算。前置浏览器/认证和持久化的完整硬时限仍需后端 supervisor 闭合，不能称整个进程生命周期已受控。

**禁止重提的已证范围是本 JS worker。** Rust 的旧等待/错误分类、Gateway 外层 retry/fallback、远端 executor 与结果准入尚未完成，不能外推成公共调用已保证只生成一次。

### native 准入与响应处理

新增 [native-submit](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/udio-browser/native-submit.mjs)，通过原 native-flow 委派，复用现有 `createProgramResponseCapture`；没有修改 Gemini/S06 源码或测试。

- 点击前安装并 await ready；pageOnly、固定配置 origin/路径、仅选择 POST、128 KiB 单响应 decoded-body 准入，只有一次 click。模糊二次生成/redirect 响应拒绝。
- success HTML 在正文访问前拒绝；native onError 优先于空正文结算；坏 JSON、HTTP/native/transport 错误不回显本次元数据原文或 CDP 诊断。
- 等待和 body 共用 timeout，finally 停止 capture；click/ready/timeout 异常不留下 unhandled response rejection。
- 页面内 fetch 的元数据流上限从 16 MiB 收紧为 128 KiB，溢出 cancel reader、释放 lock；禁止 fetch 自动跟随重定向，分块 UTF-8 和小 chunk 仍保真。
- generation owner 在 `JSON.parse` 前执行 1 MiB 累计准入，耗尽后不发下一次查询。此预算**不是累计实际读取、heap 或 wire 的严格上限**，单响应可先在自己的 128 KiB 上限内被读入，再被累计准入拒绝。

pageOnly 仍可能短暂 auto-attach 其他页面再 detach；不宣称完全没有 Target 事件。native redirect 拒绝不是阻止浏览器自身跟随网络跳转。capture.stop 和本地取消都不保证撤回远端生成、退款或取消页面原请求。

### 正常退出

借用 CDP 现在正常断开本次连接，不关闭用户浏览器/原页；自建浏览器照常关闭。必需关闭失败返回安全错误，不伪装成功；原 runtime state 的条件持久化保留。stdin 新增 1 MiB 上限，stdout 改为 await write callback，不再 `process.exit(0)` 截断输出。

这不是 Rust 子进程树、stdout/stderr 上限、强取消、远端 executor、Server 或原生包验收。旧 runtime state 的 best-effort 保存也没有被改称强持久化保证。

## 3. 测试与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-20261106/`。

实现前修正夹具时钟后，真实 whole-entry 红测为 **0 passed / 11 failed**（`node-red-v2.log`），覆盖部分结果、坏/重复/失败/外来 ID、disjoint、超时、challenge 重提、预算扩大、借用连接与清理假成功。首轮夹具使用过去的固定时钟，导致相邻重试 helper 提前超时；其 2 pass/9 fail 只作为夹具问题记录，不冒称完整业务红测。

中间 core 43 项、native 18 项、相邻 91 项均通过，不能直接相加。最终 `--test-concurrency=4` 的 17 文件组合为 **139 passed / 0 failed / 0 skipped**：Udio 102 项（39 新增）与共享 native 37 项，按完整名去重。最终源码身份以本目录 `node-final.json / node-final.log / audit.json / closeout.json` 为准；整个组合冻结1204项scripts输入，运行期间无漂移。

真实隔离 Chromium/CDP 实际走 worker 子进程、native reader 和页面内流式 fetch，覆盖九场景：partial→逆序完整歌曲、chunked、解压后超限 gzip、200 HTML、HTTP 503、坏 JSON、重复 ID、challenge、失败歌曲。partial 场景在本次响应返回前，让同 context 其他页发相同地址请求；本次仍只接受原页结果，查询仅包含原 IDs。正常结束后两页/浏览器存活、他页 URL 不变且计时器继续。所有服务为回环，全新 profile 在 linshi，无真实账号或平台请求。

合成包合同已验证实际新增模块的递归载荷、support records/hash/checksums 及 import/export，不是新 EXE 或真实 devtest 包。checker31/ratchet4710/diff通过，根 report直接备份并精确恢复。14源码（7新增/7修改）最大300有效行，新文件最大95，无例外；worker476→300、native-flow175→89。正式语法/编码、依赖/Rust不变和最终资源审计另见回执。

## 4. 下一步与未完

1. **优先补 Udio Rust local/remote 调用 owner**：现有旧 Node spawn/wait_with_output 没有共享 supervisor/stdio/树回收合同，错误也尚未统一禁止 Gateway retry/fallback；必须先闭合，不能用本批 JS 单次提交当公共防重放证明。
2. local/remote worker 成功 envelope 仍只做有限旧校验，需以原 IDs/目标输出/等待策略准入；正常公共音乐 pipeline、usage/audit/结算、精确媒体 probe/费用确认/UI 与 Keep 全链继续。
3. 当前模型/参数/生成及查询 wire、session/refresh/受众/challenge、真实有效音乐与平台内播放/嵌入权益仍缺。音频/视频/stems 下载已停用，不通过绕行补成虚假“全功能”。
4. 其他原始渠道、P50 全范围无凭据集成/devtest、P60 集中录制与 P70 真实逐渠道验收保留，总目标 active；不继续无限打磨此元数据子链替代 Luma Web/FreeBuff 等核心缺口。

本批无 Rust/依赖改动，无真实凭据/平台生成/付费请求、在线实例/容器操作、出包/提交/推送；P34 已消费授权不再次复用。
