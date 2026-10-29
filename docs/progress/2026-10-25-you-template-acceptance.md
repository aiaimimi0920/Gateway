# You 标准模板：接入开发与首轮真实验收

> 后续更正：本轮导入材料经复核是 `${YOU_API_KEY}` 占位符，不是真实密钥。
> 导入前后一致不能证明材料有效；403 不能据此归因于真实账号权限。
> 用户提供真实密钥后，独立凭据重新导入及普通搜索验收已通过，见
> [重新导入验收记录](2026-10-25-you-key-reimport-acceptance.md)。以下保留首轮当时的失败证据。

日期：2026-10-25（Asia/Shanghai）。**接入代码和定向检查完成；真实搜索验收未通过，
You 不登记 adaptation receipt、不标蓝。** 现有唯一凭据被官方搜索端点以 403 拒绝，
尚未拿到任何成功搜索结果，不能宣称本轮移植成功。

## 隔离、备份与实际 UI 流程

- API devtest 后端为 `127.0.0.1:4300`，真实源码前端为
  `http://127.0.0.1:1435/ui/`；未操作、停止或重启 4200 实例。
- 独立数据目录：`C:/Users/vmjcv/.ng-devtest-4300-you-20261025`。
  初始 `r0-ffe56ca6e895` 无服务商、无凭据、无审计记录。
- 原 You 服务商和一份凭据先备份到
  `C:/Users/vmjcv/.ng/backups/you-template-20261025/providers.json`；目录 ACL
  仅当前用户及 SYSTEM。凭据内容不进入源码、公开证据或截图；后续确认保存值是占位符。
- 实际点击“添加服务商 → 标准模板 → you → 添加”，得到
  `r1-dd5402888014`，确认 `explicit_credentials_only: true`、`credentials: []`。
- 展开账号库，点击“手动录入账号”，保留 `you-cred-0` 和原账号名称导入密钥，
  声明 `you-search`；得到 `r2-566534d80ace`。导入后回读密钥与备份完全一致。
- 从该账号点击“测试账号 → 运行测试”，执行模板继承的“搜索连通”用例。
  创建服务商、导入账号和运行测试均通过 UI，未直接改写数据库或路由文件。

## 官方合同和三适配器

当前官方文档已联网核实：

- <https://you.com/docs/api-reference/search/v1-search>
- <https://you.com/docs/using-the-api/authentication>

固定 Base URL 为 `https://ydc-index.io`，请求为 `POST /v1/search`，
使用 `X-API-Key`，JSON 中为 `query` 与小数量 `count`。不启用额外抓取或提取功能。
官方说明 GET 仍兼容，但新功能使用 POST；本轮不把搜索服务伪装成 Chat、Responses 或 SSE。

| 维度 | UI 与运行合同 |
| --- | --- |
| 调用适配器 | 搜索 API，`search_api_compatible` |
| 指纹适配器 | 自动，复用标准 API 身份层 |
| 专用适配器 | 无（标准 API），`direct_http` |
| 保存策略 | `call_flow: auto/auto/auto`，继承 You preset |
| 站点健康 | 只读 `you-status`、`https://status.you.com/`；默认关闭，未实际轮询 |

原有 UI 提示词测试拒绝所有非 OpenAI 文本适配器，You 会返回 Unsupported。
本轮补充 You 专用 probe：复用三适配器 prepare、统一 planner、指纹 headers 和
profiled send；所选凭据直接发送，不向其他账号兜底。响应要求存在有效网页标题和
HTTP(S) URL，不把 HTTP 200、空数组或错误 JSON 当成功；限制 60 秒及 1 MiB，不重试。
返回安全数量摘要，不保存上游错误正文、网页片段或密钥。

审计复用原记录 owner，但协议明确为 `search/search`，不记成聊天；保留精确凭据归属、
真实 HTTP 状态和模型健康 CAS。新模板用例没有标准答案，`score`、`correct` 均为 null，
`capabilityLevel: unrated`，不虚构聊天智力评分。

## 实际调用结果与阻塞边界

| 检查 | 实际结果 | 能证明什么 |
| --- | --- | --- |
| UI 单账号搜索，`you-cred-0` | 403，1904 ms；failed | 所选凭据的真实搜索被上游拒绝 |
| 一次独立官方 POST 诊断 | 403，JSON `{"message":"Forbidden"}`，`x-amzn-errortype: ForbiddenException` | 不是只有 Gateway 路由才失败；不能据此确定密钥过期、撤销或 scope 中的具体原因 |
| 普通 Gateway `POST /v1/search` | 503，62 ms；`upstream_credential_unavailable` | 失败后不继续调用已不可用凭据；唯一账号不可用时没有其他可用候选 |

普通调用审计的 `candidateCount: 1`、`routeAttemptCount: 0`、`attemptedProviderIds: []`；
未再次发送上游请求、未调用其他服务商。它不是“上游再次返回 503”。
本轮两条 Gateway 审计全部失败并保留，没有清除历史或伪造可用性：

- UI probe：`probe-2836327d-db9b-4abd-9225-dc9761156138`，精确归属 `you-cred-0`。
- 普通请求：`gw-audit-092ef4d5-9229-4d1a-a8f2-32a8578ecfea`，本地可用性准入拒绝。

官方文档将 403 说明为路径权限不足，但本次仅返回通用 Forbidden；不据此断言是额度不足。
没有轮换密钥、创建上游账号、充值、试探其他产品权限或改用 keyless MCP 绕过凭据验收。
后续需要一份当前确实能访问 You Web Search API 的有效凭据，再完成真实成功调用和标蓝。
`q` 别名的真实上游成功、其他参数和站点健康轮询均未验收。

## 验证与保护

- 前端新测试先在旧聊天测试策略上失败，改为无评分搜索策略后通过；覆盖固定地址、
  三适配器只读、固定健康源、空池及搜索路由，不引入首个账号。
- Rust 回环测试先实际返回 Unsupported 而失败；修复后与 Nvidia、手动账号测试一同
  通过，共 7 项。覆盖精确 X-API-Key、POST、query/count、本地 model 不外发、
  无聊天消息、空结果、401、非法 JSON、超限、错误载荷、非法 URL、禁用账号、
  不支持模型、无评分结果、审计与模型健康归属；合成凭据，不访问上游。
- 前端定向回归 8 个文件、41 项通过；TypeScript typecheck、官方 rustfmt、
  有效行检查器 31 项、ratchet 和 `git diff --check` 通过。不执行全量回归或正式发布审计。
- 12 个本轮核对的源码/文档文件均为 UTF-8 无 BOM，未包含真实密钥；完成标记文件
  逐字保持不变，仍仅登记 Nvidia/OpenRouter。
- 有效行：runtime 模块 70→71、console probe 288→299、recording 116→140、
  workflow 90→93；新搜索 probe 143 行、新 Rust 合同测试 183 行。无新例外、
  无依赖升级、无存储版本变更、无 baseline 修改。
- 使用官方 `build-gateway-devtest.ps1 -Edition api -Jobs 4` 生成本轮验证后端，
  构建凭据为 devtest；EXE SHA256：
  `f49584c19b2531119e7ec7501fd4324d7103f343a4400fd0c5410f7d290c94a8`。
  这是 API 验证构建，不是新桌面分发包或正式 Release。保留既有编译 warning，不宣称零告警。
- 原 active revision 仍为 `r110-867d2b28b28f`，原 You 服务商逐字段不变；
  原 `routes.yaml` SHA256 仍为
  `3aead99987e30eaf4e64b0e9a81c435dcdf59aea21ebe369f1add3eb80e61107`。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-you-adaptation-20261025/`。
保留创建空池、UI 结果、独立诊断、普通调用、审计、完整性报告及无密钥截图；
私有状态与凭据备份不属于可发布产物。结束后仅停止自建 helper，保留隔离状态与备份。
本轮不提交、不推送；不将 You 加入完成列表。
