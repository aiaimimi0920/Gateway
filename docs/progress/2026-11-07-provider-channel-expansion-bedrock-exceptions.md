# Bedrock：modeled EventStream 异常与公共 TCP 失败结算

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：下述实现与最终定向/相邻验证通过；不是完整 Bedrock、W06 整卡 `offline_ready` 或 P50/P60/P70 完成。

## 1. 本批合同

承接[公共多轮签名历史子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-public-history.md)，本批补真实 AWS EventStream 的失败身份和终态，不增加服务商模板、认证材料、重试器或退款 owner。

| 已知 `:exception-type` | 安全 public code | AWS 异常语义状态 / 分类 |
| --- | --- | --- |
| `internalServerException` | `bedrock_internal_server_exception` | 500 / `ServerError` |
| `modelStreamErrorException` | `bedrock_model_stream_error_exception` | 424 / `ServerError` |
| `validationException` | `bedrock_validation_exception` | 400 / `BadRequest` |
| `throttlingException` | `bedrock_throttling_exception` | 429 / `RateLimit` |
| `serviceUnavailableException` | `bedrock_service_unavailable_exception` | 503 / `ServiceUnavailable` |

上述状态代表合法 discriminator 对应的异常语义，不声称已经返回的 HTTP 200 transport 实际是这些状态。Buffered 在尚未发出公共响应时按安全语义错误返回；公共 SSE 已 handoff 后仍为失败 body/中断，不伪造成功 terminal 或另造公共 error-event 协议。

- `ProtocolStreamError` 使用私有 typed metadata 保留安全 `GatewayError`；旧 Display 和 serde source 链不变，不从上游 message 文本猜测分类。
- exception/error/event 的 discriminator 冲突明确拒绝；unmodeled error 必须有 string `:error-code` 与 `:error-message`。未知 modeled 名称及 unmodeled error 统一为安全 `bedrock_stream_upstream_error` / 502。
- exception payload 必须为合法 UTF-8 JSON object。五类已知 shape 校验 optional `message`；只有 `modelStreamErrorException` 校验其 optional `originalMessage` / i32 `originalStatusCode`。未来未知 shape 只校验 JSON，不猜成员合同。未知字段跳过；原始 message/originalMessage 不保存、不记录、不回显。
- 所有响应开始后的此类上游失败均 `retryable=false`、`FallbackHint::Abort`；AWS 的重试建议不构成推理可安全重放的证明。Buffered 也不重试、不切换备用候选。
- 流诊断保留安全 typed evidence，交由原 terminal gate 恢复 model circuit/rate-limit 分类和原失败结算；退款仍由既有唯一 owner 执行。`validationException` 的 model circuit 归类是 Neutral，但既有 shared availability/smart-routing/AIMD/provider-runtime 仍可记录失败；本批不重写这套共享健康策略，也不宣称上游 validation 健康完全不变。
- 原生 Bedrock route 先让 `Err` 穿过唯一 `TrackedStream`，触发失败/退款，再在 tracker 外把五类已知异常编码成真正的 exception frame。复用原 string headers、payload、双 CRC 与 service-size encoder，不添加正常 `messageStop`/`metadata`。HTTP body 可完整读完并不把 audit 改成成功；未知或普通协议/transport 错误仍保持 body failure，不伪造已知 modeled frame。

## 2. 公开依据与红绿记录

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-modeled-exceptions-20261107`。

本批匿名联网固定了 AWS [ConverseStream](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html)、[ConverseStreamOutput](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStreamOutput.html)、[Smithy EventStream 规范](https://smithy.io/2.0/aws/amazon-eventstream.html)，以及官方 Rust SDK 固定 commit `44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7` 下的三份 error type。原文、HTTP/final URL、时间、bytes 和 SHA-256 见 `sources/receipt.json`。尝试访问的单独 ModelStreamErrorException API 地址实际重定向首页，不作为 shape 合同；optional/i32 依据为固定 SDK 类型。只作行为参考，没有复制/分发第三方代码或修改依赖。

- `red-v1` 是新测试 helper 错误要求 future `Send` 引起的 SQLx HRTB 编译失败，不计行为红测；只修 helper 在线程内部构造 future，不改变生产栈。
- `red-v2` 实际 **0 passed / 7 failed**：暴露旧分类丢失、响应已开始仍许可重放、native body 截断等行为缺口。日志保留。
- `green-v1` 为 **6 passed / 1 failed**。公共三协议连续使用同一对账户，前两个真实 throttling 已触发冷却，第三个 Responses 请求在准入超时；不是 Responses decoder 缺陷。最终为每个协议使用独立 fixture，不关闭 cooldown 或删除失败断言。
- `green-v2` 为 **8 passed / 0 failed**，并补 shape-specific optional/unknown-member 合同。SSE 用 held prefix → 客户端收到首 chunk → 释放异常，验证真正 handoff 后失败。

## 3. 最终验证

Rust 使用 `cargo test --locked --offline --no-default-features --features runtime-defaults -j 4`、`--test-threads=1`。TEMP/TMP/TMPDIR 指向本批证据目录；只有路由合同使用既有明确的 16 MiB 测试线程，不修改生产栈或全局 `RUST_MIN_STACK`。所有 Cargo 阶段串行执行。

| 范围 | 结果 | 终态回执 |
| --- | --- | --- |
| 11组 Bedrock 异常/decoder/native/reasoning/history/probe integration | 106 passed，其中本批异常8项 | `contracts-final-v1.json` |
| 库 `protocol::stream_error` | 5 passed，其中本批typed分类1项 | `library-protocol-stream_error-final-v1.json` |
| 库 `protocol::bedrock_converse` | 4 passed | `library-protocol-bedrock_converse-final-v1.json` |
| 库 `pipeline::stage_send` | 69 passed | `library-pipeline-stage_send-final-v1.json` |
| 库 `upstream::stream` | 52 passed | `library-upstream-stream-final-v1.json` |
| 正常生产 lib / 代表性关闭 Bedrock 的生产 lib | 均 exit 0 | `production-final-v1.json`、`disabled-final-v1.json` |
| 本批15项 Rustfmt / checker tests / ratchet | 均 exit 0；checker31/31，ratchet扫描4820项 | `rustfmt-final-v1.json`、`checker-final-v1.json`、`ratchet-final-v1.json` |

按 harness 与完整测试名去重，最终 **236 passed / 0 failed / 0 ignored**，其中本批新增9项。只统计上述终态，不叠加中间绿测或前批631项；不是全仓回归、完整 feature 矩阵或正式发布门禁。既有 unused/dead-code warnings 保留，不为本批做无关清理。

原 `remaining.py` 在正常生产 lib 通过后、关闭组合启动前因使用不存在的 line feature 名触发断言；这是一次性 verifier 缺陷，不计产品编译失败。保留原脚本和 `remaining-v1-failure.json`，只用 `remaining-tail.py` 补尚未执行阶段。实际从 `runtime-defaults` 精确去掉 `family-bedrock-converse-official-api` 与 `line-aws-bedrock-converse-official-model-api`，不改 feature 定义、不削弱断言、不重跑或覆盖已有通过回执。

测试复用生产 `build_app_state`、Local SQLite、真实管理 TCP 创建的普通接入 Key，余额预置 2；Gateway 与合成 AWS 均绑定随机回环端口。配置明确备用候选，检查一次上游发送、失败 audit、余额仍为 2、active permit 为 0、无成功计数。五类 native 异常检查合法头/双 CRC/唯一 terminal exception/私密值不泄漏；三公共 SSE 检查中断且无成功 terminal。这不是独立程序或真实 AWS 验收。

10个终态阶段各自冻结的4245项所选 Cargo/Rust/worker MJS 输入与终态逐项一致，不外推全部UI/环境/runtime数据。15项Rust（4新增）、五份文档、固定原文hash、UTF-8/无BOM、本地链接、有效行前后与范围diff见 `audit.json`、`test-identities.json`、`scoped-diff.patch`；`git diff --check` 通过。前镜像中只有 `src/protocol/bedrock_converse/eventstream.rs` 是移除恰好新增函数后重建，完整bytes与修改前 `initial-source.json` 的SHA一致，不称其为direct pre-edit copy；其余保留直接写前副本，细节见 `preimage-provenance.json`。

本批15项Rust最大456有效行，无501–700例外或baseline调整；ratchet继续报告14项固定immutable runtime assets，`governed source >700=0`。根有效行报告复制入证据后按本批写前原bytes恢复，保留继承dirty报告；baseline/exception原bytes未改。源码审阅覆盖输入/秘密、JSON/帧预算、分配/错误链、一次失败/退款、健康分类、source释放与服务端task收尾；本批不新增持久schema、后台调度器或重试机制。

## 4. 剩余边界

本批补公共 TCP 的异常/退款子链，**公共 TCP 完整成功、工具、历史与取消链**仍未集中验；完整参数/模态、Bedrock 专属各阶段完整取消审计、独立进程/原生包、真实 AWS IAM 权限/模型资格/额度与到期行为也未完成。前批 631 passed / 2 ignored 是公共历史子批的历史证据，不改成或叠加到本批数量。

累计新增模板仍 17；FreeBuff 公共业务 guard、其他未完渠道、Suno/Luma MCP client/tools、Qwen OAuth 待决和 P50/P60/P70 继续。没有读取真实 `.ng`/凭据、调用真实供应商、操作现有实例、新建 Redis 容器、出包、提交或推送；P34 授权已消费并清理，不重复使用。
