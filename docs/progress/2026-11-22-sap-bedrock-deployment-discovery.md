# SAP AI Core：Bedrock 部署发现

## 改动

- 现有管理查询新增可选 `protocol: "bedrock"`，固定按 `foundation-models`／`aws-bedrock`／`RUNNING` 在当前资源组查询。未指定协议时原 Azure Chat 查询及模型白名单不变。保留管理与敏感信息授权、同源 URL、部署 ID、限时限量、无重定向和 `no-store`。
- 对照 [SAP 场景目录](https://help.sap.com/docs/ai-launchpad/sap-ai-launchpad/models-and-scenarios-in-generative-ai-hub-fef463b24bff4f44a33e98bb1e4f3148?locale=en-US)、[Deployment API](https://sap.github.io/ai-sdk/java-api/v2/com/sap/ai/sdk/core/client/DeploymentApi.html) 及 [SDK 的可选模型提取](https://github.com/SAP/ai-sdk-js/blob/3b791d54/packages/ai-api/src/utils/model.ts)。Bedrock 列表资格独立于模型元数据；界面按实际返回显示模型名、配置名或 ID。目录只返回候选，不确认 Converse 支持或其他模型能力。
- 控制台选择 Bedrock 协议后方可查询该目录；临时 Key 不进入草稿，显式选择才填入 URL，仍可手填。资源组和协议切换分别隔离查询状态。无推理调用、部署变更或积分消耗。

## 验证

- SAP 定向回归 **76 通过／0 失败／3 跳过**。经真实管理 router 与本机模拟 SAP，核对 Bedrock 查询的管理与敏感信息授权、资源组和 `GET` 查询参数、混合 executable／状态／场景及异源或 ID 不匹配的过滤、缺失和异常的可选元数据、配置 revision 不变。既有 Azure 查询含 EU20 与 X.509 用例仍通过。3 个跳过用例需另行准备隔离 Hosted PostgreSQL／Redis。
- 前端先复现新控制项缺失，再验证通过：4 个定向套件共 **44/44**。并行运行时三个用例触发默认 5 秒超时；改用单 worker／15 秒 CLI 预算复跑全套通过，未修改产品或测试的默认超时。
- 独立 `line-sap-aicore-chat` lib／bins 检查、TypeScript typecheck、定向 Rust 格式、56 项 Provider manifest、31 项有效行检查器测试及 ratchet 均通过。

## 复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
npm run typecheck --prefix apps/desktop
npm run test --prefix apps/desktop -- --run --maxWorkers 1 --testTimeout 15000 src/features/console/SapAiCoreCatalogTemplate.test.tsx src/features/console/useCredentialDialogEditor.sap.test.tsx src/features/console/ProviderCatalogDialog.test.tsx src/features/console/ConsoleGlobalDialogs.luma-agents.test.tsx
```

本切片未连接真实 SAP 租户，未制作或部署新包，也未进行原生窗口业务验收。
