# FreeBuff 公共 Chat 实时 SSE：原付费 owner 与唯一终态

日期：2026-11-08。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。本批代码仍在继承的 dirty 工作区，不能把 HEAD 当成本批构建来源。

## 本批完成与明确边界

目录 CLI / Local 账号的公共 `/v1/chat/completions` 已支持限定文本／函数工具的真实增量 SSE：正常公共请求先进入既有逐次购买批准，再沿原持久 claim、签名 START/model、费用观测与显式 FINISH 返回。不是缓冲完整原生 SSE 后模拟 token，也没有第二个付费执行器或后台生成任务。

Responses／Messages 的既有非流式 JSON 保留；这两个入口的 `stream=true` 仍在报价前明确拒绝。本批不把 Chat 子集重新定义为整张 FreeBuff 卡完成。17 个新增模板不变，无新增整卡 `offline_ready`；P50 全范围无凭据集成/devtest、P60 集中录制、P70 真实逐渠道验收及总目标均未完成。

完整行为合同见 [公共 Chat SSE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-public-chat-sse.md)。上一内部接缝见 [原 run 逐帧观察](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-observed-stream.md)。

## 改了哪些内容

- `paid::Input` 捕获最小 owned 快照；原 JSON wrapper 与实时 source 共用同一 `execute_observed`。购买确认、动态资格、Grant/revision、原 claim/run、journal 与费用责任不复制。
- `RelayStep` 保留合法 bool `stream`，实际最终签名 bytes 向原模型请求原生流。非流式请求的既有路径继续复用。
- 新 source 由 consumer 同次轮询原付费 future 与容量 1 mailbox，无 `tokio::spawn` producer；observer 背压计入原步骤预算，permit 跨 HTTP handoff 归原 future 持有至终止或 Drop。
- 只有已计入 attempt 的原操作能取得 `AdmittedStream`，绑定 attempt 序号并保留首次停止原因及原 deadline。普通检查、新 attempt 和 handoff 后新能力获取仍被原围栏拒绝；没有续期或新增发送槽。
- provisional 帧保留 native 文本、reasoning/details、工具参数片段与扩展，但隐藏早到的 finish/DONE。真实 EOF、原严格 decoder、最终工具 JSON 与累计序列化预算通过后，先等待原 FINISH/journal 与最后 fence，再交付唯一公共成功终态。
- JSON fallback 明确为一次完整 delta，不伪造逐 token 时序；未知 token usage 保持未知。全部输出帧、escaping、framing 与 DONE 共用累计 64 MiB 上限。
- 提取 `streaming::terminal::track`，让普通 stream 与此 catalog stream 共用原 metrics、健康、usage、archive、audit、扣费／退款终态 owner。catalog 不借旧 lane 的 lease，也不套会先 Drop inner 的通用 deadline wrapper。
- validated-terminal flag 只在 source 实际 yield 最后 bytes 时设置，不因预取或 FINISH 已完成就提前成功。读到 DONE 后关闭可结算一次成功；早 Drop 同步回收原 future/body/mailbox/permit，保留原持久事实，不补 FINISH/end、不重买或重放。

协作 caller/closed/shutdown/deadline 与真正 Drop 不是同一保证。已确认 START 的协作停止仍等待原费用 FINISH（既有最多 20 秒 closeout）；未知费用不填 0，在途 FINISH 不取消后另发。停止 poll 但不 Drop 不承诺主动推进收尾。

## 离线验证与固定来源

本批证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-chat-sse-20261108`。

| 检查 | 最终结果 | 证据／说明 |
| --- | --- | --- |
| FreeBuff 相邻 lib | **497 passed / 0 failed / 4 ignored** | `freebuff-final.log/.json`；包含本批 11 项新增，不与定向、中间或历史数相加 |
| 共享预算 | 12 passed / 0 failed | `budget-final`；含本批 2 项 continuation/handoff/deadline/取消/attempt 绑定测试 |
| 流观察与 tracker | 52 passed / 0 failed | `tracker-final` |
| 管线生命周期 | 6 passed / 0 failed | `lifecycle-final` |
| 既有 Bedrock 公共 TCP | 16 passed / 0 failed | `shared-tcp`；普通流四入口 JSON/SSE/EventStream 的历史、成功与取消退款仍通过 |
| 正常生产 lib | `cargo check` 通过 | `production-check`；`--locked --offline --no-default-features --features runtime-defaults -j 4` |
| 代表性仅关闭 FreeBuff 的生产 lib | `cargo check` 通过 | `production-freebuff-disabled`；精确 feature 列表保存在 receipt，不宣称全 feature matrix |
| 官方 Rustfmt | 26 个范围 Rust 文件通过 | `rustfmt-final`；`--check --config skip_children=true`，无源码写入 |
| Checker tests | 31 passed / 0 failed | `checker-tests`；后续仅三份 Rust 测试变化，不改变 checker 消费输入 |
| Effective-line ratchet | 通过，4,980 文件 | `ratchet`；14 个 classified immutable，governed source >700 为 0 |

同一 lib harness 按完整测试名去重为 **567 passed / 0 failed / 4 ignored**；Bedrock integration 的独立 harness 另为 16 passed。`test-identities.json` 同时核对上一批 486 项通过和 ignored 身份均未丢失。4 项 ignored 为 3 个旧 opt-in 浏览器 fixture 和 1 个 parent-owned child entry；父测试的六阶段真实 kill+wait 仍实际通过。

新增公共测试使用随机回环真实 HTTP/TCP、正常管理批准、原 signed 上游与 SQLite：在上游 EOF 被 gate 阻挡时已经读到文本／reasoning／工具片段；公共 finish/DONE 确实等待原 EOF 与 FINISH ack。坏尾帧、未知费用、FINISH 拒绝、JSON fallback、无 token 数但费用已知，以及 caller/closed/shutdown/deadline/早 Drop/读 DONE 即关均有各自断言。mailbox、预取不当消费、唯一 callback、typed error/fuse 和累计输出预算另有定向测试。工具是 caller 提供的函数历史及转发，不代表实际 host 工具执行。

8 个终态行为／编译／格式检查前后所选 **4,445 个 build inputs** 均逐项同 hash，且与最终源码相同。ratchet 也对应同一输入；checker tests 后仅三份测试源变化，未变化的 checker 不重复执行。两种生产编译均为本批实际运行，仍有既有 unused/dead-code 警告；不是无警告编译，也不是 EXE 或整包验收。

范围 26 个 Rust 文件最大 272 有效行；`streaming.rs` 456→236，提取的单一终态 owner 为 272。policy/baseline/exceptions 未修改；生成报告按本批直接 preimage 原字节恢复。`final-audit.json`、`scope-counts.json`、`scoped-vs-preimage.diff` 记录来源、UTF-8 无 BOM、范围 diff 和保护文件。`harness-processes.json` 记录精确测试／构建进程终态，不清理无关进程。

## 真实失败与修复

1. `public-red` 已成功编译后实际执行失败：公共 `stream=true` 在旧 guard 处结束，未产生购买 pending。保留 `public request ended before producing a pending purchase` 原日志，不能称编译失败或把红测算通过。
2. `live-check` 暴露提取终态 owner 时漏带 `route_policy_config` 的 `E0425`。补入 owned 字段并由两个 sender 传递；最终正常／关线生产编译与公共 TCP 回归已通过。
3. 首次 `public-green` 为 4 passed / 1 failed：工具测试错误期望 25 credits，而既有 SDK 公式对 `0.25 * (1 + 0.055) * 100` round 得 26。只修测试期望并写明原公式，没有改费用语义；最终完整 FreeBuff 集合通过该用例。
4. `budget-beforefix` 为 1 passed / 1 failed：初版夹具的能力获取顺序先碰到既有 `attempt_limit`。修正测试采用原 `run → begin_attempt → admitted_stream` 顺序；不放宽生产预算，新发送／新能力的 handoff 围栏仍有断言，最终预算 12 项通过。
5. 最终测试身份核对最初把带 child 诊断行的 parent 测试漏算；核对原日志中的六阶段与末尾 `ok` 后，只修临时审计解析器。不是业务测试丢失，也不需重跑未变化的测试。

所有失败日志保留，不把中间结果、历史通过数或独立 harness 的同名短标识重复累计。

## 下一步接力

优先以同一个原付费 source 接入 Responses／Messages 的真实增量投影，并保留不等价 native 字段与晚置成功终态；随后验证正常浏览器逐次确认与活 SSE 的联合。不能用整段缓存或新 producer 冒充实时，也不能把异步 FINISH 塞入同步 tracker callback。

整卡仍缺完整参数／模态、实际工具与 child 执行、持久完整 history、START/FINISH 未知结果的明确对账与安全恢复、远端 lease、精确 probe、Server/跨进程/原生包和真实平台。公开合同未证明的查询／幂等／金额 receipt 不编造，持久事实不当恢复执行许可。其余渠道以 [当前全范围状态](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-status.md) 为接力入口。

本批没有前端变更，未重跑前端／浏览器；没有真实凭据／供应商调用、Redis/Docker 操作、现有实例变更、出包、提交或推送。P34 Redis 授权在历史批次已消费并清理，本批不复用；新的高风险清理另需明确同意。总 goal 保持 active，不标记 complete 或自行 paused。
