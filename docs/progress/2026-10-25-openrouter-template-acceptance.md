# OpenRouter 标准模板：首轮真实接入验收

验收日期：2026-10-25（Asia/Shanghai）。`openrouter-openai` 的新调用流程初步通过，
登记 `flowVersion: 1`。模板接入通过不等于已有两份凭据都有效，也不代表全部模型通过。

## 隔离与实际 UI 添加

- API devtest 后端为 `127.0.0.1:4300`，管理页面为 `http://127.0.0.1:1435/ui/`，
  使用真实前端源码与管理 API，不是 HTML 原型或直接写数据库。
- 独立数据目录 `C:/Users/vmjcv/.ng-devtest-4300-openrouter-20261025`；初始配置
  `r0-ffe56ca6e895` 无服务商、无账号、无调用记录，SQLite/standalone。
- 原服务商和两份凭据先备份到
  `C:/Users/vmjcv/.ng/backups/openrouter-template-20261025/providers.json`。
  目录仅当前用户及 SYSTEM 可访问，回读一致；密钥不进入 Git、截图和公开证据。
- 实际点击“添加服务商 → 标准模板 → openrouter → 添加”。配置
  `r1-33bec8f14122` 为 `preset: openrouter-openai`、`explicit_credentials_only: true`、
  `credentials: []`，确认先创建空服务商，不偷带旧账号。
- 展开账号库，分别点击“手动录入账号”，保留旧凭据 ID 导入两份密钥。导入回读
  与备份逐份相同；旧实例仍使用原来的 legacy `openai` preset，本轮不改写旧数据。
- 通过账号编辑 UI 调整测试模型；测试均从各账号的“测试账号 → 运行测试”发起。

## 模板和三适配器

| 配置 | 固定 UI 值 | 运行合同 |
| --- | --- | --- |
| Base URL | `https://openrouter.ai/api/v1` | 官方规范 API 地址；沿现有 planner 去重推导路径的 `/v1` |
| 调用适配器 | OpenAI API | Chat Completions 与原生 Responses，按入口协议选择 |
| 指纹适配器 | 自动 | 已有标准 API 身份层，本轮不新增自定义应用标识头 |
| 专用适配器 | 无（标准 API） | `direct_http`，不进入 Web/Basispoints 会话 |
| 站点健康适配器 | `openrouter-status` | 固定 `https://status.openrouter.ai/`；创建默认关闭 |

保存 `call_flow` 为 `auto/auto/auto`，继承官方 preset 的编码、身份与执行 owner。
普通调用审计均显示 `selectedProtocolProfile=openrouter`、
`selectedPipelineMode=direct_http`，Chat 为 `openai_chat`，Responses 为
`openai_responses`，不是把 Responses 桥接到 Chat 后冒充原生接口。

官方文档已联网核对：Bearer API Key；`HTTP-Referer` 和应用标题头为可选归因字段，
不强制添加 Gateway 品牌标记。参考：

- <https://openrouter.ai/docs/api_reference/authentication>
- <https://openrouter.ai/docs/api_reference/responses/overview>
- <https://openrouter.ai/docs/api_reference/limits>
- 实时目录：<https://openrouter.ai/api/v1/models>，本次返回 467 个模型。

## 账号与模型的实际结果

| 原账号 | 官方身份接口 | UI 生成测试 | 结论 |
| --- | --- | --- | --- |
| `openrouter-cred-0`，OpenRouter live verified | 401 | 401，Credential authentication rejected | 当前密钥无效，不能当成可用账号 |
| `legacy-14cac1a1e17d6d48d92392ae`，OpenRouter Live | 200 | `openrouter/free` 返回 OK，评分 100 | 本轮可用 |

原默认 `google/gemma-4-26b-a4b-it:free` 在目录存在，但有效账号的实际生成返回 429。
一次独立直接上游诊断确认 `limit_source=upstream_provider_shared_pool`，来源为
Google AI Studio 临时限流。此诊断不计入 Gateway 接入通过证据；没有通过反复请求
规避限制，也没有充值、修改上游账号或改用付费模型。

模板默认模型改为官方免费模型路由 `openrouter/free`，不是承诺固定底层模型。
它由 OpenRouter 选择免费模型，仍受该平台额度和可用性限制；本次一个 Responses
响应的实际 `model` 为 `thinkingmachines/inkling:free`，报告 `cost: 0`。

以下五次经 Gateway 的生成均返回非空 `OK`：

| 入口 | 结果 | 客户端观测耗时 |
| --- | --- | --- |
| UI 单账号测试 | passed，评分 100 | 2345 ms |
| `/v1/chat/completions` 非流式 | HTTP 200 | 969 ms |
| `/v1/chat/completions` SSE | HTTP 200，13 个事件及 `[DONE]` | 718 ms |
| `/v1/responses` 非流式 | HTTP 200，completed | 2984 ms |
| `/v1/responses` SSE | HTTP 200，43 个事件及 `response.completed` | 2140 ms |

四个普通请求审计依次为：

- `gw-audit-e72b5ecc-7006-413d-b3d5-bb8613f5f126`
- `gw-audit-ab58c0e8-94e0-435e-938c-cf761f4e3c31`
- `gw-audit-f6c571c9-bea9-429e-9a20-1c5e8e495bdd`
- `gw-audit-0669287b-352b-4130-b1c7-1b81de2f8c9d`

成功请求最终凭据均为上述 legacy 账号；只尝试 `openrouter` 服务商，无其他服务商兜底。
两次非流式的路由尝试数为 2，之后各自流式为 1，不能报告全部请求均一次发送。
审计只记录最终凭据，不把中间每次尝试的身份或错误当成已经完整留证。
失败记录保留：401、Gemma 429、其后本地模型熔断 503。总历史为 5/8 成功，
`openrouter/free` 为 5/5；不清空失败历史或把总历史写成 100%。

## 检查、数据保护与范围

前端回归覆盖官方地址、只读三适配器、固定健康源、空池创建、模型路由合并、
原账号保留，以及只为 Nvidia/OpenRouter 显示蓝色。有效行、类型与相关回归结果
见同轮非敏感证据；未修改依赖锁、存储版本、有效行 baseline 或例外。

- 新测试分别先因旧 Base URL、旧默认模型失败，修改模板后转绿。
- 定向前端测试 8 个文件、42 项通过；`npm run typecheck` 通过。
- 有效行检查器 31 项通过、ratchet 通过、`git diff --check` 通过。
- 7 个本轮源码/文档文件 UTF-8 无 BOM，未包含真实密钥。
- 源码有效行：catalog 480→480、receipt 16→21、dialog test 86→90；
  新 OpenRouter 测试 54 行，无需新增例外。
- 刷新真实 UI 后显示 1 个可用、1 个失效；模板列表只新增蓝色 `openrouter`，
  辅助状态文案仍隐藏。截图保留原失败历史，没有伪装全历史 100%。

本轮没有 Rust 生产代码修改；复用既有 API devtest 可执行文件，真实 UI 由源码
开发服务构建，不声称旧 EXE 的完整源码回执已更新。本轮不生成分发 EXE，不提交、不推送。
本次运行 EXE 的 SHA256 为
`306d3b4982b07dc28b2f3f5427fd62448b32e58bef1aa885cc66a63e28fd57d5`。

原 active revision 仍为 `r110-867d2b28b28f`，原服务商逐字段不变，
原 `routes.yaml` SHA256 仍为
`3aead99987e30eaf4e64b0e9a81c435dcdf59aea21ebe369f1add3eb80e61107`。
未写入、重启或停止 4200 实例。测试结束后停止自建 helper，保留备份与隔离状态。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-openrouter-adaptation-20261025/`。
包含官方目录/身份摘要、创建空池状态、UI 测试结果、四份普通调用结果、逐请求审计、
完整性报告及无密钥截图。私有数据、凭据备份和 token 不属于可发布产物。

未验收：全部模型、付费模型、工具/多模态、长上下文、压力与故障恢复矩阵、
站点健康实际轮询以及桌面 EXE 打包后的同版手测。其他模板不因本轮成功自动标蓝。
