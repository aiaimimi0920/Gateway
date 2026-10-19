# 相似网关特性对照与首批实现

## 固定参考

2026-10-19 联网读取官方 GitHub API、README、许可证和选定源码。默认分支与
GitHub `releases/latest` 分开记录，不能把默认分支新代码都算作已发布能力。

| 项目 | 默认分支及固定 SHA | GitHub 最新 release | 许可事实 |
| --- | --- | --- | --- |
| sub2api | main / `b0a02a3304a0a78cdd003ddda1dd23ba02c03084` | [v0.2.17](https://github.com/Wei-Shaw/sub2api/releases/tag/v0.2.17) | [LGPL-3.0](https://github.com/Wei-Shaw/sub2api/blob/b0a02a3304a0a78cdd003ddda1dd23ba02c03084/LICENSE)，README 另有无商业授权声明 |
| gpt-load | main / `1cedccf2fa6e7a45c01a5651e0bbe9aad78a4fc9` | [v2.0.0-rc.51](https://github.com/tbphp/gpt-load/releases/tag/v2.0.0-rc.51) | [根 MIT](https://github.com/tbphp/gpt-load/blob/1cedccf2fa6e7a45c01a5651e0bbe9aad78a4fc9/LICENSE)，依赖另有许可 |
| AISIX | main / `33b55b8f337485c31c9df850bae01ffb0fedcb71` | [v1.6.0](https://github.com/api7/aisix/releases/tag/v1.6.0) | [Apache-2.0](https://github.com/api7/aisix/blob/33b55b8f337485c31c9df850bae01ffb0fedcb71/LICENSE) |
| AxonHub | unstable / `8d3c3865dc642307cabe0950e241d2c231c08f42` | [v1.0.0-beta11](https://github.com/looplj/axonhub/releases/tag/v1.0.0-beta11) | [NOTICE](https://github.com/looplj/axonhub/blob/8d3c3865dc642307cabe0950e241d2c231c08f42/NOTICE) 区分 Apache-2.0、`llm/` LGPL-3.0 和单文件声明 |

gpt-load 和 AxonHub 的 API 虽标记 `prerelease=false`，标签仍含 RC / beta，不能称为 GA。
本批只借鉴可观察行为，代码与测试均用 Gateway 自有 Rust 类型重新实现；不复制参考
源码、注释或测试夹具，不引入其执行器、依赖、控制平面或业务系统。

## 本批采用：完成证明不能由干净 EOF 代替

参考 sub2api 的 [raw stream truncation](https://github.com/Wei-Shaw/sub2api/blob/b0a02a3304a0a78cdd003ddda1dd23ba02c03084/backend/internal/service/openai_raw_stream_truncation.go)。
Gateway 的 Responses 已有完成保护，本次定位到 `openai/stream_translate.rs` 的
Chat → legacy Completions 转换仍会在没有终止信号的正常 EOF 后补 stop / DONE。

- 只约束该转换器，包含它识别出的 native legacy 兼容分支；不是所有 Chat 原生透传路径。
- 非空首 choice `finish_reason` 或 `[DONE]` 证明完成；单独 usage 不证明完成。
  不照搬参考项目把任意 usage 对象当成终止信号的宽容策略。
- 缺少证明时保留已交付及已按既有 EOF 规则解析的 delta，输出一次固定脱敏
  `ProtocolStreamError`，消息为 `openai_chat_stream_incomplete`；不补成功终态。
- transport error 保留原对象；EOF / error 在报告前释放 source；下游 Drop 释放原流。
  没有新增请求、重试、任务或 replay。仅 `[DONE]` 或既有 finish + usage 条件提前结束。
- 保留 finish → usage → DONE/EOF 的累计语义。单独 finish 后仍可等 usage，不新增
  idle timeout；整个请求继续受既有预算约束。不能声称任意 finish 一出现就立即断流。
- 保留既有 decoder EOF 合同：已完成换行的字段可 flush；无结尾换行的最后一行仍忽略。
  native legacy 有 finish 而无 DONE 时保持既有透传输出，不额外合成 DONE。
  该分支沿用既有 SSE 格式化，不承诺整个原始输入逐字节透传。

这是有意收紧旧兼容行为：原来缺 finish/DONE 的上游会收到协议错误，而非虚假成功。

## 本批采用：仅流首 UTF-8 BOM 兼容

参考 AISIX 的 [公共 SSE 实现](https://github.com/api7/aisix/blob/33b55b8f337485c31c9df850bae01ffb0fedcb71/crates/aisix-gateway/src/sse.rs)，
并核对 [WHATWG SSE 解析规范](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream)。

- 接入共享 `BoundedSseDecoder` 和只读 `ObservationLines`，不复制另一个解析器。
- 只忽略第一行开头的一个 UTF-8 BOM，支持 BOM 三字节和 UTF-8 文本被任意网络 chunk 拆分。
  后续帧、后续行和数据内容中的 U+FEFF 不删除；discard 恢复不重置流首身份。
- BOM 原字节先经过既有准入再识别，不从 64 MiB frame 或 observation 行数预算中扣除。
  只使用一个 bool 和既有缓冲切片，无新缓冲、扫描全流或后台任务。
- native Responses 与 usage/completion observer 保持输入 Bytes 的内容和 chunk 边界。
- **裸 CR 分行没有在本批实现**。仍是既有 LF / CRLF 路径，不宣称完整 SSE 规范兼容。

## 其余取舍

| 参考行为 | 本地现状与决定 |
| --- | --- |
| gpt-load [模型冷却沿用 Retry-After](https://github.com/tbphp/gpt-load/blob/1cedccf2fa6e7a45c01a5651e0bbe9aad78a4fc9/internal/health/model_cooldown_test.go) | Gateway 已有模型隔离与请求等待提示，但 model health 默认固定 5 分钟。值得下一批贯通明确的限流证据；必须区分显式提示与分类默认值，覆盖 Local/PG，不直接全局改默认。 |
| AxonHub [流式/非流式分开延迟评分](https://github.com/looplj/axonhub/blob/8d3c3865dc642307cabe0950e241d2c231c08f42/internal/server/orchestrator/lb_strategy_latency.go) | Gateway 已记录 TTFT，但评分主路径没有该输入。值得后续做有界样本、过期和冷启动中性策略；不能只增加未接线公式。 |
| AxonHub [模型级单探针恢复](https://github.com/looplj/axonhub/blob/8d3c3865dc642307cabe0950e241d2c231c08f42/internal/server/biz/model_circuit_breaker.go) | 值得继续核查冷却到期惊群；尚未证明全部 owner 缺失。需明确 Local 多进程 / Server 多实例租约及取消，暂不加入仅进程内锁。 |
| AxonHub 硬 priority 分组 | 与 Gateway 默认 weight-first 不同；不是已证 bug，不静默改变默认策略或 priority 数值方向。 |
| sub2api 拼接 JSON 恢复 | 非标准容错，暂不放宽全局 JSON/SSE 解析；必须先有真实兼容需求和严格文档数、大小、event/type 一致性合同。 |
| AISIX SSE 裸 CR | 是真实可定位差异；需要同时验证跨 chunk CRLF、即时分帧与原始字节准入，不靠预先 replace 换行实现。 |
| heartbeat、共享 retry/deadline、模型健康、sticky | Gateway 已有相应 owner；不按参考项目名重复造一套。 |
| 支付、用户中心、编排/ensemble、隐式跨账户回退 | 不搬入 Gateway，保留跨项目所有权与现有精确回退授权。 |

额外静态线索：`routing/queue.rs` 的 blocked sticky 提升与自身注释存在矛盾；尚未验证
send gate 是否阻断，未在本批修改，也不把它误归因为 AxonHub 已解决的参考行为。

## 验证记录

首轮新合同实际运行 10 项：6 failed、4 passed，复现缺终止信号被补成功和流首 BOM
丢首帧的行为；不是用编译失败作 red。随后扩展到 13 项，首轮 12 passed、1 failed，
发现 `unfold` 在完成后被重复轮询会 panic；已用 `fuse()` 保证终止后轮询安全。

首轮完整 lib 回归为 3441 passed、1 failed、36 ignored；唯一失败是
`execute_chatgpt_official_forced_streaming_accumulate_returns_canonical_response`。
其旧夹具只有 `type: response.completed`，缺少当前 Responses 完成合同要求的
`response` 对象；专用负例测试明确要求拒绝这种输入。第二轮补入对象后仍失败，
错误进一步定位到 canonical reader 要求 `output` 数组；最终只把该夹具补为
`response: {status: completed, output: []}`，保留原成功断言，没有放宽生产解析器。

最终默认 feature 完整 lib 回归通过：3442 passed、0 failed、36 ignored、0 filtered out，
包含修正后的原失败用例。36 项显式 opt-in / 子进程探针忽略项不计作通过，涵盖隔离
PostgreSQL/Redis、真实供应商、浏览器及仅由父测试调用的 child probe。
不把中间的 12/13 或上述失败轮次当作最终 green。

6 个直接相关 integration targets 已通过 61 项，均为 0 failed、0 ignored、0 filtered out：

| Integration target | 通过数 |
| --- | --- |
| `pipeline_send_runtime_contract` | 29 |
| `reference_stream_contract` | 13 |
| `reference_stream_runtime_contract` | 2 |
| `responses_native_completion_contract` | 5 |
| `responses_stream_completeness_contract` | 8 |
| `sse_typed_pipeline_contract` | 4 |

新增真实 loopback HTTP 合同验证截断不记成功、不 replay，以及 BOM 下正文、usage、资源
释放。最后一次改动只涉及 `#[cfg(test)] mod tests` 内的一行 lib 测试夹具，不改变上述
integration targets 使用的生产逻辑；因此 lib 与 integration 结果分别取自
`closure-lib.log` 和 `final-regression.log`，不是宣称一次全量命令全部绿色。
两部分合计 3503 passed、0 failed、36 ignored；没有运行全仓所有 integration targets、
44 条线路 feature 编译矩阵、Docker/发行包或真实供应商业务验收。

已完成的独立治理检查：

| 检查 | 实际结果 |
| --- | --- |
| `npm run test:effective-lines --prefix scripts` | 31 passed |
| `npm run check:effective-lines --prefix scripts` | ratchet passed |
| effective-lines strict | passed；2969 个受测文件，受治理源码中超过 700 有效行的文件为 0；14 个 immutable runtime assets 单列 |
| `cargo fmt --all -- --check` | passed |
| `git diff --check` | passed |
| line manifests | 44 checked、0 errors |

有效行使用仓库权威 lexer 计数：`stream_translate.rs` 269 → 284、
`stream_translate_tests.rs` 230 → 231、`stream_decode.rs` 307 → 343、
`observation.rs` 140 → 151、`observation/tests.rs` 194 → 261；
新增两个 integration 文件分别为 267、86。
回归发现的 `upstream/chatgpt/official_api/execution.rs` 仅修正一行测试夹具，
有效行 358 → 358，生产逻辑不变。
没有增加依赖、基线或例外；已有 dirty 的 `scripts/artifacts/effective-code-lines.json`
保持原字节。checker 的默认新产物实际位于仓库根 `artifacts/`，本轮报告另外复制到
证据目录；不把旧的 scripts 下报告误当成本轮报告。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-reference-20261019/`。
本批不提交、推送、部署或替换已有发行包，不改 S06、存储版本、schema、依赖或秘密配置。
最终检查保留本轮前已有的 37 个 dirty / untracked 文件的原字节；HEAD 未改变。
本轮 9 个新增或修改的源码、测试和文档均为 UTF-8 无 BOM。
