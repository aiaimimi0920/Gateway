# Bedrock：实时 reasoning 事件与原生流往返

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：下述实现与定向验证通过；不是完整 Bedrock、整卡 `offline_ready` 或 P50/P60/P70 完成。

## 范围与合同

承接[非流式响应子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-buffered-reasoning.md)，这次接真实 AWS EventStream 的实时 reasoning、signature、redacted 分片，以及 text/tool 原 native block index、逐块 close、stop/metadata。

- normalized Chat SSE 每个原生事件带显式 `gateway_bedrock_event`（sequence/event/body），普通文本/工具 delta 仍照常输出；不把 reasoning 内容充当普通答案或其他协议原生 thinking。普通 text/tool input 用可选 `projection: content|tool_input` 指向同帧标准 delta，原 body 对应位置为 null 占位；receiver 必须按明确路径恢复，再验证整个标准 delta 与原生状态一致，不重复保存/序列化大段可见内容。未知引用、缺少来源、非 null 占位和错误事件类型均拒绝。
- 最终 `gateway_bedrock_terminal` 与可选 `gateway_bedrock_turn` 必须与已观察的原事件、可见 delta、完整 usage/finish 一致；生产者只有 CRC/frame/state/真实 EOF 全通过后才生成终态。
- Messages/Responses 用显式 `gateway.bedrock.event` SSE 扩展传递已恢复的完整实时事件（不含内部 projection 占位），正常文本/工具事件保留；公共成功终态需要消费完整 carrier、DONE 与 EOF，不把仅有快照当实时保真。
- 原生 encoder 不再对 carrier 走旧的单 text/dense tool block 重排路径；保留原索引/分片/逐块 close，messageStop/metadata 保留到实际 EOF 成功边界。
- 新 carrier 只是响应状态，不是凭据、来源签名验证、模型/产品/前缀绑定或公共下一轮历史授权；公共签名多轮继续单独追踪。
- text/XML 工具注入检测会整体缓冲并可能重建输出，不能与原生 witness 并用。共享 detector 在首个声明处显式拒绝该组合并释放 source，不丢扩展后伪装成普通流成功；正常 Bedrock native tools 不走此检测。

## 来源与红绿

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-reasoning-stream-20261107`。
已联网取得并固定 AWS reasoning delta、content delta、message stop、metadata 与 Smithy EventStream 五份原文，HTTP 状态、原 URL、时间、字节数和 SHA-256 见 `sources/receipt.json`。delta 是 text/signature/Base64 bytes 的 union；完整 opaque 值不 trim、拼 Base64 文本或假冒其他产品签名。

`red-stream-v1`：新增四项真实行为测试 **0 passed / 4 failed**，均在当前的 `streaming reasoning response is not yet supported` 失败。公共双协议循环只证明实际走到的首个分支，不虚称两分支都独立红测。源码在红测执行期间未漂移。

`green-stream-v1`：3 passed / 1 failed。直接增量、原生逐事件往返及 EOF 前交付已通过；公共双协议触发 futures `Unfold must not be polled after it returned Poll::Ready(None)`。修复 Bedrock 专属的已知 EOF 分支，避免 Responses 终态之后再 drain 已结束的非 fused source；不修改 futures 或依赖版本。

`green-stream-v2`：新增12项与前批18项合计30 passed；构建期间新增/接线了 stream route 测试两个文件，完整所选输入 `source_unchanged=false` 如实保留，生产输入未变，不能把它当最终冻结证据或与后续重复累计。

`red-frame-budget-v1`：34 MiB 普通 text 的针对性测试 **0 passed / 1 failed**，实际复现 carrier 双写把已支持的 client frame 预算缩小；之后改为同帧投影引用。原生 output 保持符合 service 限制的原分片，仅对过大 text/tool input 按 UTF-8 边界分片，保留 native index 与 body 的其他字段，不用服务端 24 MiB 限制收紧入站 client decoder。

中间 `stream-final-v1` 为32 passed / 1 failed，失败项为新加的原生大文本输出测试；该次 frame/native/test 在运行期间变化，`source_unchanged=false`，不作最终冻结证据。最终以修补和格式化后的 v2 为准，不把中间绿测、失败后重跑或前批测试重复累计。

## 最终定向验证

Rust 使用 `cargo test --locked --offline --no-default-features --features runtime-defaults -j 4`，`--test-threads=1`，TEMP/TMP/TMPDIR 指向本批证据目录；没有给全部测试增加 `RUST_MIN_STACK`。新增正常 router 合同沿用明确的16 MiB专属测试线程，不改变生产栈或 future。

| 范围 | 结果 | 回执 |
| --- | --- | --- |
| 实时、非流式响应及公共 JSON reasoning 三组 | 33 passed，其中实时15项新增 | `stream-final-v2.json` |
| 13组 Bedrock 认证、参数、长模型、native、上游、probe、签名历史合同 | 93 passed | `adjacent-final-v1.json` |
| 库过滤器 `bedrock` | 30 passed | `library-bedrock-v1.json` |
| 库过滤器 `anthropic` | 94 passed | `library-anthropic-v1.json` |
| 库过滤器 `responses` | 158 passed / 1 ignored | `library-responses-v1.json` |
| 库过滤器 `tool_inject` | 64 passed | `library-tool_inject-v1.json` |
| 库过滤器 `vertex_turn` | 12 passed | `library-vertex_turn-v1.json` |
| 正常生产 lib 与仅关闭 Bedrock 的代表性生产 lib | 均 exit 0 | `production-final-v1.json`、`disabled-final-v1.json` |
| 18项 Rustfmt、checker tests、ratchet | exit 0；checker 31/31，ratchet扫描4810项 | `rustfmt-final-v1.json`、`checker-final-v1.json`、`ratchet-final-v1.json` |

按 harness 与完整测试名去重，最终 **478 passed / 0 failed / 1 ignored**；库过滤器相互重叠，不直接相加。ignored 是要求 `GATEWAY_STATUS_FIXTURES` 的显式官方状态响应录制回放，不是本次 Bedrock 合同，也未计入通过。不是全仓回归、完整 feature 矩阵或正式发布门禁。

12个终态阶段各自冻结的4235项所选 Cargo/Rust/worker MJS 输入与终态逐项一致；不把它外推为所有 UI、环境、外部依赖或任意 runtime 数据均被证明。18项本批 Rust 源码、五份文档与固定公开来源另按 `audit.json` 核对严格 UTF-8/无BOM、direct preimage、范围diff、有效行和链接；未修改 baseline/exception，根目录有效行报告在证据复制后按原始 bytes 恢复，不覆盖继承脏改。

本批18项Rust、8项新增，最大478有效行；`to_anthropic.rs` 500→423、`to_responses.rs` 470→478、`frame.rs` 127→170、`tool_inject/streaming.rs` 429→448。新增 relay200、native49、capture67、Messages wire85、Responses completion45；三个新增测试文件239/270/203，均不需要501–700例外。一次临时测量命令的 PowerShell 嵌套引号错误已改用 here-string 完成；未改生产语义或计作功能红测。

安全及生命周期审阅限定于这些变更：carrier 不承载凭据，未知投影/顺序/混合产品状态/终态篡改明确拒绝；4 MiB可选有序快照与既有frame/queue上限继续生效，普通文本不重复进入同帧；出站分片按UTF-8边界并保留原事件其他字段。失败、取消与EOF释放source，不重试、不新增后台任务或浏览器进程；错误不回显reasoning/signature内容。原生成功跟踪只在完整输出及EOF校验后收敛。没有把响应 witness 当作真实模型签名校验或后续历史授权。

## 继续保留的边界

公共下一轮签名历史、完整参数/模态、modeled exception、专属完整取消审计、公共 TCP/独立进程/原生包及真实 AWS 权限/模型/额度仍未验。累计模板仍17；没有读取实际 `.ng`/凭据、调用真实供应商、改在线实例、创建容器、出包、提交或推送，不复用历史 P34 授权。

下一步最小任务是把有序响应内容接入带产品/模型/原请求历史约束的公共下一轮签名重建；先补多轮和策略冲突合同，不能直接将 `gateway_bedrock_turn` 视作可信历史 token。FreeBuff 的完整公开执行与费用/终态、其他渠道和 P50/P60/P70 保持原计划，不因本子批通过撤掉 guard 或标全功能完成。
