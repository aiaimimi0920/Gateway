# Nvidia 标准模板：首轮真实接入验收

验收日期：2026-10-25（Asia/Shanghai）。模板 `nvidia-openai` 初步通过，
只为该模板登记 `flowVersion: 1`，不代表全部模型或所有 API 能力已验收。

## 隔离、备份与实际 UI 操作

- 使用独立 API devtest 后端 `127.0.0.1:4300`、真实前端源码页面
  `http://127.0.0.1:1435/ui/`。不是 HTML 预览、mock 数据或直接写入数据库。
- 独立状态目录为 `C:/Users/vmjcv/.ng-devtest-4300-nvidia-20261025`，未复用生产状态。
  SQLite/standalone readiness 返回 200，初始路由为空。
- 旧 Nvidia 服务商及两个凭据备份至
  `C:/Users/vmjcv/.ng/backups/nvidia-template-20261025/providers.json`；权限限制为当前用户及 SYSTEM，
  回读逐字段一致。真实密钥未写入 Git、本文、截图或公开测试证据。
- 真实页面依次点击“添加服务商 → 标准模板 → nvidia → 添加”，保存空服务商。
  `r1-c1a73bff2372` 包含 `explicit_credentials_only: true`、`credentials: []`。
- 再从 Nvidia 卡牌展开账号库，点击“手动录入账号”，依次输入备份中的两个账号并保存。
  保留 `nvidia-cred-0` 和 `legacy-912dd393b5296232804ffa02` 的 ID，回读密钥与备份一致。
  仅隔离测试账号启用；旧实例两个账号仍禁用，旧服务商记录完全一致。
- 通过账号编辑、模型池编辑选择本次实际可调用的模型，并点击两个账号各自的
  “测试账号 → 运行测试”。账号、模型声明与路由修改均经 UI 保存。

## 模板与三适配器

官方固定地址为 `https://integrate.api.nvidia.com`，preset 补齐 `/v1/chat/completions`，
不能把 Base URL 改成带 `/v1` 后再与固定路径重复拼接。

| 项目 | UI 固定值 | 实际运行归属 |
| --- | --- | --- |
| 调用适配器 | OpenAI API | preset 的 OpenAI Chat 编码器 |
| 指纹适配器 | 自动 | 现有 Nvidia/标准 API 请求身份层 |
| 专用适配器 | 无（标准 API） | `direct_http`，没有 Web/Basispoints 会话 |

三项保存为 `call_flow` 的 `auto/auto/auto`，表示继承官方 preset，并非未实现的占位值。
普通调用审计确认 `selectedProtocolProfile=nvidia`、`selectedPipelineMode=direct_http`、
`selectedUpstreamTargetProtocolFamily=openai_chat`，每次仅尝试 `nvidia`，没有其他服务商兜底。

模板默认模型调整为本轮真实验证通过的 `openai/gpt-oss-20b`。这里的 `openai/` 是 Nvidia
托管模型的模型 ID 前缀，实际服务商仍为 Nvidia，不是调用 OpenAI 服务商。

## 真实模型结果

模型目录 `/v1/models` 200 不等于生成可用。本轮无凭据及两份凭据读取目录均列出 85 项，
但 `nvidia/nemotron-3-super-120b-a12b` 的实际生成对两个账号均返回 404。
一次直接上游诊断也返回 Function 对账号不可用的 404，排除 Gateway 请求路径独有问题；
该直接诊断不计入接入通过证据。旧默认 `deepseek-ai/deepseek-v3.2` 已不在本轮目录。

改用 GPT-OSS 后，以下 **6 次经 Gateway 的真实生成均获得非空 `OK`**：

| 入口 | 账号 | 结果 | 观测耗时 |
| --- | --- | --- | --- |
| UI 单账号测试 | `nvidia-cred-0` | passed，答案 OK，评分 100 | 2142 ms |
| UI 单账号测试 | `legacy-912dd393b5296232804ffa02` | passed，答案 OK，评分 100 | 2605 ms |
| `/v1/chat/completions` 非流式 | `nvidia-cred-0` | HTTP 200，OK | 1437 ms |
| `/v1/chat/completions` SSE | legacy 账号 | HTTP 200，OK，36 个事件及 `[DONE]` | 1954 ms |
| `/v1/responses` 非流式桥接 | `nvidia-cred-0` | HTTP 200，completed，OK | 1578 ms |
| `/v1/responses` 流式桥接 | legacy 账号 | HTTP 200，OK，9 个事件及 `response.completed` | 1141 ms |

四次普通 pipeline 调用的审计 ID：

- `gw-audit-e9492f31-55c2-4ada-ab54-161465b3349a`
- `gw-audit-dcafa759-d5d9-4f51-9ef9-0753800b9193`
- `gw-audit-0149de14-416e-475b-a449-1257b83bf281`
- `gw-audit-5653508d-88b5-42c9-8d37-3c715ea40868`

它们均为单次路由尝试，正确归属各自凭据。UI 刷新后显示可用 2、失效 0，当前模型 6/6 成功。
历史两次 Nemotron 404 及一次无可用 Nemotron 候选的请求原样保留，不清除失败来美化统计。
因此服务商聚合卡是 6/8 成功（75%），不能将它误报为全历史 100%。

## 实测发现并修复的 UI 缺口

- `accountManagementViewModel` 实际重导出 `routeDocumentAccountCatalog`，后者缺少空池标志判断，
  会伪造 `nvidia::default`；补齐与另一个目录入口相同的兼容条件，旧配置仍保留默认账号。
- `accountLedgerSections` 原先无条件丢弃零账号服务商，导致添加成功却看不到卡牌。
  现在保留真实空显式池，但不复活因账号筛选而隐藏的非空池；有卡牌时不再叠加“暂无账号”空态。
- 首个蓝色模板暴露了不存在的 `sr-only` 样式类；改用项目的 `nt-visually-hidden`，
  只显示蓝色 `nvidia`，验收说明保留给辅助技术及 title，不新增可见解释文案。

## 开发侧检查与证据

- 定向前端检查 9 个文件、51 项通过；蓝色标记隐藏文案修复另有先红后绿的回归测试。
- `npm run typecheck`、有效行检查器 31 项测试、有效行 ratchet、`git diff --check` 通过。
- 本轮生产源码有效行：catalog 480→480，route document catalog 210→210，
  ledger sections 430→430，workspace 413→413，directory 31→31，receipt 11→16。
  新测试分别为 52 与 43 行；未修改 baseline 或例外，依赖锁未变化。
- 为隔离测试构建 API devtest（`--locked`、4 jobs，7m25s）；本轮没有打 EXE 分发包、
  没有正式发布、提交或推送。真实 UI 使用开发服务，不声称嵌入页面已重新打包。
- 旧 `routes.yaml` SHA256 仍为
  `3aead99987e30eaf4e64b0e9a81c435dcdf59aea21ebe369f1add3eb80e61107`，旧 active revision
  仍为 `r110-867d2b28b28f`。未停止、重启或写入 4200 实例。

非敏感证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nvidia-adaptation-20261025/`。
包括 `gateway-acceptance-evidence.json`、`ui-test-results.json`、四份 `relay-*.json`、
`final-integrity.json`、`final-effective-lines.json` 和 `03/04` 开头的成功截图。
私有状态及凭据备份不属于可发布产物。测试 helper 在验收结束后停止，状态及备份保留。

未验收：全部目录模型、工具/多模态、长上下文、压力与故障恢复全矩阵，以及桌面 EXE 打包后的
同版手测。其他模板仍待逐个接入，不因本轮成功批量标蓝。
