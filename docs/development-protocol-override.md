# 开发环境强制目标协议

用于验证客户端输入协议与上游目标协议之间的转换，不更改账号的发现结果、优先级配置或默认路由。

## 开启与请求

显式编译开发版本：

```powershell
cargo build --locked --features dev-protocol-override --bin gateway
```

使用正常 Gateway API Key 鉴权，额外发送请求头：

```text
x-gateway-debug-upstream-protocol: anthropic_messages
```

另一个常用目标为 `openai_chat`。同时接受 `openai_responses`、`openai_legacy_completions`、
`gemini_generate_content`、`cohere_chat`、`bedrock_converse`，但目标必须已被账号/路由支持。
另支持 `dashscope_text` 和 `dashscope_multimodal`。通用发现路由可执行 Chat、Responses、
Messages 和这两种 DashScope 表面；其余目标不会因调试开关被假装启用。

- 保留正常鉴权、权益、模型、负样本和有损转换限制。
- 强制目标不可用返回 `debug_upstream_protocol_unavailable`，不会退回其他协议。
- 输入协议不变，响应仍使用客户端原来的格式；调试头不转发给上游。
- 不发送该头时，仍优先同协议，其次使用已验证的可转换协议。
- 非流式审计中记录 `debugForcedUpstreamProtocolFamily`；最终目标仍可通过
  `selectedUpstreamTargetProtocolFamily` 核验。

## 发布边界

功能要求同时满足 `dev-protocol-override` feature 和 `debug_assertions`。
feature 不在默认构建中；标准 `--release` 构建关闭 `debug_assertions`，即使误带该 feature
也会拒绝调试头，返回 `debug_protocol_override_disabled`。正式发布工具保持标准 Release 配置，
不要把开发 EXE 当作公共服务发行物，也不要对发布 profile 开启 debug assertions。
开发环境只应绑定受控接口。该开关不允许指定任意 URL、账号或密钥。

## 参数与验收

跨协议转发将 Bedrock `inferenceConfig`、Gemini `generationConfig` 中标准长度、采样和停止字段
转换为目标协议字段；Responses/Chat 输出长度限制转 Messages 时使用必填 `max_tokens`。
旧版 Completions 入口也保留通用生成参数，避免 `max_tokens` 在归一化时丢失。
同协议原生转发保留原始格式。未承诺所有厂商私有生成选项或状态型工具语义都能转换。

```powershell
cargo test --locked --features dev-protocol-override --test protocol_target_matrix_contract
cargo test --locked --test protocol_target_matrix_contract
```

`protocol_target_live_contract` 为显式忽略的真实上游测试。需用户授权，再通过环境变量提供
`GATEWAY_HY_PROBE_KEY`、`GATEWAY_TEST_KEY` 和 `GATEWAY_MATRIX_OUTPUT`。
它使用隔离的内存路由和生产 HTTP handler，不修改运行中 4200 实例、账号配置或发现状态。
结果记录 HTTP 状态、目标、回答与耗时；协议成功不等于模型回答事实正确。
