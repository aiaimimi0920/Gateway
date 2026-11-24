# SAP AI Core：Orchestration v2 原生函数工具

## 合同和改动

- 核对 SAP 官方 [Tool Calling](https://github.com/SAP-docs/sap-artificial-intelligence/blob/main/docs/sap-ai-core/generativeAI/tool-calling-2e47871.md) 的两轮例子与 [固定版 OpenAPI](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/packages/orchestration/src/spec/api.yaml)。使用明确定义的 `config.modules.prompt_templating.prompt.tools`，模板消息追加在历史之后。末条消息使用 Gateway 固定占位符，调用方内容作为 `placeholder_values`；不会把调用方文本当模板。纯文本请求保持原来的无模板线路。
- Chat JSON／SSE 接通原生函数定义、助手函数调用与带匹配 ID 的文本工具结果。保留参数字符串、历史顺序、部署绑定及资源组；Gateway 不执行函数。未声明工具时禁止上游新增工具调用；不接入 `tool_choice`、`parallel_tool_calls`、自定义／托管工具、媒体或其他 Orchestration 模块。
- 只投影 `final_result`；片段完整性及终态沿用共享 Chat 守卫，缺身份、不一致结束原因、截断不发完成标记或用量。复用已有 OAuth、固定上游、脱敏和不重放边界。

## 验证

- 本机独立协议红测：原请求规划拒绝合法第二轮工具结果。修复后，使用仓库实际规划器及真实本机 HTTP 流回放，验证原样参数（含字面实体）、固定占位符、模型别名、工具片段、终态和一次 5-token 计数；非流式结构与未声明工具的拒绝亦通过；缺身份、截断和错误 finish 均未释放终态。旧 SAP SDK 成功／失败夹具与三个坏尾也通过。该回放不覆盖 Gateway 路由、退款或打包。
- 仓库公共测试覆盖 JSON／SSE 两轮、复用 OAuth、两次 5-token 审计、错误前零 OAuth、流中失败退款和不重放。早期公共复跑因共享 Quick OAuth 的 `E0063`／`E0061` 未进入测试；后续同一命令实际完成，**65 passed／0 failed／3 ignored**。其中 Orchestration 文本与工具用例全通过；3 个忽略项要求隔离 Hosted PostgreSQL／Redis。本回执不包括独立 feature 构建、打包或真实租户。
- 定向 Rust 格式、有效行检查器 31/31、ratchet、56 项 manifest、UTF-8 无 BOM 和 `git diff --check` 通过；本次新增及触及的 Rust 文件均在 276 有效行以内。
- 未使用真实 SAP Service Key、租户或额度；未出包或部署。

## Server Hosted 验收

- 两个独立 Server AppState 使用空路由文件和同一隔离 PostgreSQL／Redis。写入实例通过管理 API 保存账号、凭据和项目别名；读取实例仅以公共 Key 成功完成 JSON 两轮和 SSE 两轮（请求函数、回传结果）。凭据试图覆盖固定模型，实际四次请求仍使用账号指定的 `gpt-4.1-mini`。
- 本机模拟 SAP 收到一次 OAuth 和四次 `/v2/inference/deployments/deployment1/v2/completion` 请求；验证了资源组、短期 Bearer、函数定义、结果占位符，未透传调用方 Cookie 或管理凭据。JSON／SSE 保留公共别名和函数参数，SSE 单次终态和 5-token 用量；中间模块不回显。无效公共 Key 和不支持的 `tool_choice` 均在 OAuth 前拒绝。
- PostgreSQL 读回 **4 条精确归属的完成审计**，每条为 1 次尝试、2／3／5 tokens，共 20；另有 1 条无生成用量的失败审计。常规 SAP **79 通过、0 失败**，隔离 Hosted 用同一测试 EXE **5/5**；`cargo check --tests`、Rust 格式、有效行测试 **31/31** 和 ratchet 通过。仅增加测试；隔离容器已停止，保留数据。不是包内双 EXE、真实 SAP 或现金扣费验收。

复跑沿用 [Hosted X.509 隔离环境说明](2026-11-22-sap-hosted-x509.md#复跑)，将 Hosted 测试过滤词换成 `sap_aicore_server_hosted_orchestration_v2_tools`，或对同一 EXE 使用 `sap_aicore_server_hosted --ignored --test-threads=1` 运行整个 Hosted SAP 组。

## 定向复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
```
