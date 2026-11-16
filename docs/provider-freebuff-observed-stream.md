# FreeBuff 原 run 的逐帧增量观察合同

后续更新（2026-11-08）：[公共 Chat SSE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-public-chat-sse.md)已沿本接缝接入 owned handoff 与原 FINISH／公共终态。下文“公共 guard 不变／仍待接”的文字是本内部接缝子批的历史边界，不代表当前 Chat 仍关闭；Responses／Messages 实时 SSE 与其余交付卡范围仍待完成。

本合同描述目录业务执行器的内部增量接缝，**不是公共 Chat、Responses、Messages 实时 SSE 已开放的声明**。公共输入 guard 与 `RelayStep` 的非流式准入保持不变，新增模板数也不变。

## 唯一执行责任

`CatalogRun::execute_step_observed` 与既有步骤共用 `execute_built_step_observed`。准备、原 live claim 校验、原 serializer/signature、model pending 持久化、一次模型请求及原步骤预算均不另起 owner；没有新的购买、START、重试或 detached producer。

观察函数只取得一份已解析的原生 JSON frame，不能取得 run、claim、active session 或下一步骤能力。它返回的 future 在原步骤 future 内直接等待；因此支持背压与 fallible 投影，但不创建生产任务或事件队列。既有不观察的步骤走同一实现，使用无操作回调。

## 逐帧顺序

1. 原 body 的真实 chunk 进入既有有界 SSE framer，总字节预算不变。
2. 每次只取一帧，严格解析 JSON 并检查当前 frame 的身份、choice、delta、usage 与工具片段等原有约束。
3. 将费用快照复制回原步骤的 `Accounting`，先等待原 run journal 保存已变化的 reported cost，再传播 frame 错误或调用观察函数。相同快照不会每 token 重写 SQLite。
4. 等待本次观察函数完成，才读取同 chunk 后续 frame；未消费的 bytes 留在 framer 内，不先展开为 frame 列表。
5. 直到真实上游 EOF，才调用原 strict finish，检查残帧、完整 choices、真实 finish 与最终工具 JSON，并返回唯一的 `DecodedCatalogRun`。

完整 frame 校验失败时，若其费用字段已成功观察，仍先保存费用，但不交付该坏 frame。更早的有效 provisional frame 不能把之后的错误变成成功。观察函数自己失败时，也收回原 run 并保留截至此处已观察的费用；不会跳过该错误继续读取、重放或默认修复工具参数。

## provisional 不等于终态

观察函数收到的 `finish_reason`、usage-only frame 或 `[DONE]` 之前的输出都不是整流成功。`[DONE]` 不交付给观察函数；本批没有增加“必须出现 `[DONE]`”的合同，合法的原生完成仍由真实 EOF 与已有 strict finish 决定。

工具参数片段可以尚未构成 JSON；它们按原样交付，最终有效性仍由 EOF owner 判定。native reasoning、reasoning_content、reasoning_details、工具扩展及费用保留在原生 frame 中，不假造 Anthropic thinking/signature 或未知 token usage。JSON 响应仍完整有界收集，不伪造增量 frame。

**公共 SSE 接线必须另行保证**：不能将收到 native finish frame 当作公共成功终态提前输出；成功终态必须晚于真实 EOF、最终投影校验及原显式 FINISH/journal。普通 `TrackedStream` 的同步 callback 不能补发这些异步步骤。

## 取消、期限、Drop 与 FINISH

同一个 pinned cancellation future 贯穿准备、headers、body、journal 等待和观察函数。即使观察函数同步完成，读取同 chunk 下一帧前也再次检查取消。观察函数背压计入原步骤期限，不续期，不增加模型 attempt。

协作取消、步骤超时、原生错误或观察函数错误均返回原 decoded owner；原费用 ledger 只记入一次。host 可对它显式调用原 `finish_run`：已知费用按 completed/failed/cancelled 合同发送，未知仍拒绝 wire，不填 0。FINISH 的 HTTP 接受不等于 session 结算，不释放 claim 或自动 end。

若 SQLite 保存 future 被取消，commit 结果不确定，原 journal 继续锁存 unavailable；观察函数不得跨过这个未确认写入，后续 FINISH 也不得发送。

真正 Drop 同时销毁原执行 future、观察 future 与 body，不创建后台 FINISH/补偿或续跑。已保存的费用与原 run 事实保留；同目录重开不是恢复执行许可。没有 detached 任务也意味着停止 poll 后不能保证主动推进 FINISH；公共 body 接线必须如实区分协作取消与直接 Drop。

## 仍需接线的工作

- 公共请求的 owned SSE future、预取/handoff 与原取消/期限能力；不得放宽新 send 的 handed-off 围栏，也不能直接套会先丢弃 source 的通用 budget wrapper。
- 三协议真正增量输出、原生非等价字段的显式扩展，以及严格晚置的公共成功终态。
- 既有公共审计、usage、配额/退款、健康与 permit 的唯一终态 owner；正常公共 TCP/UI 的端到端验证。
- 真实平台、完整参数/模态/实际工具与 child、未知结果显式对账/安全恢复、远端 lease、Server、原生包及总计划 P50/P60/P70。

本合同不修改认证方式、服务商额度池或账号权限；不代替[公共非流式 JSON 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-public-json.md)。
