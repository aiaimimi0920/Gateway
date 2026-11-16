# Full Vertex 工具、历史与签名往返子批

日期：2026-11-05。对应 [总计划 R11/W07](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。

本批接续 [文本 pipeline 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-pipeline.md)，
完成 Full Vertex 的工具/history/签名及现有 Preview 构造子集接入；不是完整 decoder、R11/W07、P50 或总目标完成，不增加整卡 `offline_ready`。

## 1. 改了哪些内容

- [产品合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_product.rs) 显式区分 Express 与 Full Vertex 的 label/profile/error prefix/turn source；凭据、目标和额度仍归原产品 owner。
- 将原 Express 的 [响应投影](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/projection.rs)、[Preview assembler](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/function_stream.rs)、[Part 捕获](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/turn.rs) 和 [v1 历史处理](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_history.rs) 提取为共同实现，不复制两套状态机。两个响应入口分别绑定自己的合同；旧 generic Gemini/Vertex 不自动转换，不改 S06 Gemini 内部。
- Full Vertex 完整 functionCall 保留同名同参数的多次调用，生成 `call_vertex_oauth_...` 公共 ID；无参数调用保留空对象语义，不修复坏参数。后续结果按公共 ID 关联、重排并合并为一个 user Content；固定 v1 不发送 v1beta1 call/result ID。
- `gateway_vertex_turn.source=vertex_user_oauth` 保留原 text/完整 functionCall 的 Part 顺序、空 text、thought/signature 和公共工具 ID。`gateway_vertex_history` 验证模型、可见文本、工具 ID/name/args 后恢复原 Parts；两个产品均核对 source，不能因枚举认识另一 source 就接受它。
- 旧全局 Express-only gate 只扩展到 typed Full Vertex，其他产品仍拒绝该历史扩展。Raw 状态绕过和有效 contents 覆盖继续拒绝；Responses 保留原未知字段白名单，忽略的 contents 实测不能覆盖 typed 历史。
- Full Vertex 的显式 `streamFunctionCallArguments` boolean 开关保留 canonical tool choice。复用顺序调用、嵌套/连续数组/转义路径及两层 continuation 的 Preview 构造子集，不全局开启。
- 过滤终态丢弃此前文本/工具/签名但保留报告 usage；坏状态/参数和未支持 Part 不制造成功、不重放未知消费。接入共同投影时保留 Full Vertex 原未支持 Part 拒绝边界，Express 也不再静默忽略不受支持的 Part。

不解释或认证签名的加密内容，不推断账号/project/Key 绑定或 TTL。客户端须显式保留扩展，缺失必要签名可能令 Google 400 拒绝后续工具请求。

## 2. 公开合同与测试身份

本批重新联网只读核实 [thought signatures](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/thinking/thought-signatures)、[Content REST](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/Shared.Types/Content) 和 [function calling](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/tools/function-calling)。
三页完整 HTTP-200 HTML、URL/hash/抓取时间在证据目录：v1 并行结果需一起按原序回传，签名留在原 Part、不合并，流式尾部空 text 不可丢弃；Preview 仍有模型和表示范围限制。
不从通用 Content 的 ID 字段外推固定 v1 的跨模型 call-ID 支持，不猜签名与 partialArgs 的关联。

测试沿 Full Vertex 自己的生产 Local bootstrap、正常管理创建/充值 Key、typed OAuth context、公共 router oneshot 和真实 SQLite fixture。
只复用公共协议包络 helper 和非转发 loopback peer，不复制 Express fixture 后改标签。每轮检查完整 project/location/publisher/model URL、精确账号、Bearer/quota 和 caller Cookie/Key 隔离。
这是合成上游的本地业务子链，不是本批新跑 TCP/UI、原生 EXE/OS 重启或真实 Google。

## 3. 失败与修复记录

- `red-tools`：2995 项输入无漂移，默认栈 **7 passed / 4 failed**。完整工具和签名响应被文本 gate 拒绝，未知工具结果实际进入上游；signed 历史仍由旧 Express-only gate 拒绝，错误归属不符合 Full Vertex 合同。
- 第一次终端调用用逗号拼接 PowerShell 参数，ParserError 且未启动 Cargo；改为原生参数数组，不改生产语义。
- `green-vertex`：静态复核发现旧全局 gate 尚未放行 typed Full Vertex。核对 PID/父进程/command line 后仅停止本批自己的编译器，保存 `green-vertex-cancelled.json`；尚未跑测试，不算通过或产品测试失败。修补接缝后用新阶段重建；取消阶段 2998 输入无漂移，没有停止在线服务。
- 公开检索误用 `fuzzy_pattern` 被工具拒绝，改为 `pattern` 后成功；不是上游产品失败。首次文档 patch 锚点不匹配，整批未写入，修正后重做，不改代码或测试。

## 4. 最终验证

| 阶段 | 实际结果 |
| --- | --- |
| 默认栈 `vertex_`，含两个产品与 source 双向隔离 | 195 passed / 0 failed / 2 ignored |
| OpenAI / Responses / Anthropic | 63 / 57 / 61 passed |
| Gemini API | 16 passed |
| 非测试库 `runtime-defaults` 编译 | exit 0 |
| 代表性关闭，仅 `line-gemini-web-reverse` 编译 | exit 0，不是完整矩阵 |

按完整测试名去重 **390 passed / 0 failed / 2 ignored，11 新增**；两个 ignored 是既有显式浏览器 host，不算通过，也未重跑。不叠加前批 524 项充当本批新增成果。
最终阶段固定 `--locked --no-default-features`、Cargo jobs 4、清除 `RUST_MIN_STACK`；关闭 management-ui 以避开 Web 构建/清理 hook。
七个最终阶段 **2998 项输入**逐项相同，阶段内/阶段间/汇总时无漂移，见 [verification-summary.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-tools-20261105/verification-summary.json)。

新增真实子链覆盖三公共协议 JSON/聚合 SSE 完整工具和有序签名两轮，正常消息余额 **2→1→0**；原 total/unknown/Token Key 回归也在 Vertex 组中通过。
同名调用 ID 不同、反序结果按原序恢复、签名/空 text/字面 HTML entity 参数完整往返；坏历史网络前 400/上游零调用/余额不变，坏上游状态 502/一次发送/退款；过滤保留 usage 但不暴露可执行工具或签名，caller Drop 关闭源并退款。
Preview true/false 保留 canonical tool choice，错误类型网络前拒绝，完整子集工具两轮结算及等待期取消通过。

27 项 scoped 源码审计（含复核未改文件）UTF-8 无 BOM、Rustfmt/whitespace/diff 通过，最大 484 有效行。
planner 483→484、原 Full response 85→21、Express response 33→23、共享 history 203→230；每个新/提取文件均低于 500。
checker **31 passed**；ratchet 4492 文件、14 immutable runtime 单列、governed source >700 为 0，未改 baseline/exception。
原始 before、迁移映射、含新增文件的 scoped diff 及最终文档/资源审计均在证据目录；三个旧 Express 回执只修迁移后的源码链接，不重写历史成绩。

复现入口：

```powershell
cargo test --locked --no-default-features --features runtime-defaults --lib vertex_ --jobs 4 -- --test-threads=1
cargo check --locked --no-default-features --features runtime-defaults --lib --jobs 4
cargo check --locked --no-default-features --features line-gemini-web-reverse --lib --jobs 4
```

## 5. 未完成与接力

- 仍是完整聚合后桥接 SSE，不是实时 token/参数流。Preview 只承诺上述顺序构造子集；signed Preview、未支持的交错/表示、全部 Part/模态、多 choice 与完整 decoder 继续，明确拒绝不能算功能完成。
- Server/Worker 真实 Redis、PG/Hosted、跨进程 receipt、原生 EXE、真实 OAuth/Key/模型资格和实际额度未由本批验证。仅官方线的共享 Gemini Web 既有 15 项缺符号问题不因此消失，不称完整矩阵绿色。
- 累计新增模板仍 **15**；R11/W07、P50/P60/P70、其他原定平台、Express/Luma/Web/媒体剩余及集中真实验收全部保留，不标整卡或总目标完成。
- 没有读取历史凭据或真实 .ng，没有创建 Redis 容器、操作在线实例、升级/重装依赖、出包、提交或推送。临时数据在本批 linshi，无需现在逐项录制凭据。

证据目录：[gateway-vertex-oauth-tools-20261105](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-tools-20261105)。
