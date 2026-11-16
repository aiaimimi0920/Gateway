# Vertex Express 完整工具 part、过滤终态与 v1 工具历史子批

日期：2026-11-05。范围：R11/W07 的 Express 子链；总目标继续执行，不增加整卡 `offline_ready`，新增模板累计仍为 14。

## 改了哪些内容

1. Express 保留自己的有界 SSE/UTF-8/JSON/单 candidate 完成准入，最终改用独立 projection，
   不再通过共享 Gemini parser 去重工具。两个完全相同、没有上游 ID 的完整调用保留两个；
   缺 ID 时生成本轮内独立、跨轮不复用的公共 ID，显式 ID 保留，重复显式 ID 拒绝。
2. 完整调用要求有效 name 和 object 参数；坏 JSON 参数、字符串/数组/null、坏 ID、冲突 text/function part 不补 `{}`。
   合法无参数调用允许省略 args。Preview `partialArgs` / `willContinue` 尚未实现 assembler，当前明确 Abort，不重放未知消费。
3. 九类安全/屏蔽 finish 映射 `content_filter`，完整缓冲允许丢弃该 candidate 先前的文本和工具，保留实际报告 usage。
   `MALFORMED_FUNCTION_CALL`、`UNEXPECTED_TOOL_CALL`、`OTHER`、`IMAGE_OTHER`、`NO_IMAGE` 不转普通成功。
   Messages JSON/SSE 将 `content_filter` 输出为官方 `refusal`，不再冒充 `end_turn`；Chat/Responses 沿既有 filter/incomplete 合同。
4. Express 自有 history owner 在网络前关联公共 call ID，检验原 canonical 参数，阻止共享准备链的 `{}` 修复隐藏错误。
   固定 v1 不向上游发送 functionCall/functionResponse ID；同轮结果允许公共端反序到达，按原调用顺序聚合成一个 user Content。
   Responses 相邻 assistant 工具 items 合并为同一 model turn。ID 轮内唯一，跨轮只查当前 pending，不作全历史名称猜测。
   缺失/重复/未知/旧轮结果、名称冲突、未闭合轮、Raw function 注入均网络前拒绝；完整历史前后的普通内容保留。
   v1 functionResponse.response 要求 object；当前不将数组/null 结果暗中替换成空对象，非 object 结果网络前拒绝。
5. 正常管理创建/充值的公共 Key、生产 local bootstrap、SQLite audit/余额及精确 loopback 上游增加三协议 JSON/SSE 两轮工具回传。
   test-only server 增加最多 16 项顺序回复，保留原静态/取消夹具。Responses SSE 额外核对工具 added、参数 delta/done 与最终快照的 identity。

生产 owner：
- [Express projection](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/projection.rs)
- [Express history](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_history.rs)
- [最终发送前接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/request_plan_part_2.rs)

共享 Gemini encoder/parser/translator 内部没有修改。公共 canonical 类型未扩展，不把 tool raw 当作签名/思考响应承载能力。

## 公开合同与留证

本批只读联网，无真实 Key、OAuth、生成或刷新请求：
- [Function calling reference](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/models/function-calling)：
  functionCall/functionResponse ID 明确限 v1beta1 的 Gemini 3+；不将其无条件用于当前固定 v1。
- [Function calling](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/tools/function-calling)：
  同轮并行 functionResponse 放入同一 Content；v1 按 name 与顺序匹配，Preview 参数分片需要专门 assembler。
- [GenerateContentResponse](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse)：真实 finish 与 usage 合同。
- [Anthropic stop reasons](https://platform.claude.com/docs/en/build-with-claude/handling-stop-reasons)：`refusal` 与自然结束 `end_turn` 区分。

原始公开响应、写前快照、构建输入、各轮日志及最终审计位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-tool-decoder-20261105/`。

## 实际验证与过程失败

| 检查 | 结果与边界 |
| --- | --- |
| 初始工具/过滤红测 | 8 项：1 passed、7 expected failed；实际复现无 ID 去重、坏工具假成功、过滤泄漏及 Messages 错误终态 |
| 首轮实现 | 42 passed、0 failed、1 ignored；随后只读审查发现 v1 历史聚合测试不足，不据此宣布两轮完整 |
| v1 历史红测 | 1 expected failed；真实公共调用发出 2 个结果 Content，而合同要求 1 个，保留失败日志 |
| 最终默认栈 Express 定向 | 47 passed、0 failed、1 ignored；16 项本批新增测试；ignored 为需独立浏览器会话的既有 UI host |
| 直接依赖协议回归 | 当前绿色 Cargo 所产测试 EXE 直接运行 Anthropic、Responses 和 canonical SSE：126 passed；另 Gemini facade 1 passed。二进制 hash 和 2917 项源码输入前后均核对 |
| 三协议工具往返 | Chat/Messages/Responses × JSON/SSE；公共工具 ID 独立，工具 schema/choice 到达精确上游，公共结果逆序回传后按调用顺序聚合；两次调用各 usage=5，消息余额 2→1→0 |
| 三协议过滤/失败 | 过滤不输出先前工具/文本，usage=5 按已完成请求结算；坏生成/坏参数不发成功终帧、不重放，Gateway 余额不按成功扣除；不代表上游未消费 |
| 三协议非法历史 | JSON/SSE 均为 400、失败 audit、余额保持 2、上游零调用 |
| 关闭默认 feature，仅官方 Vertex 线 | **失败**：15 个未改共享 Gemini Web 符号缺失；与总计划 P37 已记录的 feature-off 边界一致。未做写前版本对照编译、不接管 S06 修复，不报全矩阵绿 |
| 关闭默认 feature，启用官方 Vertex + Gemini Web core | `cargo check --locked --lib --jobs 4 --no-default-features --features line-google-agent-platform-official,line-gemini-web-reverse` 通过；不替代上一失败组合 |
| 静态/规模 | Rustfmt、checker 31 项、最终 ratchet、Git diff/UTF-8 与来源审计见 closeout.json；新增/修改源码均不超过 500 有效行，无新增例外 |

所有 Cargo 操作串行，`--jobs 4`，没有设置 `RUST_MIN_STACK`。另一次拟执行协议回归的 Cargo 重编译由本代理主动中止，
其 `regression-anthropic` exit 101 明确为中止，不是测试失败或通过；随后在源码未变且 hash 核对的既有绿色 EXE 上完成上述回归。
最初多 filter 中 `protocol::gemini::api::` 没有匹配测试，未计作 Gemini 通过；实际 facade 另行运行 1 项并独立留证。
最终按实际测试名去重为 172 项 Rust 测试通过；2 项 Messages 新回归同时存在于 Express 与协议套件，已去除交叠。

## 规模、安全和资源审查

response owner 232→230，新增 projection 123、history 184；发送文件 479→479，仅替换一条准备接线。
Anthropic response 185→186、SSE translator 486→487；新增测试模块与夹具均低于 500，具体计数以 closeout.json 为准。
工具身份和结果使用线性遍历/HashMap/HashSet，不恢复共享 parser 的二次方去重；输入仍受现有请求/响应预算约束。
错误使用静态分类，不回显工具参数、Key 或上游诊断；历史拒绝在 `.send()` 前，响应不完整/非法生成保持 Abort。
新生产逻辑没有 task/thread/进程或锁跨 await。测试正常路径关闭 SQLite/freebuff/listener，Server Drop 保留取消兜底。
断言 panic 路径不等同于完整 Fixture 异步清理证明；不能拿 CloseProbe 代替全资源关闭验收。

## 未完成与下一接力

- Express 完整 decoder 仍缺 signed reasoning/thoughtSignature 的三协议输出/回输、有序混合内容、多 candidate、Preview 参数分片及其 assembler；聚合 SSE 的 cache 细项及全部模态也不由本批证明。
  当前的明确拒绝不是这些功能的完成；canonical 与公共输出承载能力仍需专门设计及协调。
- 本批是 router oneshot + 真实 loopback 上游/SQLite；未重新操作浏览器，不是原生 EXE/OS 重启、Google 实际工具调用、计费或权益验收。
- 跨轮复用同字符串 ID 的允许政策和每个 Responses output-item-done 恰好一次，尚无本批专项正向断言；不把静态审查当实测。
- Full Vertex OAuth 的独立 client/context/project/location、登录/refresh/持久化仍未完成；其余渠道以及 P50/P60/P70 继续按总计划推进。
- 未读历史凭据/真实 `.ng`，未调用真实服务商，未创建 Redis/Docker 容器，未出包/替换在线实例，未提交、推送、reset 或 clean。
