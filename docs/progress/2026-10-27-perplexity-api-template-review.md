# perplexity-api 模板复核

## 范围与验收依据

用户没有有效 API Key，授权使用占位 Key 核对官方接口。验收依据为官方文档、本地表单与协议合同测试，以及真实接口的鉴权拒绝；不是有效账号或真实生成验收。本轮不编译桌面交付包，不操作用户的 4200 实例，不向真实账号库写入假账号，也不启动 4300 实例。

2026-10-27 联网核对官方文档：

- https://docs.perplexity.ai/docs/agent-api/migrate-from-sonar/overview
- Sonar Chat Completions 已于 2026-09-27 结束支持，但同步与流式请求仍由官方兼容处理。
- 文档的 Sonar HTTP 示例为 `POST https://api.perplexity.ai/v1/sonar`，Bearer 鉴权，`model`、`messages` 请求以及 `choices` 响应。
- 新项目推荐 Agent API；其 `/v1/agent` 接收 `input` 并返回 `output`，不能与 Chat 请求格式混用。本轮仅修复现有 Sonar 兼容模板，不宣称完成 Agent API 接入。

## 改动

- 模板 ID、默认服务商 ID、中英文名称、preset 统一为 `perplexity-api`；旧 `perplexity` preset 不再注册。
- 默认 base URL 保持 `https://api.perplexity.ai`，Chat 路径修正为 `/v1/sonar`，移除未经确认的 `/v1/responses` 声明。
- 创建空凭据池、空模型列表与空路由，不写入 API Key；自动账号测试关闭，主动测试仅选择 `sonar`。
- 更新品牌、站点健康、协议映射与配置示例。独立 `perplexity-search` 产品不变。
- 新增表单合同测试和有界 loopback wire 测试，覆盖 Bearer、URL、JSON、SSE、401；仅使用假凭据。

## 真实接口证据

脚本与原始结果：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-perplexity-template-review-20261027/official-probe.py`、`official-probe.json`。

仅对文档端点、`sonar` 模型发送两次请求，分别 `stream=false` 和 `stream=true`；没有模型遍历、重试或其他账号调用。
两次均返回 HTTP 401、`application/json`、`error.type=invalid_api_key`、`error.code=401`。
这证实官方接口可达并能正常拒绝无效凭据；不证明付费权限、余额、成功生成或真实 SSE 内容。

## 验证记录

- 前端新增测试先红：旧模板下找不到 `perplexity-api`；修改后相关 3 个文件 18 项测试通过。
- TypeScript typecheck 通过。
- effective-lines checker 测试 31/31、ratchet 通过。
- Rust 首轮 9/10：旧 YAML 测试仍引用 `perplexity`，修正测试和配置示例后最终重跑 10/10 通过，包含独立 Search 产品的相邻回归。
- 同一已通过的 Rust 测试二进制追加执行 preset registry 测试，2/2 通过。登记前端完成标记后 Cargo 因嵌入资源变化要求重新编译，已取消这次冗余构建；未取消其他线程进程，也未将取消的命令记为通过。
- 相关 Rust 文件 rustfmt 检查通过；`git diff --check` 通过（存在其他文件的换行符提示）。
- 验证通过后登记 `documentation-and-local-tests` 完成标记，不将其解释为真实账号可用或生成成功。
- 原生 UI、4300 端到端与有效 Key 的真实生成未验证，不用表单组件测试替代这些结论。

本轮涉及的既有核心文件有效行数：providerCatalog 423→423、providerCatalogWorkflow 118→118、preset/openai 205→205、protocol/registry/profiles 222→222、upstream/client 439→441。新增前端测试 34 行、Rust 合同测试 125 行，均低于 500。保留其他线程全部已有改动，没有修改依赖、持久化数据库或真实密钥。
