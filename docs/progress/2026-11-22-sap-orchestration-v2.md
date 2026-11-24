# SAP AI Core：Orchestration v2 非流式文本 Chat

## 官方合同与实现

2026-11-22 已匿名联网核对 SAP 固定版本的 [Harmonized API 请求和回复](https://github.com/SAP-docs/sap-artificial-intelligence/blob/5703d899f052edc8883758a6f7474b2089c389ef/docs/sap-ai-core/generativeAI/consume-models-with-the-harmonized-api-2392d9a.md)、[Orchestration OpenAPI](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/packages/orchestration/src/spec/api.yaml) 与 [Bedrock 部署合同](https://github.com/SAP-docs/sap-artificial-intelligence/blob/5703d899f052edc8883758a6f7474b2089c389ef/docs/sap-ai-core/generativeAI/aws-bedrock-6a81f85.md)。OpenAPI 明确允许无 `prompt` 的 `messages_history` 请求，响应有 `final_result`。Bedrock 部署的 `/invoke`、`/converse` 与各自流式接口另有线协议，不用此实现冒充原生 Bedrock。

- 增加独立的部署模式，固定 SAP 模型名而不复用 Azure Chat 日期版本；凭据不能把模式转移至其他部署或资源组。复用 SAP 认证、超时、固定 HTTPS origin 和禁止重放的 sender。控制台可手填部署；仍通过账号对话框独立保存 Service Key。
- 复用公开 Chat API、路由模型别名和用量归属。仅发字符串文本历史及单个可选输出 token 上限。原生工作流配置、流、工具、媒体和其他公共端点不在本切片，在 OAuth 前拒绝。只接受 `final_result`，不能用中间结果补成功。

## 验证状态

- 控制台红测复现缺少 Orchestration 模式；修复后 SAP 表单 11/11、TypeScript、定向 Rust 格式、有效行 checker 31/31、ratchet、UTF-8 无 BOM 和 `git diff --check` 通过。初次类型检查指出新增测试读取 `unknown` 的断言问题，已修正并复验。图片手测包早于本功能，不包含 Orchestration。
- SAP 定向回归 **59 passed／0 failed／3 ignored**（隔离 Hosted 用例未运行），独立 `line-sap-aicore-chat` 的 lib／bins 编译检查通过；共享 OpenAI／Responses **143/143**。三个新增 Local 合成公共用例覆盖固定模型和原样文本历史、恰好一次 OAuth／发送、输出别名和 5 tokens 的审计；不支持的参数和入口在 OAuth 前拒绝；只有中间结果时返回失败、不重放并恢复本地预付。
- 未给 Orchestration 重新出包，也未做 Orchestration 的原生桌面操作、Hosted 双实例、真实租户调用或付费生成。未部署。下一开发切片可先补原生 Orchestration SSE 及终态用量，再推进函数工具和原生 Bedrock。

## 定向复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
npm.cmd run test --prefix apps/desktop -- --run src/features/console/SapAiCoreCatalogTemplate.test.tsx
npm.cmd run typecheck --prefix apps/desktop
```
