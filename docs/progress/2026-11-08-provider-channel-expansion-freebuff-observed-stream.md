# FreeBuff 原 run 逐帧增量观察接缝

日期：2026-11-08。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`，实现仍在继承的 dirty 工作区，不把 HEAD 当新代码的交付凭据。

## 本批交付与边界

本批完成原目录业务执行器的可背压、可失败、可协作取消的逐帧观察接缝，**尚未开放公共 Chat/Responses/Messages 的实时 SSE**。原公共非流式 guard 和 `RelayStep` 原字节受保护；17 个新增模板、无新增整卡 `offline_ready`、P50/P60/P70 与总目标未完成的口径不变。

实现与明确合同见 [逐帧观察合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-observed-stream.md)。

- `StreamDecoder` 将 feed 与 next_frame 分开：每次只交付一个严格校验的 native JSON frame，不按网络 chunk 预建事件数组，不提前读同 chunk 的后续费用。
- `CatalogRun::execute_step_observed` 与原步骤共用准备、claim/live、serializer/signature、model pending、唯一发送及单步期限 owner。原普通/relay/root 步骤同样走这条核心路径，只使用无操作 observer。
- 原 body 的逐帧读取先更新 Accounting、等待原 journal 保存 changed reported cost，再传播该 frame 的错误或调用 observer；费用合法但其余 frame 无效时，费用先保留，坏帧不对外观察。
- observer 的 future 在原执行 future 内直接等待。背压计入原期限；错误、取消与超时收回同一个 decoded run，原 ledger 只计一步，不重买/重放/自动 end。明确 FINISH 仍由原 owner 单独等待，未知费用不补 0，HTTP 接受仍不是结算 receipt。
- finish_reason/DONE 不当整流成功。真实 EOF 后仍经过原 strict finish、完整工具 JSON 和 context anchor；JSON 不伪造 token 流。native reasoning/details、工具片段与扩展不转成伪 thinking。
- 每个 frame 前与 journal/observer 等待处复核同一取消 future，同 chunk 中立即返回的 observer 也不能越过取消。SQLite 写入被取消时保留原 unavailable 围栏，不交付下一 frame，也不发送 FINISH。
- 真正 Drop 同步销毁原 future、body 与 observer，没有后台生成/FINISH。已持久费用、原 claim/run 事实保留，同目录重开不恢复发送权限。

## 验证

本批证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-observed-sse-20261108`。

新增 13 项定向测试已通过（`observed-green.log/.json`），包括：真实随机回环 HTTP、原 signed START/model/FINISH 与 SQLite；上游 EOF 被显式 gate 阻挡时已经收到两帧及对应持久费用；同 chunk 不预读后续 cost；UTF-8 按字节分片/原生工具参数；坏 frame、observer 投影失败和未知费用；背压时取消/期限、立即回调取消、写入等待期间取消，以及 Drop/重开无后台补偿。

10 个 Rust 文件官方格式通过；checker tests 31/31，ratchet 4,970 文件通过（14 个 classified immutable，governed source >700 为 0）。本批最大 233 有效行；5 个新 Rust 文件最大 215。policy/baseline/exceptions 未修改，生成到根目录的 effective-lines 报告按本批直接 preimage 原字节恢复。

最终 FreeBuff 相邻测试 **486 passed / 0 failed / 4 ignored**，含本批 13 项新增，不与定向或历史通过数相加。4 项 ignored 为 3 个旧 opt-in 浏览器 fixture 和 1 个 parent-owned 进程 child entry；父测试内六阶段强制退出仍实际通过。完整测试名与上一批 473 项通过集合核对，旧身份无丢失。

正常 `runtime-defaults` 与代表性仅关闭 FreeBuff 的生产 lib 均实际 `cargo check` 通过；不是复用上一批编译结果，仍有已有 dead-code/unused 警告。最终 7 个 gate 的前后所选 4,435 个 build inputs 均逐项同 hash，且与最终源码一致；范围 diff、UTF-8 无 BOM、保护文件原字节及精确 harness 无活进程由 `final-audit.json` 核对。未执行关闭配置单测、全 feature matrix 或全仓 all-target 回归。

### 真实失败与修复

- `observed-red` 是已编译后运行的行为红测，不是“新 API 不存在”的编译失败。初始无操作 observer 在真实 EOF gate 前不能交付 frame，测试以 `Err(Timeout)` 失败。
- 初次实现的 13 项定向结果为 12 passed / 1 failed。失败点是故意取消 SQLite 写入后的 Windows 测试目录清理：`Cannot remove isolated local runtime test root: ... (os error 32)`；业务/不可 FINISH 断言已通过。精确同一测试重现了清理失败。
- 只在该测试显式 drop 已关闭 runtime，并以独占文件访问探测等待真实 SQLite 句柄释放（最多 12 秒，覆盖既有 10 秒 busy wait 与收尾）；没有固定 sleep 冒充释放，没有修改生产 busy_timeout、全局删除容忍或费用语义。随后 13 项全过。失败日志保留，不将中间通过数叠加到最终统计。

## 后续接力

下一个完整纵切是公共 owned SSE future、预取/handoff 与三协议增量投影，再接既有唯一审计/usage/配额/退款/健康/permit 终态。公共成功终态必须晚于真实 EOF、最终投影及原显式 FINISH/journal；不能用 `TrackedStream` 同步 callback 补 FINISH，也不能直接套超时先丢弃 owner 的通用 budget stream，或放宽新 send 的 handed-off 围栏。

完整参数/模态、实际工具与 child、START/FINISH 未知结果显式对账/安全恢复、远端 lease、Server、独立进程/原生包及真实平台验收继续；本批内部接缝不是这些能力的完成证据。

无前端源码变化；没有重跑前端/浏览器或打包，没有使用真实凭据/供应商、Redis/Docker、现有实例，也没有提交/推送。历史 Redis P34 授权已消费，本批未复用。
