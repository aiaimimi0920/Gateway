# 服务商分渠道扩展：Suno 浏览器完整终态与 native 响应准入

日期：2026-11-06。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
这是 R15 的一个已验证离线子批，承接 [worker 生命周期子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-suno-worker.md)。不代表有效音频、完整 Suno 渠道、P50 或总目标完成；模板仍为 16 个，无新增整卡 `offline_ready`。

## 1. 本批解决的问题

原浏览器路径可能把单个 complete clip 当作整次生成完成，默许坏 ID/重复 ID，并在默认等待超时后输出成功的 incomplete envelope。生成与 feed 还通过普通 Playwright `response.text()` 读取，不能提供 native decoded-byte 预准入。

新增 [clip-state](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/suno-browser/clip-state.mjs)、[generation](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/suno-browser/generation.mjs)、[responses](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/suno-browser/responses.mjs) 三个单责模块，由原 [worker](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/suno-browser-worker.mjs) 与 [页面操作入口](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/suno-browser/browser.mjs) 接线。没有另建浏览器池、native reader 或并行生命周期。

### 完成语义

- 提交结果要求 1–16 个 clip；每项 ID 非空、无首尾空白、UTF-8 不超过 128 bytes、不可重复；提供的 status 必须为非空字符串、无首尾空白、不超过 64 bytes。
- 页面 feed 可能是账号全局列表，最多接收 128 项，逐项验证 ID/status，仅选原始提交 ID 并恢复原顺序。合法其他歌曲不计入完成，其失败也不使本次失败；重复的外来 ID 仍拒绝。
- 只接受同一个快照内全部原始 ID 都成功且含目标资产字段。不会把 t1 的 A 与 t2 的 B 拼接成一次完成。
- 原始 clip 的 `error / failed / canceled / cancelled`（不区分大小写）立即失败；未知状态、缺资产、部分集合不能证明完成。
- `waitCompletion=false` 保留完整 pending ID 集与 `completed=false`；坏项或失败项不能借 no-wait 伪装成功。默认等待超时返回 504，不返回成功的 incomplete envelope。
- 仅被动观察页面已有 POST feed，不增加主动轮询、不改当前请求 schema、不重提生成或换号。

### 响应所有权与上限

复用既有 `createProgramResponseCapture`，在点击前安装并等待 ready，设置 `pageOnly:true / failOnBodyError:true`。本批只导入共享 owner，**没有修改 Gemini/S06 源码或测试**。

- 精确绑定配置 baseUrl 的 origin 与 generation/feed pathname；只有 POST 进入业务队列。生成响应被观察前的 feed 忽略，第二个 generation 响应视为归属不明并失败。
- 单响应 native decoded-body/text 上限 128 KiB，应用层最多排队 16 个 response handle；JSON 累计准入 1 MiB，检查在 `JSON.parse` 前执行，预算耗尽后不再开始下一次 native read。
- **累计准入不是累计实际读取、heap 或 wire 的严格 1 MiB 上限。** 剩余额度不足一个响应时，已经通过单响应预准入的至多 128 KiB 正文可能先被读取，再因累计超限被拒；共享 owner 另有自己的 pending/envelope 上限。
- 成功响应必须为 `application/json`；HTML 在正文访问前拒绝。HTTP 错误、坏 JSON 与 native 错误均输出静态安全错误，不回显原始文档/CDP 诊断；native onError 优先于 reader 的空字符串结算。
- redirect 响应拒绝，不宣称阻止了浏览器自身可能自动跟随的网络跳转。
- 每次 next 的超时覆盖等待和正文读取；超时停止 capture、阻断迟到响应。click 先注册 waiter 并附拒绝处理，click 失败不会遗留 unhandled rejection。
- 正常清理顺序为 capture.stop → owned browser close 或本次 CDP disconnect → lease release → await 输出；清理失败不伪装成功。capture.stop 不是取消页面原网络请求，也不保证撤回远端任务或退款。

## 2. 定向验证与真实边界

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-browser-20261106/`。

| 检查 | 实际结果 | 说明 |
| --- | --- | --- |
| 实现前 CLI 算法红测 | 1 passed / 8 failed | `node-red.log`；真实复现部分集合误完成、坏/重复/失败 ID、disjoint/外来集合与超时假成功 |
| 最终 Suno Node 组合 | 53 passed / 0 failed / 0 skipped | `node-green-v2.log`；比前批 18 项新增 35 项，早期阶段不重复累计 |
| 共享 native capture 相邻回归 | 37 passed / 0 failed / 0 skipped | `shared-capture.log`；与 Suno 组合按完整名去重共 90 项 |
| 包合同 | 1 passed | `package.log`；现有合成包夹具中的实际三个模块、support records、字节/hash/checksums 和 import/export；不是新 EXE 或 devtest 包 |
| 语法/编码 | 12 个 Node `--check`、1 个 Python `ast.parse` 通过 | 本批源码 UTF-8 无 BOM；未引入 formatter/依赖安装 |
| 官方行数门禁 | checker 31 项、ratchet 4703 files、diff check 通过 | 根 `artifacts/effective-code-lines.json` 直接备份后逐字节恢复，未改 baseline/exception |

最终 Node 两组分别冻结 1197 项 scripts 输入，包合同冻结 1342 项相关输入，运行期间及收尾均无对应输入漂移；口径不同，不冒称覆盖整个仓库。前批 Rust 采集器的 3314 项宽口径输入中，仅本批 Python 包合同及其运行生成的既有 pyc 改变，不能称 3314 项全未变；其中 2968 项 Rust/Cargo 输入无变化，本批不重编 Rust，复用前批生产/测试/格式证据，不把旧二进制当新结果。pyc 作为运行产物单列，保留现场、不改源码迁就它。最终来源和审计以本目录 `audit.json / closeout.json / source-final.json` 为准。

真实隔离 Chromium/CDP 新测试覆盖七个场景：partial→完整逆序全局 feed、无 Content-Length 的 chunked JSON、压缩小但 decoded 超 128 KiB 的 gzip、200 HTML、HTTP 503、坏 JSON、重复提交 ID。实际走 worker 子进程与共享 native reader，不是 mock 的字节准入证明。全部网络目标为回环夹具，全新 profile 位于 linshi，不读取用户浏览器或凭据。

同一个借用 context 的另一页面向**相同 origin/path**发送生成请求，没有串入本次结果；结束后另一页 URL 不变、计时器继续推进、原页未关闭、浏览器仍存活，lease 按次释放。`pageOnly` 仍可能短暂 auto-attach 其他页面再 detach，不宣称没有 Target 事件或绝不暂时暂停。

算法/正常清理夹具仅在显式测试子进程的 `--import` 中替换 capture import，不是 native decoded-byte 证据。原真实 CDP 成功/challenge 测试仍通过。测试 clip 的 `audio_url` 指向假资产，**没有证明音频有效或可播放**。

## 3. 代码规模与审查

本批 13 个源码文件：9 新增、4 修改，均不超过 500 有效行，无新增例外。原 worker 329→194，browser 403→395，生命周期 preload 49→61，Python 包合同 257→270；9 个新文件最大 119，有效行明细见 `effective-lines.json`。

审阅覆盖 ID/status/JSON 信任边界、固定响应来源、静态错误秘密省略、单体/累计/队列准入、一次等待与停止/迟到响应、借用浏览器与 lease 所有权。clip 选择在原 ID≤16、feed≤128 的上限内执行；没有无界集合合并或新重试层。复用 owner 的资源与预算，不维护职责重叠的 reader。

## 4. 必须继续的范围

- 当前模型/feed wire、持久 `suno_device_id`、Clerk 会话/refresh/受众与 challenge 仍需可靠合同及集中录制，不能拿回环夹具证明当前平台协议可用。
- 有效音频读取/播放、精确媒体 probe/逐次费用确认/UI、正常公共媒体结算与 Keep 全链未完成；视频订阅检查保留，不表示视频操作已验。生成与另扣 credits 的下载/转换不得隐式混用。
- 硬取消不能保证远端 EasyBrowser lease 释放；远端 executor 自身进程/lease 回收、过期对账、Unix 实机、独立 Gateway/Server/原生包仍未验证。
- 官方 OAuth/MCP 仍是 discovery-only 分支，客户端注册/tools schema/授权/持久刷新未实现；不创建假模板，不替代 `suno-web`，见 [渠道合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-suno-channel-contract.md)。
- 前批仅 Suno 最小 feature 组合的 Gemini Web 缺符号编译失败继续保留，本批未改 Gemini、未复验或宣布全矩阵通过。

不重复已验元数据细节替代 Luma Web、Udio、FreeBuff 公共业务等原范围；全范围 P50 无凭据集成/devtest、P60 集中录制与 P70 真实逐渠道验收继续，总目标保持 active。本批未读真实 `.ng`/历史凭据、未发实际平台认证/生成/付费请求、未改在线实例或容器、未出包/提交/推送。P34 历史 Redis 授权已经消费清理，不再次复用。
