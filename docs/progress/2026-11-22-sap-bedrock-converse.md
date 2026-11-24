# SAP AI Core：原生 Bedrock Converse 文本切片

## 合同与范围

- 固定版 [SAP 官方文档](https://github.com/SAP-docs/sap-artificial-intelligence/blob/5703d899f052edc8883758a6f7474b2089c389ef/docs/sap-ai-core/generativeAI/aws-bedrock-6a81f85.md) 要求在 `foundation-models` 场景使用 `aws-bedrock` 部署。调用 `<deploymentUrl>/converse` 或 `<deploymentUrl>/converse-stream`，认证为 SAP Bearer 和 `AI-Resource-Group`。没有 AWS `/model/{id}`、SigV4 或 Azure `api-version`。
- 固定版 [SAP Python SDK](https://github.com/SAP/ai-sdk-python/blob/898fbd1e7a36ba422f5a32e2377df5ecf7b43a98/packages/gen/gen_ai_hub/proxy/native/amazon/clients.py#L46-L66) 保留原生 Bedrock Runtime 客户端的响应解析，流请求使用 `application/vnd.amazon.eventstream`。Gateway 按此格式接入；真实租户 MIME/响应仍需验收。
- 账号池显式选择 Bedrock，使用 `extra_body: {sap_bedrock_converse: true}`，不混用 Azure 或 Orchestration 设置。控制台仅手填 Bedrock 部署 URL，隐藏 Azure 目录；凭据覆盖不能改变此模式、部署或资源组，Keep 识别协议更换。
- 当前支持公共 `/v1/chat/completions` 的文本 JSON／SSE 与专属文本探测。共用 Converse 编码器映射 `inferenceConfig`，不会发送公共别名或任意原生扩展。EventStream 校验 CRC、消息顺序和终态用量；干净 EOF 后才发布终态，不发布内部 relay/continuation。工具、reasoning、图片、原生 `/model/{id}/converse` 入站及公共 Messages／Responses 均不在此切片；不会静默转成 Azure Chat。

## 验证状态

本文件保留文本切片的首次证据；之后的函数工具切片和流终态修复见[函数工具回执](2026-11-22-sap-bedrock-converse-tools.md)。

- 控制台新用例先因缺少 Bedrock 功能入口失败；修复后两组 SAP 前端套件 **19 passed**，`tsc --noEmit` 通过。
- 有效行检查器 **31/31**、ratchet、56 项 manifest 与 `git diff --check` 通过。
- 后端用例覆盖 Local 公共预付 Key、两个真实路由、JSON／流用量、OAuth 缓存、失败退款、不重放，以及凭据合并和 Keep 绑定。首次完整回归 **69 passed / 1 failed / 3 ignored**。文本 JSON 发送正常，但文本流投影输出空缓存计数，触发公共 Chat 终态校验失败。独立构建修复 `TryStreamExt` 导入后已通过；流式修复与最终复跑见[函数工具回执](2026-11-22-sap-bedrock-converse-tools.md)。
- 没有出新 devtest 包、运行 Hosted 双 EXE、改动实际服务或使用真实 SAP 账号／额度。

## 定向复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
```
