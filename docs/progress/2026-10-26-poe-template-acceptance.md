# Poe 模板二次核对与隔离验收（2026-10-26）

## 当前结论：免费模型完成隔离真实验收，已登记完成标记

已按标准新流程，通过源码 UI 创建空服务商、正常导入唯一账号并读取真实目录。
首轮 `gpt-4o-mini` 返回 `subscription_required`，失败证据完整保留。用户明确授权换用
免费模型后，实时公开目录中 request=`0.00` 的 `gemma-4-31b` 实际调用成功，随后重新在
独立数据目录完成正常 UI 建池、导入、精确账号测试与 Chat/Responses 非流式、流式验收。
只有这些检查全部通过后才登记 Poe 蓝色完成标记；302 个目录模型中仅验证了 1 个。

没有修改、停止或接管用户 4200 实例、其凭据、路由或数据库；没有生成桌面整包、
正式发布、提交或推送。本轮仅构建供独立 4300 验收的 API devtest。

## 合同核对与最小改动

- 官方 API base 固定为 `https://api.poe.com/v1`，认证为 Bearer API Key；沿用标准
  OpenAI 适配器，不新增旁路 transport。公开目录为 `https://api.poe.com/v1/models`。
- 模板移除单个历史默认模型 seed，新建显式空池，无隐式账号、无初始模型、无初始路由。
- 账号正常导入与显式刷新复用共享官方目录 owner、管理认证、secret grant、精确账号
  绑定、revision/CAS 保存、模型池并集与路由同步；目录读取不执行生成或写成功证据。
- 仅允许精确 Poe preset 与 HTTPS 官方 root 或 `/v1`，不接受任意目标、账号恶意覆盖
  地址、混合账号/Key 输入或带 CRLF 的 Key。响应禁重定向、2 MiB 上限、最多 2048 个
  模型 ID、25 秒请求/30 秒总时限；错误不回显上游正文或凭据。
- `poe-status` 站点健康默认开启；自动模型测试保持关闭。默认手动策略仅选择
  `gemma-4-31b`、一个 `Reply with only OK.` 连通用例，无标准答案，不生成 IQ 分数。
  首轮默认值曾为 `gpt-4o-mini`；按零价候选真实成功证据修正，不执行付费目录扫描。
- manifest 的 `supportsModelsEndpoint` 与实际官方目录能力一致。并行目录工作线对
  共享模块及其测试的修改已保留，没有用历史副本覆盖。

官方资料：[OpenAI-compatible API](https://creator.poe.com/docs/external-applications/openai-compatible-api)、
[List models](https://creator.poe.com/api-reference/listModels)、
[Current balance](https://creator.poe.com/api-reference/getCurrentBalance)。余额 HTTP 200
只证明认证与余额读取成功；公开目录 HTTP 200 不能证明 Key 的模型调用权限。

## 首轮 UI 与付费模型拒绝证据（历史，保留原结果）

隔离后端绑定 `127.0.0.1:4300`，源码 UI 为 `127.0.0.1:1435/ui/`，数据根仅位于本轮
linshi/private/runtime。显式端口与数据目录覆盖，不依赖当前手测构建的 4200 默认值。

UI 流程：添加服务商 → 标准 poe 模板 → 添加空池 → 添加账号 → 选择 Poe → 填入
账号 ID、名称和用户 Key → 保存。没有用管理 API 或 SQL 代替创建和导入。

- 空池 revision：`r1-4da48fb5e674`；无账号、无模型、无路由。
- 导入后 revision：`r2-0581c710ac78`；唯一 provider=`poe`，唯一账号
  `poe-acceptance-20261026`，名称“Poe 验收账号”，私有 Key 回读一致。
- 真实目录 302 个 ID，同步账号池、服务商池及 302 条路由。无 generative discovery job，
  无伪造 verified_models；导入后审计记录为 0，UI 为待观测而非可用。
- 在该账号的测试面板确认仅一个模型、一个用例、自动关闭后，只点击一次运行。
  `2026-10-26T13:52:59.0952615Z` 返回失败；模型 callable=false、gradedCount=0、score=null。
- UI 审计 `probe-f8e7fc36-b389-4799-af9a-1cfc6f83ca26`：provider=poe、
  realCredentialRef=`poe-acceptance-20261026`、model=`gpt-4o-mini`、上游 402，
  elapsedMs=601；没有其他账号兜底。

为取得被安全层隐藏的具体拒绝原因，只向固定官方 origin 发送了一次同模型诊断，
max_tokens=1、无重定向、无重试。它仍返回 HTTP 402：

```text
The model gpt-4o-mini requires an active Poe subscription for API access.
type=subscription_required
code=subscription_required
```

这是上游明确的模型 API 订阅资格拒绝，不能靠修改 Base URL 或伪造成功解决。
仅测试了这个模型，不推断目录内所有模型都能或都不能调用，不逐个尝试绕过资格限制。
原计划的四次普通 Chat Completions/Responses 非流式与流式调用均未发送。
该轮当时暂停，没有发送原计划的四次普通 API 请求。用户随后明确允许选择免费模型，
下述新轮次复用同一 Key，但不覆盖、删除或改写首轮运行数据和失败证据。

## 免费模型续验：六次有界生成，全部成功

官方 [External Application Guide](https://creator.poe.com/docs/external-applications/external-application-guide)
区分需要 points 的付费模型 API 与免费模型；目录中的 `pricing=null` 不是免费保证。
本次实时读取的 302 项目录中，`gemma-4-31b` 明确列出 request=`0.00`，其余价格项为 null，
支持 Chat Completions 与 Responses。该快照只作为候选依据，不承诺未来价格或所有账号权限。

1. 固定官方 origin、禁重定向、无重试，先进行一次 max_tokens=512 的短文本请求：
   HTTP 200、finish_reason=stop、最终回答 `OK`，耗时 6000 ms。
2. 新隔离数据根为 `linshi/gateway-poe-adaptation-20261026/free-model-acceptance/private/runtime`；
   不复用首轮 402 健康状态，不写 SQL，不借用 4200 数据。仍使用经核验冻结的 API devtest
   后端；源码 UI 包含本轮默认策略修改，不声称旧二进制来源凭据覆盖后来修改的整个源码。
3. 正常 UI 创建空池 revision=`r1-f31737b2ffe2`；正常 UI 导入账号
   `poe-free-acceptance-20261026`、名称“Poe 免费模型验收账号”，revision=`r2-aab01daa444c`。
   Key 回读一致，真实目录与路由各 302 个；导入完成后审计为 0、状态为待观测，自动测试关闭。
4. UI 固定唯一账号、仅选择 `gemma-4-31b` 和一个连通用例后只运行一次：
   `2026-10-26T14:17:58.4580438Z` passed，回答 `OK`，elapsedMs=5103、gradedCount=0、score=null。
   审计为 `probe-f2dc0ea5-9107-46f8-84c3-0ddfc3c49a7a`，上游 200、精确账号归属无兜底。
5. 普通网关请求均从 `127.0.0.1:4300` 进入，没有直接官方调用代替协议验收：

| 入口 | 模式 | HTTP | 正常结束与最终回答 | 耗时 ms | request id |
| --- | --- | --- | --- | ---: | --- |
| `/v1/chat/completions` | 非流式 | 200 | stop / OK | 6360 | `dcd9b5c9-d661-4cfc-85e3-81a883262613` |
| `/v1/chat/completions` | SSE | 200 | stop、DONE / OK | 4593 | `1776f29a-5747-49ce-b2b2-aaf3eda23c4c` |
| `/v1/responses` | 非流式 | 200 | completed / OK | 3391 | `46799fea-5241-4939-b98d-507d076d3a44` |
| `/v1/responses` | SSE | 200 | response.completed / OK | 3031 | `20f8ac68-183b-4788-8f13-5e74f1195c46` |

Responses 非流式原文同时含 reasoning 与 message；续验核对脚本只从 message 提取最终
回答，确认 `OK`，不把 reasoning、空 200 或不完整事件算通过。此复核只读已保存响应，
没有重放生成调用。四次请求均只尝试 provider=poe 一次，realCredentialRef 完全一致。

本轮共 1 次官方免费候选调用、1 次 UI 测试、4 次普通网关调用；网关 5 条审计全为
completed/200。持久模型健康仅出现 `gemma-4-31b`，没有将其余 301 个模型写成验证成功。
余额前后观测减少 0 points；这不是逐请求精确账单，也不推导 USD 或 IQ 花费。

新证据位于原目录下 `free-model-acceptance/`：`candidate.json`、`ui-empty-pool.json`、
`ui-imported-account.json`、`ui-import-no-generation.json`、`ui-test-results.json`、四份 wire
结果、`audits-sanitized.json`、`integrity.json` 与 `acceptance-status.json`。各 started guard
保留，原始响应与凭据只放 private。`2026-10-26T14:22:15Z` 复核通过后登记完成 receipt。
截图为 `ui-account-success.png`；未新增订阅、付费操作或其他模型试探。

## 定向验证与构建

- 前端初次红测复现旧 base/单模型 seed、导入未走真实目录和缺少 Poe 刷新链，修复后
  7 文件 75 项通过；TypeScript 和 44 条 line manifest 校验通过。
- Rust 目录相关 43 项和 Poe 管理接口权限合同 1 项通过。覆盖目录 GET、去重、恶意
  origin、禁止重定向、非法/超大响应、账号绑定、secret grant 与零生成 POST。
- 首轮 Rust 目录测试 42/43，失败为同期共享目录测试对 legacy manual fallback 的旧
  断言；保留 owner 的修正后重跑 43/43，没有放宽 Poe 的 origin 或鉴权合同。
- 官方 API devtest 构建首轮 Cargo 成功（11m16s），但编译期间全仓来源发生变化，
  provenance gate 正确拒绝签发凭据。未使用该无凭据产物，保留失败日志。
- 重新执行同一官方构建，Cargo 校验缓存后通过，完整 wrapper 24.85 秒、exit 0；
  前后源码指纹一致。此数据是本次增量实测，不承诺普遍提速倍数。
- 第二轮 API 来源指纹：`01c1eda5fcde6de905ee0db963004932d869cc0f72a98f23ec85c5afbcd98144`。
  冻结的验收 EXE 为 80,861,184 bytes，SHA-256：
  `2c0f89c4c09a4e7cf87599d7e993d3dafd67344b45c09c9a7ea8a1f3e75d19fb`。
  构建凭据、复制后的哈希和独立 healthz=200 已核对；不是桌面或正式发布验收。

本轮没有修改数据库格式、历史账号 ID 或凭据，没有新增后台生成循环或重试。
SQLite integrity=ok，原 `.ng`、`.ng-devtest-4300` 的 route/console 配置及仓库 routes
哈希与本轮初始快照一致。Key、管理 token、原始响应和运行数据仅保存在私有目录。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-poe-adaptation-20261026/`。
`ui-test-started.json` 和 `diagnose-402-started.json` 为防重放 guard；不得删除后悄悄重试。
后续恢复必须先核对既有失败证据，不把本轮两个失败尝试改写成成功。

## 首轮收尾复核（历史）

- 最终前端 8 文件 86 项、TypeScript、定向 rustfmt、44 条 manifest、31 项 checker、
  effective-line ratchet、diff-check、有效行测量及 UTF-8/秘密扫描全部 exit 0。
  没有跑全量 Cargo 回归、其他厂商付费测试或正式发布安全审计。
- 有效行：模板 450→450、workflow 98→102、共享前端目录 71→74、Rust 目录 170→184、
  目录测试 150→221、管理目录接口 127→144、console exports 320→322、router 243→247、
  重载测试 87→95；新前端合同 70、新 Rust 合同 58。包含保留的同期共享修改，
  全部低于 500，无新增超限或历史超限增长，不申请例外。
- 官方余额前后观测减少 0 points；两个调用尝试均 402，没有成功生成。
  余额差不等同于精确请求账单，也不据此推导 USD 花费或 IQ。
- `ui-imported-402.png` 记录唯一账号、302 个目录模型、1 次失败与 0% 成功率。
  Poe 模板仍未登记完成标记，现有完成名单保持不变。
- `2026-10-26T14:00:34.2189082Z` 收尾：drain=202，本轮 API/UI helper 已退出，
  4300/1435 无监听，临时浏览器标签已关闭；隔离数据与原始证据保留。
  原本机 4200 PID=35060、局域网 4200 PID=4280 不变，本机 healthz=200。

## 免费续验的最终检查与收尾

- 默认模型先红测（3 项中 1 项因旧 `gpt-4o-mini` 不符失败），改成免费候选后 3/3；
  完成名单先红测（10 项中 1 项因尚无 Poe receipt 失败），真实验收通过后登记并绿测。
  最终相关前端 8 文件 86 项通过，TypeScript、44 条 manifest、31 项 checker、ratchet、
  diff-check、有效行测量、UTF-8 无 BOM 与公开证据秘密扫描全部通过。
- 本轮没有再改 Rust 或重编后端；复用前述 43 项目录、1 项管理合同与定向 rustfmt 结果。
  API EXE 实际哈希再次核对一致；不把源码 UI 的新增默认策略或完成标记冒称已进入旧 EXE。
- 本次续验有效行：workflow 102→102、Poe 合同 70→70、完成 registry 56→61、目录界面测试
  101→101。没有新增超限；未增加后台生成、数据库迁移、无限重试或其他服务商调用。
- `2026-10-26T14:31:11Z` 收尾：仅隔离后端 drain=202，自建 Node helper 在核对其精确
  命令行后退出；4300/1435 无监听、临时标签已关闭，隔离数据与原始证据保留。
- 首次最终基线断言因活动环境发生变化而失败，不能写成“4200 全程未变化”：本机 4200
  PID 从 35060 变为 38244；`.ng-devtest-4300/local/state/console/local-active.json` 的
  内容哈希改变，修改时间为 `2026-10-26T14:20:28Z`。仓库 routes、正式 `.ng` 的两份配置
  和手测 routes 内容哈希仍与最初一致；局域网 4200 PID=4280，本机 healthz=200。
  本任务的启动、导入、生成与 drain 均明确绑定独立数据根和 4300；没有向 4200 发送写入
  或重启命令，也没有还原、覆盖活动 revision。该环境差异单独记录，不伪造基线相等。
- `final-config-hashes.json`、`cleanup.json` 保留上述漂移；最终验收 receipt 明确区分
  业务 passed 与 liveConfigDriftObserved。模板策略截图为 `ui-template-policy-complete.png`，
  截图中 `poe-2` 仅是打开新建面板自动生成的未提交草稿，本轮未创建第二个服务商。

未执行：302 模型批量调用、付费模型订阅开通、长期稳定性/全部能力验收、桌面新整包、
正式发布、提交、推送或将隔离账号移入用户当前 4200 实例。
