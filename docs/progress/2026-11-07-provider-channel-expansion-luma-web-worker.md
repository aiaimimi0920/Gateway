# Luma Web：worker 生命周期、总时限与禁止重放

日期：2026-11-07（Asia/Shanghai）。仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；这是继承 dirty tree 上的源码子批，
不是该 HEAD 的发布或整卡验收。总目标保持执行中，累计新增模板仍 16，无新增整卡 `offline_ready`。

## 1. 本轮实际推进与范围

上一完整目标轮完成了 ChatAIBot 精确图片 probe/UI，属于实际 progress。本轮继续 R14 Web，
先读真实 sender、worker、协议/材料、共享 supervisor 与测试，再做匿名官方公开调查和定向红测。
新发现的官方 OAuth/MCP 作为必要同品牌分支保留，见
[Luma 渠道合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-luma-channel-contract.md)。
同 team MCP/App credits 及相同费率有官方证据；不证明 REST Key 共享、不先建空模板。

当前公开材料不足以冻结登录后的 board wire、模型 action、完整 Cookie/JWT 组合与刷新。
本轮不猜新 wire、不用 Agents REST 成功替代 Web；先修直接确认的执行生命周期和费用重放风险。

## 2. 改了哪些内容

### 2.1 Rust 进程与媒体 owner

- `src/upstream/lumalabs/browser_worker.rs` 为 Web 独立结果/错误 owner，复用既有
  `execute_browser_worker_process_with_limits`，不另建进程管理框架或修改 S06。
- 原 supervisor 提供 16 个共享 slots、Windows JobObject/进程树、并发写 stdin 与读双输出、
  取消信号、kill/reap 及超出同步清理窗口后的持有资源后台 owner。
- Luma 输入 1MiB，stdout 128KiB、stderr16KiB；非零退出即使带成功 JSON 也拒绝；
  stdout 严格 UTF-8，空/坏 JSON 不回显正文，OS/pipe/共享 Producer 错误投影为 Luma 身份。
- 本地总时限包括等待 slot、写 stdin、进程运行；caller Drop 仍由 supervisor 持有和回收树。
  媒体 owner 的共享异步 deadline 覆盖远程/本地分派与最终取图，不为后续阶段重置预算。
- 远程一经分派，HTTP/传输/解码/业务不确定结果均不转本地生成，307/308 不重投；
  后续解析或下载失败也 `retryable=false`、`FallbackHint::Abort`，禁止框架换账号生成。
  没有配置远端时，原显式本地策略仍可用。
- 远端 Luma Web metadata 复用现有解码后有界 reader，限 128KiB；其他 provider 默认不改，
  Udio 既有 128KiB 保留。本轮未改现有宽泛错误分类器的普通 rate-limit 可重试语义。

### 2.2 JS 正常清理与边界

- CLI 不再 `process.exit()` 跳过异步 `finally`；先等待 owned browser close，再 await 输出。
  close 失败不报生成成功，模型化业务失败输出 JSON/exit0，非零 OS exit 由 Rust 视为执行失败。
- launch、导航、operation 使用同一剩余预算；operation 不再另拿完整导航前预算。
  worker timeout 上限调整为30分钟，不把既有600秒视频预算硬截为300秒。
- 自身 deadline 和实际 Playwright `errors.TimeoutError` 均映射504，不回显导航或清理原始异常。
- 导航后按预期 board 的精确 origin/path 判断，不再用 `includes` 接受其他站点同路径或
  登录页 query 中的 board 子串；拒绝时不执行生成、不回显完整最终 URL。
- stdin 按实际 bytes 限1MiB并严格UTF-8解码；该限制不等于浏览器内部全部 fetch/heap/wire 的总上限。

## 3. 真实红绿与验证

证据目录：
[gateway-luma-web-worker-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-worker-20261107)。
含直接 preimage、来源快照、每阶段 command/exit/时间/log、`effective-lines.json`、
`scoped.diff`、去重测试名和最终 `audit.json`/`closeout.json`。

### 3.1 红测不是编译失败

Rust 先把原启动段原样提取为可显式传 launch 的 seam，尚未改变资源/退出/重放行为；
`luma_web_red` 真实运行 **0 passed /5 failed**：远端503变为本地fallback、非零exit仍成功、
超预算metadata仍成功，以及业务/解析错误仍可重试。之后才换入共享 supervisor。

Node 首轮生命周期 **0/4**，实际缺少close或仍接受cleanup后成功；随后精确board的子串绕过
红测0/1；只读复核提出的 Playwright真实TimeoutError类型分类再红测0/1，均在生产修改后转绿。
未将红测或名称筛选排除项计为通过。

### 3.2 最终通过的范围

Rust 均使用 `--locked --offline --lib --no-default-features --features runtime-defaults -j 2`，
避免 `management-ui` 的发布/清理hook，并依据本机可用内存限定两个Cargo任务。

| 定向筛选 | passed | ignored |
| --- | ---: | ---: |
| `luma`（含Web、Agents与helpers，不称纯Web134） | 134 | 1 |
| `browser_executor` | 83 | 4 |
| `producer_browser_worker` | 30 | 0 |
| `retry`（宽名称筛选，不称retry模块115） | 115 | 1 |
| `udio_executor` | 9 | 0 |
| `suno::tests::worker`（含protocol worker contracts） | 20 | 0 |

按完整测试名去重 **376 passed /6 ignored**，其中14项本批新增。
6项未运行分别为4项既有Redis lease、1项folder retry并发和1项Agents正常浏览器host opt-in，
不当作通过、不创建新Redis。实际Node子孙的timeout/caller Drop后TCP probe关闭已验，
不是当前平台生成、OS断电或错误返回前同步完成一切后台清理的证明。

Node最终四文件 **35/35**：旧Luma browser operation7、新Luma lifecycle9、Producer12、Suno7；
9项新增。CLI走真实Node subprocess，但Playwright对象为显式合成preload，不是实际Chromium/当前平台。

正常生产lib、仅关闭Luma Web的代表性生产lib、全仓Rustfmt检查、checker **31/31**、
ratchet **4744**项通过；14个immutable runtime artifacts另列，governed source >700为0。
本轮11个受审源码均≤250有效行，最大223；主sender142→125，两个共享文件168→168和223→223。
没有改baseline/新增例外，没有为改进测试而放宽生产限制。

### 3.3 来源与保存边界

Rust red/green的完整4726输入快照在构建期间有JS更改，回执保留`source_unchanged=false`；
变化不涉及Rust消费子集或Node进程夹具。最终Rust消费子集与green快照逐项核对；
后续相邻/生产/关闭/Node最终/格式/checker阶段的4726项所选输入均未变化。
该通用选集不覆盖所有Python/PowerShell、工具链二进制或外部动态依赖，不称全仓冻结。
根effective-lines报告保存本轮结果后，按直接preimage逐字节恢复，不抹去继承现场。
UTF-8无BOM、scoped新增行空白、本地文档引用、独立仓库`git diff --check`结果见最终audit。

## 4. 仍未完成与接力

1. 当前board `actions`/events wire、全部状态、模型/参数、会话/JWT/refresh/challenge、
   原任务ID归属与完成标准、真实费用/额度。旧默认和`finalizing`并不自动成为当前完成合同。
2. local/remote signed URL有效性、固定资产受众、无凭据有界读取、完整图像/音视频验证；
   现有URL/metadata成功仍不能算有效媒体或用于恢复精确模型健康。本轮没有宣称修完旧下载缺口。
3. 正常公共pipeline/Local结算、精确媒体probe/逐次费用UI/完整Keep、独立TCP与进程、Server/PG、原生包。
4. 远端executor硬取消/lease恢复、原任务查询/恢复、跨请求exactly-once。停止本地等待不保证
   平台取消或退还credits；单独CLI的close挂起由Rust生产owner兜底，本轮没有独立CLI硬超时证明。
5. 媒体owner跨远程生成与取图共享总预算的多阶段定时合同本轮未作独立实测，
   不以本地worker超时测试替代；当前总deadline的接线与静态复核已确认。
6. Luma OAuth/MCP按新合同继续client身份/工具/任务/资产、授权保存/刷新；
   FreeBuff、Suno/Udio、Kiro OAuth与Qwen OAuth等其余未完项、P50/P60/P70全部保留。

未读取历史凭据/真实`.ng`，未登录、注册client、真实生成或付费；没有新容器、真实浏览器、
在线实例/数据库变更、出包、提交或推送。P34授权早已消费并清理，本轮没有复用。
