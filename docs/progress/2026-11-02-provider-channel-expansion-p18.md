# 服务商分渠道扩展：第 18 批，普通 Qwen 流式终态与 Chat 缓存用量

> 2026-11-02 开工、2026-11-03 收尾（Asia/Shanghai）；R09 / W05 / P01 / P10 的无凭据切片。实现及本批定向验证已收尾；完整通用 Responses suite 仍有两项失败，不是 W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有脏改，没有提交、推送、打包或重启实例。

## 1. 产品与实现

仍是普通 `qwen-api` / `qwen-official-api` workspace API Key 产品。API Key、OAuth、Web 分别评估；Web 不是兜底，不因其他认证存在而删除。没有证据的额度共享关系保持隔离、标 `unknown`。本批没有改变产品名称、认证受众、额度池或最终 R01–R16 范围，也没有新增外部事实、依赖或第三方源码。

- [stream_preflight.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/stream_preflight.rs) 按 `attempt.effective_payload` 的 workspace 产品 ownership 接入新 guard，在协议转换和 usage tap 之前工作。该产品不再先走 generic early-stream replay，避免其抢先解释或回显首帧 error；其他产品的旧路径不变。
- [workspace_stream.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/qwen/workspace_stream.rs) 是惰性的同 task Stream，不 spawn、不后台 drain。复用 strict UTF-8/SSE decoder；正常文本和工具参数仍逐帧流出，不累计整段输出。相同 chunk 内先好帧后坏帧，已经合法输出的好帧不会整块被吞掉。
- [workspace_stream_state.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/qwen/workspace_stream_state.rs) 分离同帧 delta 和 finish：最后文本/工具参数仍实时转发，只暂存 finish、最终 aggregate usage 和 `[DONE]`，保持终态事件原序。必须观察到每个已见 choice 的合法 finish、唯一 DONE 和干净 HTTP body EOF，才释放终态尾部。
- 这是为现有 Responses converter 的实际行为设置边界：它不仅会在 EOF/DONE 补 completed，也可能在 finish+usage 时提前完成。新 guard 不使用转换后的 completed 自证上游完成。等待的是 HTTP body EOF，不是要求关闭可复用 TCP 连接。
- 帧上限复用共享 **64 MiB**；暂存尾部另有 **64 MiB 累计字节 / 256 事件**预算，choices 最多 128 个唯一 index，每 choice 最多 128 个唯一 tool index，不按最大 index 分配数组。每次 poll 最多处理 32 个步骤后协作让出。这些是 Gateway 本地准入策略，不是厂商额度或服务上限。
- 同 choice finish 后再次输出、重复/非法 index、未知终态、缺 finish/DONE、DONE 后任意 data/event（包括空 data）、坏 UTF-8、畸形 JSON、未结束帧和 error envelope 均失败。完整注释/空白不会被误当 data。usage 可缺省；非 null 时仅接受已见 choices 全部完成后的单次最终 aggregate，整数/checked total/cache primary/alias 复用 P17 校验。
- 冗余 `input_tokens/output_tokens` 必须成对且与 Chat counters 一致，验证后移除，防后续 tap 切换到另一套计量分支；顶层 `response` envelope 拒绝，防嵌套 usage 绕过 Chat 准入。不会放宽共享 parser 或其他产品协议。
- 协议失败只输出固定 `invalid_qwen_workspace_stream` 诊断，不回显任意上游 body/parser 原文；错误或 drop 释放 inner transport 和尾部，不补 DONE/成功 usage，不自动重放已接受的生成。继续使用既有 TrackedStream/finalizer 的成功、失败和取消归属。
- [response_builders.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/openai/response_builders.rs) 的 Chat success/stop 两个 builder 投影 canonical `cache_read_input_tokens` 到 `usage.prompt_tokens_details.cached_tokens`。未知时保持缺省，显式 0 保留，不编造 usage；legacy Completions 和其他 finish mapping 不变。

## 2. 离线安全与真实路径

复用 P17 正式 prepare/planner/header/sender 后的 `cfg(test)` task-local seam、两个合成账号、workspace-b override、模型映射及 Codex 指纹。生产没有配置或环境开关可启用 seam；随机 loopback server 同时为非转发 proxy，无 seam 的 HTTPS CONNECT 继续被拒绝。没有读写真实 `.ng`、凭据或数据库，也没有试历史账号、联网生成或产生上游费用。

server 新增真实 `text/event-stream`、7-byte HTTP 分片、可持有未结束 body 和 Drop probe。公共 Chat/Responses 的惰性 body 消费与 drop 均包在同一 transport scope，终态 audit 明确等待 `completed/failed/cancelled`，不以“非 started”中间记录当最终完成。测试逐请求核对唯一发送、精确 Key、provider 与 `route_trace.realCredentialRef=exact`，不混淆 provider ID 和 credential ID。

取消测试在已收到正常 delta、上游已发送 finish/usage/DONE 但 HTTP body 仍未结束的窗口 drop：要求客户端此前没有成功终态，audit 为 cancelled、没有成功 usage、模型 failure_count 不增加，并观察 server body 被释放。fixture 最后等待状态引用释放、关闭独立 LocalRuntime；这不是现金退款、真实账号健康或所有 permit 策略的验收。

TEMP/TMP/TMPDIR 均指向 [P18 证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p18) 下的 runtime-temp；所有合成 SQLite 留在该目录。本任务 Cargo 串行使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，不启用 management-ui，不调用工作区 Web 发布/清理。观察到独立 Otter 构建并存，没有停止或修改它，也不把编译耗时当稳定性能指标。

## 3. 验证历史与当前结果

### 3.1 保留的红测、审查发现和失败

- `red-pipeline-v1.log`：**10 pass / 3 fail**。实际复现 Chat buffered cache details 缺失、未结束 HTTP body 已透出 DONE、delta→EOF 没有失败。矩阵在首个断言失败后停止，因此不声称红测逐个执行了全部迟发错误或 Responses 分支；最终绿测另有完整循环覆盖。
- `green-pipeline-v1.log` 15/15、`green-qwen-v1.log` 38/38、`openai-regression-v1.log` 63/63 已实际通过。随后 `responses-regression-v1.log` 的完整通用 Responses suite 为 **51 pass / 2 fail**：`accumulate_responses_rejects_explicit_failure_after_partial_output` 和 `translate_responses_failure_never_emits_successful_stop` 仍包含 `private-upstream-detail`。这些 owning 源码本批未修改，但没有 clean-HEAD 对照，不冒称已证明历史 baseline；没有通过删测试、放宽断言或隐藏日志“修绿”。v1 脚本在此实际 exit 1，后面的门禁当时未执行。
- 只读交叉审查发现空 data 被当注释、usage alias 导致后续计量分支不一致；主代理另发现嵌套 Responses usage 可绕过 Chat tap。均在正式最终验证前修正并新增回归。它们是源码审查发现，不虚构独立执行红测。
- v2 保留全 suite 失败记录，单独执行直接相关的 Chat→Responses 转换、typed error/drain/cancellation 回归；这不是完整 Responses suite 已通过，也不是完整仓库全绿。

### 3.2 最终来源的定向验证

执行器为证据目录中的 `run-green-v2.ps1`；下列 library 日志已实际 exit 0，无 0 tests 冒充通过。

| 日志 | 实际 test 数 | 当前结果 |
| --- | ---: | --- |
| `green-pipeline-v2.log` | 15 | pass；P17 buffered 与新增五项 stream 测试，含 Chat/Responses、工具/length/filter、错误/截断、UTF-8、首帧脱敏、取消和连接释放 |
| `green-qwen-v2.log` | 39 | pass；包含 7 项新 guard、8 项新状态机、6 项 P17 buffered，不重复累加 |
| `openai-regression-v2.log` | 63 | pass；包括 cache Some/0/None 和 content_filter builder |
| `responses-regression-v2.log` | 10 | pass；直接转换/输出顺序/超限 |
| `responses-error-regression-v2.log` | 7 | pass；typed error、EOF drain、取消及 transport failure |
| `decoder-regression-v2.log` | 13 | pass；strict/legacy UTF-8、EOF、BOM、帧边界 |
| `tracked-regression-v2.log` | 47 | pass；usage/终态观察、drop、单次回调及资源释放 |
| `probe-regression-v2.log` | 26 | pass；保留 P16 精确 probe 与既有测试 |
| `sender-regression-v2.log` | 3 | pass；最终 sender、编译产品和 compact 拒绝 |
| `integration-regression-v2.log` | 10 | pass；不含测试 seam 的生产 library，Cohere 真 router 四项及 Qwen workspace runtime 六项 |

最终 v2 library 定向 **223 pass**，加生产 integration **10 pass**，合计 **233 pass**；不将 v1 重跑或完整 Responses 中的重叠通过项再次累计。v2 脚本实际 exit 0，所有本任务 Cargo sessions 均已结束。P17 的 API-off 代表组合本批未重跑，不能称本批已验证全部 feature 组合。
`run-static-v1.ps1` 实际 exit 0：Rustfmt、checker **31/31**、ratchet **4014 文件 / 14 immutable / governed source >700=0**、全工作树 `git diff --check` 均通过。ratchet 使用无报告入口，未覆盖旧的脏报告；Git 的 CRLF→LF 提醒不是空白检查失败。

## 4. 来源与有效行

`source-before.json` 记录本批写入前 12 个 owner 的 hash/有效行（六个新文件为 0）；`source-frozen-v1.json` 记录最终 12 个源码，生产 integration 在同一源码状态完成。最终来源、严格 UTF-8 无 BOM、四文档本地引用/围栏和九个未跟踪源码/四文档独立 diff 检查记录在 `source-verify-final.log` / `untracked-diff-final.log`。P17 的五个 owner 因本批模块/fixture/cache 接线有意变化；其余 P17 frozen source 保持匹配。P11 的 sender seam 已由 P17 接替并按 P17 hash 核验，其他 P11/P12/P14/P15/P16 最后 frozen source 仍受审计脚本保护。

| 源码 | 前 → 后有效行 |
| --- | ---: |
| `src/protocol/qwen/mod.rs` | 12 → 14 |
| `src/protocol/qwen/workspace_response.rs` | 148 → 148，仅共享校验可见性 |
| 新 `workspace_stream.rs` | 122 |
| 新 `workspace_stream_state.rs` | 213 |
| 新 `workspace_stream_tests.rs` | 199 |
| 新 `workspace_stream_state_tests.rs` | 159 |
| `src/protocol/openai/response_builders.rs` | 210 → 249 |
| `src/pipeline/stage_send/stream_preflight.rs` | 266 → 276，仍是原始流预检单一职责 |
| `src/upstream/client/qwen_pipeline_test_support.rs` | 150 → 208 |
| `src/upstream/client/qwen_pipeline_tests.rs` | 197 → 200 |
| 新 `qwen_pipeline_stream_tests.rs` | 249 |
| 新 `qwen_pipeline_stream_support.rs` | 93 |

没有新增有效行例外、baseline、依赖升级、锁文件变化、存储版本或迁移。本批不修改其他人的桌面连接/设置/UI 脏改，不接管 S06 Rust/Gemini lane。

## 5. 未完成与下一最小任务

1. **下一产品任务：共享 probe Recording 外层 future drop 的 audit/finalizer guard。** P16 已知缺少 Drop 兜底；普通 buffered 外层取消 audit 也未闭合，不能拿本批 Stream drop 证明另两条路径。
2. 通用 Responses 两项脱敏失败独立保留待定位，不把 Qwen 产品 guard 的固定错误诊断外推为所有协议均已修复。此前 core-only Gemini feature 组合失败、P13 极端 expiry、P15 前端四项失败与 P13 发布/清理授权遗漏也不抹除，本批未重跑或接管它们。
3. 本 guard 只验证已观察 choices 的终态，不是按请求 n 验证完整 choice 数量，更不是既有单 choice canonical/Responses bridge 的多 choice 无损实现。reasoning/refusal、legacy function_call 完整跨协议保真、所有工具 metadata/参数 JSON 完整性仍需对应 owner 的独立闭环；工具基本路径通过不等于全部工具边界完成。
4. Bedrock 参数/reasoning/signature/取消，其余 API/OAuth/Web/媒体、P50 devtest 包/隔离启动、集中录制、真实逐渠道登录/刷新/生成/额度仍完整保留。没有真实账号、费用、浏览器手测、EXE 包或启动证据，W05 和总 goal 保持 `in_progress` / active。
