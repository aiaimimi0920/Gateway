# CLIProxyAPI 特性对照与增量实现

## 固定参考与边界

2026-10-19 联网核实上游最新正式版本为
[v8.0.33](https://github.com/router-for-me/CLIProxyAPI/releases/tag/v8.0.33)，
release 指向 `2422c6b`。只参考固定版本的行为合同，不将滚动 main 当作发布事实。
上游为 [MIT](https://github.com/router-for-me/CLIProxyAPI/blob/v8.0.33/LICENSE)。
本轮以 Gateway Rust owner 重新实现协议行为，不复制 Go 实现，不引入 CPA runtime。

不改 S06 Rust/Gemini owner、现有私密配置、凭据、存储版本、重试授权、发布门禁。
不提交、推送、部署、调用真实供应商或覆盖已有包。保留开始时全部 dirty edits。

## 对照与取舍

| 能力 | Gateway 事实与判断 | 本轮决定 |
| --- | --- | --- |
| 协议感知的流截断检测 | Responses 转 Chat 的 EOF 分支会合成成功 stop；buffered accumulator 会将仅有 delta 的响应返回成功；原生转发的正常 EOF 也会触发成功回调 | 优先实现 Responses 原生、跨协议及缓冲路径的终止证明 |
| 文件快照防覆盖 | routes revision/CAS 和 OAuth refresh generation 已存在；folder-sync/Redis 提交仍有待复现的存储交错风险 | 值得后续独立复现；本轮不把已有保护重复移植，也不贸然修改凭据存储 |
| Claude 短期限流与长期配额冷却区分 | Gateway 已有共享等待预算、提示解析及请求级预算，但与上游按供应商分类的 cooldown 并非同一合同 | 保留为定向核查项，不全局修改 Retry-After 优先级 |
| 大工具集 schema 清洗复杂度 | 上游优化针对其 JSON offset 修改路径；不能据此推断 Gateway AST 路径存在相同问题 | 没有本地复杂度证据前不移植算法 |
| Session affinity 的故障重绑策略 | Gateway 已有 sticky affinity 和精确跨账户授权；未经核对不能照搬上游的故障转移范围 | 暂不新增配置或放宽跨账户回退 |

截断检测参考上游提交
[`4f2e52709dcdb1116c45172d5407c7f626997648`](https://github.com/router-for-me/CLIProxyAPI/commit/4f2e52709dcdb1116c45172d5407c7f626997648)。
上游对不同输出协议采用不同兼容规则；本轮不声称复制其所有 executor 行为。

## 本轮验收合同

1. Responses 必须收到有效 `response.completed` 或 `response.incomplete`；后者保持
   incomplete/length 语义，不等于网络截断。单独 `[DONE]`、文本 delta、工具 item done
   和 HTTP EOF 都不能证明完整响应结束。
2. 成功的原生流保留原始字节、chunk、usage 和未知字段。跨 chunk、CRLF、多行 data
   用既有有界 SSE decoder 处理，不引入全响应缓存或无界队列。
3. 缺终止或明确失败产生一次脱敏领域错误；不补成功 stop/DONE，不把私密错误正文
   放入本地错误。保持原 transport error 对象及 source chain。
4. 取消/Drop 不合成截断错误；错误、EOF、取消释放该层 transport/buffer，不启动任务。
   SSE 已移交后不换账号或重发。HTTP body 错误沿现有 typed error 边界传播，
   不假称已发送 HTTP 200 后还能改成 502。
5. 仅按实际 outgoing Responses wire 接入原生检查；Responses 入站但实际上游 Chat
   仍保持现有桥接行为。转换器原有的 legacy Chat passthrough 不在本轮严格化范围。
6. 先运行可失败回归，再做窄改动；验证完整 Responses 单元、真实 loopback Pipeline
   合同、邻近预算/取消合同、formatter、编译、checker、ratchet、strict 和 scoped diff。
   不以源码检查代替新 Windows 包或生产验收。

转换器沿用收到有效 response-level terminal 即输出终止并完成的语义，不为检查
HTTP trailer 额外等待；不声称观察 terminal 后的新 chunk。原生透传则沿既有 EOF
消费语义持续读取。两者都不能在没有有效 terminal 时因 HTTP EOF 而报告成功。

保留已有工具响应 reader 的兼容：`response.completed` 搭配非标准 `status: tool_calls`
仅在 output 确有 function_call/custom_tool_call 时接受；空 output 或失败状态仍拒绝。
这是现有工具流单元合同发现的兼容边界，不将其用于新 Responses 输出构造。

## 状态

本批 Responses 源码实现、独立只读复核及 Windows 本地聚焦回归已完成。
这不是全部 CPA 特性对照/移植完成，也不改变 Gateway 全项目发布与人工验收边界。

新增 `terminal.rs` 统一完成证明，`completeness.rs` 处理原生流的透明检查；
转换器复用既有 decoder，不叠加第二层全流 buffer。工具 wire builder 独立为
`to_openai_chat_tools.rs`，避免给原 500 行转换器增加软例外。

| 权威 effective lines | 修改前 | 修改后 |
| --- | ---: | ---: |
| to_openai_chat.rs | 500 | 455 |
| accumulate.rs | 325 | 294 |
| stream_preflight.rs | 158 | 168 |
| terminal.rs / completeness.rs / to_openai_chat_tools.rs | 新增 | 109 / 80 / 49 |

### 验证回执

先运行旧实现上的新合同测试：7 项中 5 项失败、2 项通过，实际复现 EOF 假成功、
工具项误当完成和显式错误没有进入 typed error 的问题。完整单元回归随后发现既有
`tool_calls` reader 兼容；没有删除原工具流断言，而是增加受约束兼容及三路径回归。

最终命令使用 `--locked --offline`、`--test-threads=1`，临时目录均指定到下方证据目录。
`GATEWAY_PREBUILT_WEB_UI=1` 使用通过 ready marker 校验的既有 UI 资产快照；本轮未改 UI。

| 最终检查 | 结果 | 证据文件 |
| --- | --- | --- |
| 7 个 integration targets：Responses 完整性、native 完整性、pipeline_send_runtime、typed SSE、protocol bridge、request budget、retry wait budget | 69 passed | green-integration-v2.log |
| cargo test --lib protocol::responses | 49 passed | responses-unit-v2.log |
| cargo test --lib protocol::stream_decode | 9 passed | decoder-unit.log |
| cargo test --lib http::sse | 5 passed | http-sse-unit.log |
| cargo test --lib upstream::stream | 29 passed | upstream-stream-unit.log |
| npm run test:effective-lines --prefix scripts | 31 passed | checker-tests.log |
| npm run check:effective-lines --prefix scripts | passed | ratchet-final.log / ratchet-final.json |
| node scripts/effective-code-lines.mjs --mode strict | passed，governed source >700 = 0 | strict-final.log |
| cargo fmt --all -- --check / git diff --check | passed | format-final.log / diff-check-final.log |

相关 Rust 测试合计 161 项，无失败；不是全仓库所有测试或真实供应商验收。
编译保留既有 unused warnings，不修改 S06/Gemini 范围。已逐文件复核新增代码的
输入/secret 边界、内存上限、线性解析、取消/Drop 与回调清理；未新增后台任务、依赖、
重试授权、配置字段或存储迁移。

开始时保护的 11 个源码/私密配置文件最终 hash 一致；checker 原有 dirty 报告保留原字节。
没有提交、推送、真实供应商调用、正式发行打包、运行服务替换或 Linux/Docker 验收；
本轮改动不会自动进入此前交付的 Windows 包。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-20261019/`。
