# SAP AI Core：Orchestration v2 文本 SSE 与终态用量

## 实现

- 匿名核对 SAP 的 [v2 流式文档](https://github.com/SAP-docs/sap-artificial-intelligence/blob/5703d899f052edc8883758a6f7474b2089c389ef/docs/sap-ai-core/generativeAI/streaming-3340907.md)、[固定版 OpenAPI](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/packages/orchestration/src/spec/api.yaml) 与 SDK [成功](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/test-util/data/orchestration/orchestration-chat-completion-stream-chunks.txt)／[错误](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/test-util/data/orchestration/orchestration-chat-completion-stream-chunks-with-error.txt)夹具。全局流开关是 `config.stream.enabled`，用量选项位于模型 `params.stream_options`，成功流末尾有 `[DONE]`。
- 仅为显式绑定的 Orchestration 文本 Chat 打开 `stream:true`。固定部署、模型、资源组、OAuth、不重放及旧 SAP Chat／Responses 链路不变。
- 分段严格 SSE 解码和去歧义 JSON 解析；丢弃模块结果与空起始帧。只将 `final_result` 的文本增量与权威用量投影到 Chat，使用公开别名，不将 SAP 内部失败消息透出。复用已有 Chat 守卫：收到有效 finish、`[DONE]` 且源正常 EOF 才释放终态与用量；中断时不可发假终态。

## 验收状态

- **独立协议回放通过。** 临时本机 HTTP 模拟复用了仓库的实际 Orchestration 解码、投影与公共 Chat 终态守卫。固定版 SAP SDK 成功夹具只输出一个 288-token 用量帧、一个 `[DONE]` 和公开别名；错误夹具不输出中间内容或终态。删除 `[DONE]`、在 `[DONE]` 后追加数据、篡改总用量三个负例均未释放 finish／用量／`[DONE]`。这不是 Gateway 路由、结算、取消或打包验收，临时回放不作为后续开发依赖。
- **公共入口复跑通过。** 同一默认运行时构建的 SAP 回归为 **65 passed／0 failed／3 ignored**。Orchestration 文本用例实际覆盖终态用量、错误尾部、取消释放源、退款和不重放。早期共享 Quick OAuth 编译阻碍没有在该次运行复现；3 项要求隔离 Hosted PostgreSQL／Redis，未运行。
- 定向 Rust 格式、有效行 checker 31/31、ratchet、manifest 校验（56 项）和 `git diff --check` 通过。
- 未出包、未部署、未做 Hosted 验收、未使用真实 SAP 账号或额度。独立 feature、包与真实租户仍需分别验收；函数工具与 Bedrock 在对应回执中另记。

## 定向复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
```
