# Copilot 公共请求准入适配

本块新增 `src/protocol/copilot`，供后续可信 attempt gate 调用；目前没有公共调用点，
没有解除账号禁路由，不代表 Chat Completions／Responses 端到端接通，不增加整卡完成数。

## 契约

- 仅接收 OpenAI Chat Completions／Responses 的非流式单轮 user 纯文本。
  Chat 支持字符串或单个 text part；Responses 支持字符串 input 或单个 user message，
  内容为字符串或单个 input_text part。
- 同时检查 canonical 状态和原始 JSON 顶层／message／content part 白名单。
  即使通用 normalizer 丢弃未知字段、null 工具或不支持的参数，也明确拒绝，而非静默降级。
- 本切片不接受 metadata、历史、工具、媒体、推理配置、session ID、流式及生成参数。
  这不是声称 Copilot 永远不能支持它们；后续扩展必须有可保真的执行与协议证据。
- prompt 最多 64 KiB，模型最多 512 bytes，拒绝空白。返回借用的过滤后 canonical 文本，
  不重新从 raw body 取回未过滤内容；模型使用路由选择结果，允许公共 alias 与其不同。
- 返回固定安全 400 错误、不可重试及 Abort fallback，不回显 prompt 或模型。
  此处没有进程、凭据、队列、网络或存储副作用。

## 验证

定向测试调用真实两个 normalizer，覆盖合法文本形态、过滤后文本、模型别名、原始嵌套
未知字段、null／畸形选项、历史／工具／媒体、canonical 独立变更和字节边界。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-protocol-20261113`。
最终 `integration-r2.log`：**5 passed / 0 failed / 0 ignored**，真实根库编译及协议集成测试通过。
命令：`cargo test --locked --offline --test copilot_request_contract --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2`。
本模块不依赖 SDK feature；本轮未重复运行此前通过的 SDK child 测试。

首次命令等待另一项已有构建的 Cargo 锁后主动取消本轮等待者，改用该 target 既有
runtime-defaults／debug=0 配置。根库单元测试随后被并行 Perplexity 测试的 `values`／
`wait_closed` 不可见错误阻断，改用标准集成入口，未改动另一工作线。测试搬移时遗漏
`serde_json::Value` import 导致第一次集成编译失败，补齐后上述最终命令通过。
没有终止其他构建或删除锁文件；最终命令耗时含锁等待和依赖重编译，不是用例运行时长。

官方 rustfmt、checker 31 项、ratchet、UTF-8 无 BOM 与 scoped diff 检查通过。

有效行：protocol facade 117 → 118；新准入模块 120，集成测试 `tests/copilot_request_contract.rs` 143。未改 baseline 或依赖。
后续仍需可信身份／revision、生命周期、发送防重放、未知用量及公共集成测试，不能直接启用路由。
