# Vertex Express Preview 工具参数分片子批

日期：2026-11-05。范围：R11/W07 的 Express 响应和显式请求开关；不是完整 decoder、整卡 `offline_ready`、P50 或总目标完成。新增模板累计仍为 14。

## 改了哪些内容

- Express 独立 `FunctionStream` 将同一次调用的 `partialArgs` 组装成一个完整 object 参数调用，再复用既有 projection 和公共 ID 分配。调用级和字符串参数级 `willContinue` 分开维护；支持首片 name-only、后续省略 name、空字符串末片、只有路径的字符串末片和空 `functionCall: {}` 调用结束片。两个顺序完成的同名同参数调用仍是两个公共 ID，不按 name 合并。
- 独立 JSONPath owner 支持 RFC 9535 的单位置成员路径：点号成员、单双引号成员及合法转义/Unicode、嵌套对象、非负连续数组索引。路径规范化后关联字符串，不因点号/引号写法不同误拆参数。深度最多 64；不求值 wildcard/filter/slice/descendant/union，不创建稀疏数组，也不猜负数索引或根对象的整体赋值。
- 非法 union/类型/flag、混合完整 args 与 partialArgs、身份冲突、重复完成叶子、父子类型冲突、未完成字符串/call、无真实 finish/坏尾部均不能输出可执行工具。保持静态 502、非重试 Abort，不因未知上游消费自动重放。过滤 candidate 仍整体丢弃先前内容，包括尚未闭合工具，保留报告 usage。
- 三个公共协议均可显式提交下述 boolean；缺省不启用，`false` 明确关闭，字符串/数字/null 在网络前 400。只从现有 lossless `raw_body`/extra 读取这个选项，不扩 Responses 通用未知字段白名单。显式开关与公共 `tool_choice` 共存时，恢复 canonical 的 mode/allowed names，修补 Chat/Messages 整个 toolConfig 覆盖选择语义的接缝。

```json
{
  "toolConfig": {
    "functionCallingConfig": {
      "streamFunctionCallArguments": true
    }
  }
}
```

该扩展只承诺此 boolean；工具选择仍使用各公共协议标准 `tool_choice`，不承诺 Responses 透传整个原生 toolConfig。没有开关时不顺带修复所有既存 toolConfig 覆盖行为。不按模型名自动开启或猜测支持；官方标注 Preview，v1/v1beta1 可用，但不支持 Gemini 3.1 Flash-Lite。上游实际模型资格仍待真实材料验收。

生产 owner：
- [参数流状态](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/function_stream.rs)
- [单位置路径构造](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/function_stream/argument_path.rs)
- [显式请求开关](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_history/options.rs)

## 公开合同

本轮联网只读核实，没有真实 Key/授权/生成/刷新：

- [Google function calling 指南](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/tools/function-calling#streaming_function_call_arguments)：v1/v1beta1、显式请求开关、两层 willContinue、path-only 字符串结束片、空调用结束片及顺序同名工具示例。
- [Google v1 tool.proto](https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1/tool.proto)：PartialArg 的四种 delta、RFC 9535 路径及 `$.foo.bar[0].data`。参数流不等同于仅 v1beta1 的 call ID 功能。
- [RFC 9535](https://www.rfc-editor.org/rfc/rfc9535.html)：成员/索引和转义语法；这里只实现明确的构造子集，不宣称完整查询引擎或所有 Google 服务端输出形式。
- [ProtoJSON](https://protobuf.dev/programming-guides/json/)：`google.protobuf.NullValue` 特殊表示为 JSON null；不根据 Python SDK 的 Literal 类型猜测 raw REST 应为字符串。

未知重赋值、混合完整/分片参数和空容器分片表示仍未获足够合同依据，本批明确拒绝。仅按调用级 continuation 的顺序归属组装，可识别的 name/ID 冲突拒绝；没有无 ID 同名多调用交错的额外身份依据，不承诺支持或识别所有这种越出顺序合同的输入。不以补空对象隐藏缺口，不复制第三方实现或新增依赖。

## 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-preview-20261105/`。

| 检查 | 结果和边界 |
| --- | --- |
| 生产未改红测 | 12 项：4 passed / 8 expected failed；复现 Preview 拒绝、tool_choice 丢失及非法开关被发送。原日志保留 |
| Express 定向 | `cargo test --locked --no-default-features --features runtime-defaults --lib --jobs 4 vertex_express -- --test-threads=1`：65 passed / 0 failed / 1 ignored；18 项本批新增；ignored 为既存 opt-in 浏览器 host |
| 相邻协议 | 同一绿色 Cargo 测试 EXE，源码输入及 binary hash 前后核实：Anthropic、Responses tests、canonical SSE、实际 Gemini facade 共 111 passed；与 Express 有两项重叠，不简单相加 |
| 三协议开关 | Chat/Messages/Responses × JSON/SSE × true/false，真实 loopback 请求同时保留 `ANY`/精确 allowed name 和 boolean；非 boolean 为 400、失败 audit、余额 2、上游零调用 |
| 三协议分片往返 | 正常管理 Key、生产 local bootstrap、真实 SQLite、非转发 loopback；两次同名分片调用形成独立公共 ID，参数完整，公共结果逆序回传后仍按 v1 聚合排序；usage=5，消息余额 2→1→0 |
| 三协议不完整/冲突 | JSON/SSE 均 502、无成功终帧/私有参数泄漏、失败 audit、余额仍 2、精确上游仅一次；不能据此推断上游未消费 |
| 三协议取消 | 聚合尚未完成时调用方 Drop，正常 reservation 1→2、cancelled audit、source Drop；不输出半成品工具 |
| 静态/来源 | Rustfmt、checker 31 项、ratchet、UTF-8 无 BOM、Git scoped diff 与编译输入审计见 closeout.json；没有新增规模例外 |

红测使用既有默认栈缓存；后续验证明确关闭 management-ui，不触发前端 build/publish/prune。所有 Cargo 串行、jobs=4，没有设置 RUST_MIN_STACK、添加线程栈或用新 task 规避问题。当前命令不是完整 feature-off 矩阵；前批“仅官方 Vertex 线”缺少 15 个共享 Gemini Web 符号的失败没有被本批覆盖或修复。

本轮按测试名去重为 **174 个不同 Rust 测试通过**。2923 项绿色编译输入均未漂移，137 个共享 Gemini 文件未变，编译输入变化没有越出本批 Rust owner。新增生产模块有效行分别为 153（状态机）、144（路径）、48（开关）；新增测试最大 223，修改文件最大 284，无软上限例外。逐文件计数和 SHA-256 以 closeout.json 为准。

## 安全、资源和未完成边界

外层依旧先按整次响应累计 64 MiB bounded reader 收集，再进行 SSE/JSON 准入和组装；不是只有单 frame 限额。新逻辑没有 task/thread/锁/进程，没有跨 await 状态。深度有界、数组只连续追加，字符串原位 append，不对每片重复制整个已累计参数；路径按规范化段关联，保留现有整次请求超时。没有独立无界流接收器或稀疏索引分配。

共享 Gemini encoder/parser/translator 内部未修改。公共 SSE 仍是完整聚合后的桥接，不是实时 token/参数流。没有修改 UI，本批不重复浏览器构建/手测；没有真实 Gateway EXE/OS 重启、Google 业务/账单、真实账号权益或新包交付。

继续开放：Express signed reasoning/thoughtSignature 输出回输、有序混合内容、多 candidate、聚合 SSE cache 细项、全部声明模态及未支持的 Preview 表示；Full Vertex OAuth 独立 client/context/project/location、登录/refresh/持久化；其余原定服务商、P50/P60/P70 和集中凭据录制/逐渠道权益验收。未读真实 `.ng`/历史凭据，未创建 Redis/Docker、修改在线实例、提交或推送。
