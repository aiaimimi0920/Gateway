# Azure API 模板调整完成：文档与本地合同验收

## 用户批准的验收口径

2026-10-27 用户明确：无法提供标准测试账号，只需根据文档确认接入正确，
即可认为模板调整完成。因此本模板按 `documentation-and-local-tests` 登记完成，
不再以真实账号、部署或真实生成作为本次完成前置。
完成标记不表示真实 UI 模型探测、非流式生成或 SSE 生成成功。

## 已完成

- 标准模板显示名称与新服务商默认 ID 改为 `azure-api`。
- 按用户后续要求，内部模板 ID 和 runtime preset 也统一为 `azure-api`；
  不注册旧 `azure-openai` preset，也不保留旧 preset 查询别名。
- 同步更新协议选择、健康适配器与品牌映射；Azure 协议标识及凭据目录格式不改。
  本次未创建过 Azure 运行账号，不修改运行数据库或已有凭据。
- Azure 资源地址仍可编辑；创建服务商保持空账号、空模型、空路由。
- 定向测试覆盖手动导入资源部署名，不触发 OpenAI 官方目录读取或通用生成探测。
- 真实请求用户指定资源的 `/openai/v1/models`：HTTP 200，返回 473 项目录模型。
  该响应不是已部署模型列表，未将这些 ID 导入账号，也未发起生成请求。

## 调用合同

当前 `src/preset/openai.rs` 已使用 Azure `api-key` 鉴权及相对
`/chat/completions`、`/responses` 路径，可组合 `/openai/v1` Base URL。
项目管理终点与模型推理终点分开使用，不将 `/api/projects/...` 当作模型 Base URL。

已联网读取 Microsoft 官方文档，HTTP 200：
https://learn.microsoft.com/en-us/azure/ai-foundry/openai/api-version-lifecycle?view=foundry-classic

文档要求至少一个 model deployment，调用中的 `model` 应替换为部署名称。
根据官方项目 API 文档，使用用户提供的项目终点读取
`/deployments?api-version=v1`，真实返回 HTTP 200、`{"value":[]}`。
当前项目未列出已部署模型；公共模型目录不能证明资源已部署同名模型。
参考：https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/aiproject

## 完成登记与未验证边界

- 按用户明确批准登记模板完成，不再等待用户提供 Deployment name。
- 将来进行真实测试时，必须将测试策略设为 `selected`，仅该部署，自动测试关闭。
  当前 Azure 模板继承 `all` 默认模型范围，不能据此直接测试整个已声明目录。
- 空池创建与账号导入通过本地 UI 合同测试；未在 4300 执行真实账号 UI、非流式或流式生成验收。
- `providerTemplateAdaptation.ts` 明确记录文档验收依据，与真实生成证据区分。
- 未修改运行中的 4200 数据；按后续要求另行构建桌面 devtest 包，不覆盖旧包。
- Key 仅存放于 linshi 私有证据子目录，未加入源码或报告。

## 本地验证

- 模板、空池创建、账号导入、完成标记和品牌标签定向前端测试 23/23 通过。
- TypeScript typecheck 通过；effective-lines checker 31/31、ratchet 通过。
- `git diff --check` 通过；新增 Rust 合同覆盖新 preset 注册、旧 preset 移除、
  API Key 鉴权、v1 相对路径及协议映射；未改动请求执行器。
- `cargo test --locked --lib protocol::azure_openai::tests -j 4`：2/2 通过。
- 证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-azure-api-template-acceptance-20261027/`
  - `official-models.json`：真实目录响应。
  - `official-api-lifecycle.html`：Microsoft 文档快照。
  - `official-deployments.json`：项目部署列表真实响应。
