# FreeBuff 公共 Responses／Messages 实时 SSE：共用原付费执行与晚置终态

日期：2026-11-08。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承的 dirty/untracked 工作区保留，HEAD 不是本批源码或构建凭据。

## 本批完成与范围

目录 CLI / Local 账号的 `/v1/responses`、`/v1/messages` 已支持既有文本／函数工具闭集的真实增量 SSE，与既有 Chat 共用原逐次批准、持久 claim/run/journal、签名模型发送、费用 FINISH、取消及唯一公共结算链。不是缓存整段上游响应后再模拟 token，也没有第二个付费执行器、后台 producer 或隐式重放。

这是两个公共协议的限定离线子链完成，不是 FreeBuff 整卡、完整 agent harness 或真实平台验收。17 个新增模板不变，无新增整卡 `offline_ready`；P50 全范围无凭据集成/devtest、P60 集中录制、P70 真实逐渠道验收和总目标仍未完成。

行为合同见[公共 Responses／Messages SSE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-public-protocol-sse.md)，原 source／预算／取消合同见[公共 Chat SSE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-public-chat-sse.md)。

## 改了哪些内容

- 原始 `stream` 仍须合法 bool；Responses／Messages 转换器将它保留到原 `RelayStep` 最终签名 bytes。未知字段、未支持模态／内建工具／continuation 仍在报价前拒绝，不在 normalize 时静默丢弃。
- 新投影按 endpoint 选择既有 Chat 或 Responses／Messages envelope，后者拆为公共状态、frame、Responses、Messages 和测试。全部复用原 `paid::execute_observed`、容量 1 mailbox、consumer 同次轮询、permit、attempt continuation、原 deadline 和 closeout。
- Responses 交付 item/text/function-call-arguments 增量；Messages 交付 indexed text/tool_use block 增量。公共 response/message ID 稳定，工具原 call ID 不伪造；首见公共顺序不被晚到的 native 稀疏 index 重排。
- native reasoning/details/usage/opaque 数据通过 `gateway_freebuff.delta` 显式事件保留，最终 `gateway_freebuff` 使用原严格 JSON converter。不伪造 Anthropic thinking/signature、未知 token 数或工具实际执行。
- strict decoder 和公共投影共用工具 index/ID/name 关联规则，支持 ID 迟到、缺 index、稀疏 index。迟到 ID 前只保留有界未交付参数；最终工具 JSON 与 Messages 对象输入仍严格校验。
- 所有帧、escaping、framing、扩展和晚置终态共用累计 64 MiB 输出预算。真实 EOF、严格 native decoder、最终协议投影及预算先完成，再等待原 FINISH/journal 和动态 fence，最后才交公共成功终态。JSON fallback 明确为完整有界读取后的单次投影，不冒充逐 token 时序。
- 审阅修正工具扩展与首见公共工具顺序的对应关系；原已知 native completion semantics 与 usage 一起传给唯一 `streaming::terminal::track`，避免从有损的 Responses `incomplete` 丢失 `length`／`content_filter`。普通 stream 传 `native_semantics=None`，原观察器和预算行为不变。
- source 仅在实际 yield 验证后的最后 bytes 时设置 terminal flag。协作 caller/closed/shutdown/deadline、早 Drop 和读到终态后 Drop 分别验证；不后台补 FINISH/end、重买或恢复执行。

## 最终离线验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-protocol-sse-20261108`。

| 检查 | 最终结果 | 证据／说明 |
| --- | --- | --- |
| FreeBuff 相邻 lib | **508 passed / 0 failed / 4 ignored** | `freebuff-final`；含本批 11 项新增，不与中间或历史结果相加 |
| 共享请求预算 | 12 passed / 0 failed | `budget-final` |
| 流观察／tracker | 52 passed / 0 failed | `tracker-final` |
| 管线生命周期 | 6 passed / 0 failed | `lifecycle-final` |
| 既有 Bedrock 公共 TCP | 16 passed / 0 failed | `shared-tcp`；普通流四入口的历史／成功／取消退款回归 |
| 正常生产 lib | `cargo check` 通过 | `production-reviewed`；`--locked --offline --no-default-features --features runtime-defaults -j 4` |
| 代表性仅关闭 FreeBuff 的生产 lib | `cargo check` 通过 | `production-freebuff-disabled`；精确 feature 列表见 receipt，不冒称全 feature matrix |
| 官方 Rustfmt | 25 个范围 Rust 文件通过 | `rustfmt-final`；`--check --config skip_children=true`，无源码写入 |
| Checker tests | 31 passed / 0 failed | `checker-tests`；在审阅修正前执行，不冒称对应最终 4,455 项源码全量同 hash |
| Effective-line ratchet | 通过，4,990 文件 | `ratchet-final`；14 个 immutable，governed source >700 为 0 |

同一 lib harness 按完整测试名去重为 **578 passed / 0 failed / 4 ignored**；独立 Bedrock integration harness 另为 16 passed。`test-identities.json` 核对上一批 Chat 的全部通过／ignored 身份均保留，本批新增恰为 7 个公共协议测试和 4 个投影测试。4 个 ignored 是 3 个旧 opt-in 浏览器 fixture 与 1 个 parent-owned child entry；父测试仍实际执行六阶段 child kill+wait。

新增公共测试使用随机回环真实 HTTP/TCP、正常管理批准、原 signed 上游夹具及 SQLite：阻挡上游 EOF 时，caller 已读到文本／原生扩展／函数参数；成功终态确实等待 EOF 与原 FINISH ack。坏尾帧、未知费用、FINISH 拒绝、JSON fallback／cost-only unknown usage、不可表达的 Messages 工具输入、两协议共 12 个取消／Drop 分支、原始 length/content_filter 的公共审计均有断言。caller 函数工具只是历史／转发，不等于 host 工具执行。

8 个终态行为／生产编译／格式检查前后所选 **4,455 个 build inputs** 均逐项同 hash，且与最终源码一致。最终 ratchet 也对应同一输入；checker tests 之后有 9 个列明的 Rust 输入变化（包括 1 个新增文件），不把其旧 4,454 项快照说成最终源码凭据。源码变化列表、stage 命令／时间／退出码均在 `final-audit.json` 和各 receipt 中。生产编译仍有既有 unused/dead-code 警告，不是无警告编译、EXE 或整包验收。

25 个范围 Rust 文件最大 276 有效行，新文件最大 209；既有共享终态 owner 为 272→276，普通 streaming 为 236→237，均保持单一职责。`scope-counts.json` 记录逐文件前后值。policy/baseline/exceptions 与生成报告按本批直接 preimage 原字节保护，不用 Git HEAD 代替。`final-audit.json`、`scoped-vs-preimage.diff` 记录来源、UTF-8 无 BOM、范围与 diff 检查；`harness-processes.json` 记录精确 harness／构建进程终态，不清理无关进程。

## 红测、审阅修正与证据边界

1. `protocol-red` 成功编译后实际 **0 passed / 1 failed**：旧公共 guard 让请求在 pending purchase 之前结束，日志原文为 `public request ended before producing a pending purchase`。前后 4,446 项输入一致。这是真实行为红测，不是编译失败。
2. `protocol-green` 的两协议 6 个公共 HTTP 测试曾全部通过，但随后工具扩展顺序与原 native completion semantics 接线发生实质修正，不能拿中间绿色代替最终结果。
3. 新增投影断言覆盖 native index 顺序与首见公共顺序不同的工具扩展；新增公共测试覆盖两协议的 `length`／`content_filter`。这两项修正现已包含于最终 FreeBuff 508 项及两生产编译／普通流 TCP 回归。
4. `production-initial`、旧 ratchet 与中间绿色均只保留为历史；本批实际采用 `production-reviewed`、`ratchet-final` 和最终回归证据。所有失败／中间日志保留，不叠加测试数或伪装最终来源。

## 接力顺序与仍未完成

下一步优先验证**正常浏览器逐次购买确认 → 活公共 SSE → 原 FINISH/journal → Local 结算 → 显式原 claim end／再购买**的联合，不用只调用管理 API 的公共夹具代替正常 UI；继续复用唯一执行与结算 owner。

完整参数／模态、真实 host 工具与 child、持久完整 history、未知 START/FINISH 的明确对账与安全执行恢复、远端 lease、精确 probe、Server、跨进程／原生包、真实平台及其他渠道仍须继续。公开合同未证明的查询、幂等与金额 receipt 不编造，持久事实不当恢复执行许可，FINISH 不自动 end。

本批没有前端变更，未重跑前端／浏览器，也没有真实凭据／供应商调用、Redis/Docker 操作、现有实例变更、出包、提交或推送。历史 P34 Redis 授权已消费并清理，本批不复用；新的高风险清理须重新取得明确同意。总 goal 保持 active，不标 complete 或自行 paused。
