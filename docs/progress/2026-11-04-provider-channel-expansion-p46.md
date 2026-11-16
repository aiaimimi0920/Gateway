# P46：Accio 业务响应与精确账号测试

日期：2026-11-04（Asia/Shanghai）。状态：本批实现与定向离线验证通过。R01/W01、其他原定渠道和总目标继续 active；没有新增整卡 `offline_ready`，新增模板记录仍为 12。

## 1. 本批范围

- 显式 Accio prompt/model 与保存的测试计划进入专用 probe；复用精确 target 的普通 `execute_with_provider_account_id`、三适配器准备、原生 generateContent planner/header/sender，不落入标准 Chat JSON sender，不选其他账号、不跨渠道回退或隐式重试。无请求参数不生成；管理身份、Secret Grant、enabled 和模型准入继续沿现有入口。
- Accio 非流式聚合和公共流式调用使用同一个专用 admission decoder。逐字节边界、UTF-8、CRLF/多行 SSE、outer/inner error、完整 JSON 与 EOF 均由明确边界处理；有输出且具有现有可识别 reason/Done 才能成功。usage-only、空/未知/截断、孤立工具参数和重复 active start 不构成成功。
- 正文可增量发送；成功 Finish/DONE 保留到 EOF 校验之后，后到的业务错误、无效尾帧和 transport 错误不能被当作完整成功。普通错误文案固定，不把上游 `error_message` 或任意错误码回显；已知 5015 分类保留。
- 同一事件折叠保留报告的 total 与 cache 字段，不再以 prompt+completion 重造报告 total；usage 数字溢出不 panic。probe 只接受完整、非空、stop、无工具的文本答案，回复限制 8192 字符；选定材料先脱敏再进入 audit/report/assessment。
- 不重写共享 Anthropic decoder 的终态规则；共享 renderer 仅复用状态/用量渲染，并修正已发工具参数在 Finish 重放及完整工具的输出 index。独立复核发现的完整/分片混合与源 index 重用碰撞也已修正：每次 start/完整工具共用单调输出序号，pending 保存源 index 到输出 index 的映射。再次只读复核确认原路径封闭，不把源码复核代替运行证据。

## 2. 资源和协议证据边界

单帧 256 KiB、同响应总 wire 64 MiB（沿用原聚合上限）、最多 128 次工具创建和累计 1 MiB 工具参数；stream 以 4096 字节批次推进并在消费前排空输出队列，不新建后台生成任务。取消由原 HTTP/stream owner 的 drop 释放；probe 复用现有 60 秒预算和 cancellation-safe audit/health owner。

现有固定开源参考仅覆盖 OAuth 的 PKCE/exchange/refresh/expiry，没有固定 Accio generateContent 终态参考。因此本批完成条件是**基于现有可识别事件的本地合成合同收紧**，不是已联网核实的官方实时 wire。没有把 outer `turn_complete` 擅自当成功，也没有要求 Accio 必须提供 AWS metadata。旧二进制 envelope 提取兼容仍保留并测试分块/截断；它没有 CRC 验证，不冒称真正 AWS EventStream 或 Accio 上游能力。

## 3. 红测与实现过程

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p46`。

1. `baseline.json`、`before/` 保存本批既有输入原始字节/SHA256/官方 lexer 有效行。最初编译在进入测试前取消，以便显式将 TEMP/TMP 放到本批 linshi；没有以取消记录当红测。重新编译的 `red-contract-v2.log` 用时 17m19s，实际公共 HTTP 测试 3 passed / 6 failed。
2. 公共失败分别复现：HTTP 200 空成功、buffered 把报告 total=9 重算为 7（Chat/Responses）、中途错误只留下 partial 文本、工具参数被重复拼接、两个完整工具共用 index=0。不是缺模块或编译失败冒充红测。
3. Cargo 在首个失败 test binary 后停止；另直接运行本次刚编译的精确 probe test binary，`red-probe.log` 为 1 passed / 4 failed：显式账号/矩阵/错误回复测试均在实际入口返回 unsupported。`red-source-verification.json` 核对八项既有 Rust 输入均与 before 相同。
4. 只读独立复核发现孤立 ToolDelta 可被当 output_seen、随后被 Collector 丢弃，形成空成功；已在 Accio admission 以有界 active index 拒绝，并加非流式/流式回归。混合工具回归在 `green-contract.log` 实际得到 `[0,0,0]` 而非 `[0,1,2]`；Gemini 兼容分支的 `STOP` 未规范化也实际失败，均已修补并转绿。
5. 首次 green 中公共流式负向夹具对 body collect 无条件 unwrap，错误现在正确传播反而使夹具 panic；仅改为收集正文和显式 body error，并继续要求没有成功终帧。probe 另实际触发 Windows 默认测试线程栈溢出；同一已编译 binary 设置 8 MiB 栈后为 4 passed / 1 failed，精确暴露 SQLite audit 缺 cache 字段。生产修补保留报告 cache 字段，不改变生产线程预算。
6. 新夹具初版将已构造 future 移入独立线程，`green-contract-v2.log` 暴露 SQLite future 的 `Executor/Send is not general enough` 编译错误，不能算行为红测。最终只移动 future factory，在其 owner 线程构造并由 current-thread runtime 驱动，线程栈对齐现有 main 的 8 MiB 默认值；没有给生产 SQLite 增加 Send 要求或修改生产语义。`green-contract-v3.log` 的 9+7+5 项全部通过。

## 4. 验证状态

最终共 **236 个不同 Rust 测试通过，其中本批新增 21 项**。`verification-summary.json` 按实际 test binary/module + 测试名去重，不把多轮 green/red 重复相加。

| 检查 | 实际结果与证据 |
| --- | --- |
| 新增公共 HTTP / admission / 精确 probe | `green-contract-v3.log`：9 + 7 + 5 passed，Cargo exit 0 |
| 相邻生产 HTTP / 管理 / revision | `regression-http.log`：22 passed；包含 Cohere、Bedrock pipeline/probe、Accio OAuth/secret/revision |
| Accio 相关库测试 | `regression-accio.log`：110 passed |
| 共享 Anthropic 回归 | `regression-anthropic.log`：59 passed；不将 Accio 终态收紧应用于共享 decoder |
| 模型测试与 stream error | `regression-model-probes.log`：20 passed；`regression-stream-error.log`：4 passed |
| 代表性 feature-off 编译 | `feature-exits.json` / `feature-disabled.log`：runtime-defaults 仅去掉 Accio line，`cargo check --lib` exit 0；不是所有 feature-off 组合通过 |
| checker / ratchet | `checker.log`：31 passed；`ratchet-v2.log`：4234 files，14 immutable runtime artifacts，governed source >700=0，exit 0 |
| formatter / diff | 定向 `rustfmt --edition 2021 --config skip_children=true --check`、`git diff --check` 均 exit 0；本批新增/既有文本另做 UTF-8/BOM/whitespace/有效行和 scoped diff 审计 |
| 来源冻结 | `source-frozen-v3.json` 与 `verification-summary.json`：16 项 Rust 输入在最终测试后逐项 SHA256 相同；文档补齐后冻结到 `source-frozen-final.json` / `audit-final.log` |

最初 ratchet 调用误用绝对 `--json` 路径被工具拒绝（exit 2），之后改用官方 `npm run check:effective-lines --prefix scripts` 通过，不改 checker 约束或基线。所有既有失败/中止日志保留，不以最终绿灯抹去过程。相邻回归的库测试首次编译 10m53s，核心 green v3 编译 1m45s；不把不同缓存状态误作性能倍数。

本批 8 项既有 Rust 输入中，`line.rs` 384→288、共享 renderer 338→358、disabled facade 301→321、stream_preflight 276→281、provider_runtime 84→85、console_model_probe 369→383、model_probe_recording 154→160、stream_error 69→70；8 项新增 Rust 输入最大 250 有效行。没有新 501+ 例外、baseline 重写或 oversized debt 增长。

测试入口为真实 `build_router` 的 `/v1/chat/completions`、`/v1/responses` 和正常 Console provider probe，全部使用随机 loopback 和合成材料。精确账号/原生路径/token/device/prompt/model、buffered/SSE、工具/用量、错误/截断、公共 stream body drop 释放上游和本地 SQLite audit/矩阵均分别留证，不用 planner 单测替代公共 HTTP 验收。没有新增 Accio 专属 buffered/probe 取消 audit 的全链测试；复用已有 cancellation-safe owner 不自动升级为这些未执行场景的证明。

## 5. 安全与剩余工作

没有读取 `.ng` 或历史凭据、调用真实平台、消费付费额度、创建/删除 Redis 容器、出包、替换运行实例、提交或推送。既有多处并行改动未回滚或接管；S06 Rust/Gemini lane 未进入。

本批不证明真实第三方 client 获准使用、真实业务终态/模型/额度关系、旧导入副本归一化或集中凭据验收；Accio 专属 buffered/probe 取消 audit 全链也仍待补。Kiro 的下一批公开协议依据另见 [Builder ID / IdC 调查合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-kiro-oauth-contract.md)，只完成联网调查，不算 OAuth 实现。Accio 其他未闭合项、Kiro/Vertex/Qwen 未决 OAuth、其余原定 API/Web/媒体能力以及 P50/P60/P70 继续，不删减原始范围，不把 12 个模板记录算 12 个完整渠道。
