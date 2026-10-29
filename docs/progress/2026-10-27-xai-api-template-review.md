# xAI API 模板调整完成：文档与本地合同验收

## 验收口径

沿用用户批准的文档与本地合同测试流程，不以真实生成作为完成前置。
用户随后提供可能可用的 Key；只对官方模型目录做只读请求，不遍历模型调用。

## 官方文档（2026-10-27 联网核对）

- https://docs.x.ai/developers/rest-api-reference/inference
  - 官方推理域名 `https://api.x.ai`，使用 `Authorization: Bearer <API key>`。
- https://docs.x.ai/developers/rest-api-reference/inference/chat-completions
  - 仍支持 OpenAI-compatible `POST /v1/chat/completions`，输入 `model`、`messages`。
- https://docs.x.ai/developers/rest-api-reference/inference/responses
  - 新接入推荐 `POST /v1/responses`，输入 `model`、`input`；不是 Chat 请求字段的直接混用。
- https://docs.x.ai/developers/model-capabilities/text/streaming
  - `stream=true` 使用 SSE；Chat 示例以 `data: [DONE]` 结束。
- https://docs.x.ai/developers/rest-api-reference/inference/models
  - `GET /v1/models` 返回 Key 可见目录，不证明额度或所有模型均可生成。

## 修改内容与边界

- 模板 ID、默认服务商 ID、preset 统一为 `xai-api`，旧 `xai-openai` preset 不再注册。
- 后续按用户指定大小写，模板显示名及新建服务商默认 ID 调整为 `xAI-api`；
  内部模板 ID 与 preset 仍为 `xai-api`，避免显示大小写变化影响底层协议选择。
- 同步品牌、健康检查、协议及实现线映射；内部协议名与凭据目录格式保持原状。
- Base URL 为 `https://api.x.ai/v1`；创建空账号、空模型、空路由池。
- 自动测试关闭，主动测试限定一个当前文档示例模型 `grok-4.7`，不自动调用整个目录。
- 账号通过正常 UI 手动填写模型列表；当前不新增专用 xAI 官方目录读取器，
  不将 Key 发往 OpenAI 等其他目录地址。
- 复用现有 OpenAI-compatible 请求执行器，默认路径去重避免 `/v1/v1`。
- 本地合同使用假密钥和 `127.0.0.1:0`，每次接收限制 32 KiB / 8 秒，覆盖
  Chat 与 Responses 的 URL、Bearer 鉴权、请求字段、JSON/SSE 回复及鉴权错误。
- SSE 测试消费整个 HTTP 响应并与 fixture 比对；不等同于完整客户端跨协议 SSE 验收。

## 真实账号只读结果

- 对 `https://api.x.ai/v1/models` 请求返回 HTTP 403。
- 为读取错误详情补做一次相同只读请求，仍为 HTTP 403，`code=permission-denied`。
- 上游错误明确为团队额度耗尽或达到月度消费上限；未声称能够区分这两个原因。
- 未获取模型目录，未发起生成请求，未导入运行账号，Key 未写入源码、报告或测试夹具。

## 本地验证

- 修改前新增前端测试按预期失败：`xai-api` 模板不存在。
- 模板、手动账号导入及相邻前端测试 25/25 通过，TypeScript typecheck 通过。
- `cargo test --locked --lib xai -j 4`：4/4 通过，含协议映射、API 产品健康检查、
  URL 合同与 wire 合同。wire 合同覆盖两种 API × 两种返回模式 × 成功/401，共八个本地 HTTP 场景。
- 使用本轮编译出的测试二进制执行 preset 注册回归：2/2 通过。
- effective-lines 检查器 31/31、ratchet、Rust 新测试格式检查和 `git diff --check` 通过。
- 按 `documentation-and-local-tests` 登记完成；真实账号仍受额度限制，未宣称真实生成成功。
- 本轮未编译新 EXE，未修改当前运行实例的数据。
