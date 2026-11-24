# SAP AI Core：原生 Bedrock Converse 客户端函数工具

## 范围与合同

- 继续使用[原生 Bedrock 文本切片](2026-11-22-sap-bedrock-converse.md)的显式部署及 SAP Bearer／资源组，不改 Azure Chat/Responses 或 Orchestration，不增加凭据权限。
- 依据 AWS [ToolSpecification](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolSpecification.html)、[ToolChoice](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolChoice.html)、[ToolUseBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolUseBlock.html) 和 [ToolResultBlock](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ToolResultBlock.html)，映射 schema、显式 `strict`、自动／强制／指定选择，并保留原始 `toolUseId`、参数和工具结果。`strict`／指定函数是否可用仍由上游模型决定；Gateway 不执行 JSON Schema 的实例校验或实际函数。
- 仅接受客户端函数。无声明的函数、重复调用 ID、未知／重复／缺失结果、空参数、非对象参数和重名参数键均拒绝，不被共享编码器改写成 `{}`。不转换 `tool_choice:none`、不接收任意额外原生字段。保留字面 HTML 实体，不撤销无关的 canonical 策略修改。
- 公共 JSON/SSE 使用 Gateway 模型别名；流只发布文本、工具身份/参数片段与公开终态。Chrome/Edge 不参与调用；没有内部 relay、reasoning continuation 或 SAP 调试字段。终态必须匹配工具状态且等到 EventStream CRC 校验和干净 EOF。公开用量沿用标准 Chat 输出：未知缓存计数省略，已知缓存读计数映射至 `prompt_tokens_details`；不投影原生缓存写别名。

## 验证状态

- 基线完整 SAP 回归 **69 passed / 1 failed / 3 ignored**：文本 SSE 的未知缓存计数按 `null` 原生别名输出，被公共 Chat 终态守卫判为无效。修复以标准 Chat 用量投影代替原生别名，保留守卫与原生私有路径不变。
- 变更前已运行隔离准入红测：提取未改动的实际 `plan` 函数，用有效函数工具输入，结果为 `valid tool request rejected by existing SAP admission`（退出码 101）。这不是完整 Gateway 端到端测试。
- 完整 SAP 首轮为 **74 passed / 1 failed / 3 ignored**：JSON／SSE 两轮工具、文本 SSE 修复、OAuth 前拒绝、工具选择、取消退款均通过；一个负例测试重复使用真实熔断状态，后续用例未进入上游。改为每个负例使用独立 LocalRuntime 和合成上游后，最终完整 SAP 回归 **75 passed / 0 failed / 3 ignored**（退出码 0）。覆盖两轮真实公共路由、参数分片、拒绝错误 ID／参数／终态、准确用量、退款及不重放。三条 Hosted 夹具未执行。
- 共享 Bedrock、签名与本地 wire 回归 **31 passed / 0 failed**；独立 `cargo check`（`line-sap-aicore-chat`、lib/bins）通过。定向 `rustfmt`、有效行 checker **31/31**、ratchet、56 项 manifest、22 个文件 UTF-8 无 BOM、`git diff --check` 通过。
- 本切片不包含 Bedrock 部署发现、reasoning／媒体、新包、双 EXE、Hosted、多租户或实际 SAP 账号／额度验收。

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 bedrock -- --skip protocol::sap_aicore --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
```
