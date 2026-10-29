# Mistral API 模板调整完成：文档与本地合同验收

## 验收范围

用户明确没有有效账号，批准按官方文档与本地合同测试验收。
本轮不访问真实推理接口、不导入真实密钥、不修改运行中的服务商数据。
这不是实际账户权限、余额、真实生成或 4300 端到端验收。

## 官方文档核对（2026-10-27 联网读取）

- https://docs.mistral.ai/api/endpoint/chat
  - `POST https://api.mistral.ai/v1/chat/completions`。
  - `Authorization: Bearer <API_KEY>`，JSON 请求包含 `model` 和 `messages`。
  - `stream=false` 返回完整 JSON；`stream=true` 返回 SSE，以 `data: [DONE]` 结束。
- https://docs.mistral.ai/api/endpoint/models
  - `GET https://api.mistral.ai/v1/models` 返回账号可见模型，使用 Bearer 鉴权。
  - 当前 Mistral 模板不接入专用官方目录读取器，正常导入采用手动模型列表；
    不会把 Mistral Key 交给 OpenAI 等其他服务商目录接口。

## 改动与实现边界

- 模板 ID、默认 Provider ID、preset 统一为 `mistral-api`，不注册旧 `mistral-openai` preset。
- 同步品牌、健康检查与协议/实现线选择映射；协议名 `mistral` 和凭据目录格式不变。
- 模板不预填模型；创建为空账号、空模型、空路由池。
- 自动测试关闭；用户主动测试默认仅选择官方文档示例 `mistral-large-latest`，
  不以目录可见性代表模型可用性，不批量调用目录中的模型。
- 现有请求构造器已对默认 endpoint 处理 Base URL 的 `/v1`，不重复拼接。
  现有默认鉴权就是 Bearer；未新增不必要的专用执行器或路径 override。
- 新增前端合同覆盖空池与手动账号导入；新增 Rust loopback 合同覆盖新 preset、
  官方 URL、真实 HTTP 请求的路径/鉴权/body、JSON 回复、SSE 消费至结束标记和 401 失败。
- 本地测试使用假密钥及 `127.0.0.1:0`，接收上限 32 KiB、8 秒超时，连接随测试结束关闭。

## 验证记录

- 修改前新增前端测试按预期失败：`mistral-api` 模板尚不存在。
- Rust `cargo test --locked --lib mistral_contract -j 4`：2/2 通过，
  其中 wire 测试包含非流式成功、流式成功以及两种模式的 401 失败，共四个本地 HTTP 场景。
- 前端模板、导入、品牌与目录对话框定向测试 23/23 通过，TypeScript typecheck 通过。
- `cargo test --locked --lib preset::tests::registry -j 4`：2/2 通过。
- effective-lines 检查器 31/31、ratchet 与 `git diff --check` 通过。
- 已在 `providerTemplateAdaptation.ts` 按 `documentation-and-local-tests` 登记完成；
  不以此声明真实账号、余额、生产网络或完整跨协议转发验收通过。
- 未编译或替换用户当前 EXE，未启动额外运行实例。
