# Groq 模板复核与隔离真实验收（2026-10-27）

## 结论

按标准流程完成官方合同核对、模板修正、正常 UI 创建空服务商、正常 UI 导入唯一账号、
精确账号 UI 测试和 Chat / Responses 的非流式及 SSE 验收。全部通过后于
`2026-10-26T16:56:53Z` 登记 `groq-openai` 新流程完成标记。

目录发现 11 个模型，只验证 `openai/gpt-oss-20b` 一个模型。自动模型测试始终关闭，
站点健康默认开启；没有批量调用其他 10 个模型，没有直接上游生成或充值、订阅操作。
目录包含音频及安全模型，不表示所有条目都适合普通 Chat 请求，更不表示都已调用验证。

## 官方合同与修正

- 官方 Base URL 为 `https://api.groq.com/openai/v1`，Bearer API Key；Chat 使用
  `/chat/completions`，SSE 以 `data: [DONE]` 终止。沿已有 Groq preset / OpenAI
  标准编码器和 `direct_http`，不增加独立 transport 或强制品牌请求头。
- 旧模板仅预置 `llama-3.3-70b-versatile`，而该 ID 不在本轮认证目录里。新模板移除
  该 seed，创建显式空凭据池，无默认账号、无模型、无初始路由。
- 接入共享官方目录 owner 的 `GET https://api.groq.com/openai/v1/models`，支持
  新账号导入、双栏编辑的“拉取模型”和精确账号刷新，保留手动模型及 revision/CAS 保存。
  目录不是协议发现、权限或成功证据，不生成 `discovery_job` / `verified_models`。
- 新管理入口为 `/v1/internal/gateway/console/groq-model-catalog`，复用管理认证、
  secret grant、精确账号绑定及并发限制。只接受精确 Groq preset 和官方完整 API base；
  不接受任意地址、恶意账号覆盖地址、混合账号/Key 输入或 CRLF Key。
- 共享读取器禁重定向，2 MiB 上限、最多 2048 个 ID、25 秒请求 / 30 秒总时限；
  错误不回显上游正文或凭据，失败不清空 last-good。旧配置及存储版本保持兼容。
- `groq-status` 默认开启，300 秒刷新、900 秒有效、仅提示、未知放行。自动生成测试关闭；
  手动默认只选 `openai/gpt-oss-20b` 和一个 `Reply with only OK.` 用例，无标准答案，
  不生成 IQ 分数。manifest 原有 `supportsModelsEndpoint=true` 与官方接口一致，无需修改。

本轮明确联网核实的官方资料：
[OpenAI compatibility](https://console.groq.com/docs/openai)、
[Models](https://console.groq.com/docs/models)、
[API reference](https://console.groq.com/docs/api-reference)。

## 真实 UI 与调用证据

API 后端为 `127.0.0.1:4300`，源码管理 UI 为 `127.0.0.1:1435/ui/`。数据根仅位于
`linshi/gateway-groq-template-acceptance-20261027/private/runtime`，不借用当前 4200
使用的 `.ng-devtest-4300`。没有用管理 API 或 SQL 代替 UI 创建服务商、导入账号。

- 正常 UI 选择 Groq，确认官方固定项、自动测试关闭、单模型策略及站点健康开启后添加。
  空池 revision=`r1-85fd3f413721`，账号、模型与路由均为 0。
- 正常账号表单选择品牌 Groq，ID=`groq-acceptance-20261027`，输入用户 Key，点击
  “拉取模型”：左栏出现 11 个真实目录 ID，右栏为空；保存 revision=`r2-929b5442c3f4`。
  私有 Key 回读一致，账号及服务商模型池、路由各 11 项，双栏分别持久化。
- 导入完成后审计为 0，账号为待观测；没有把目录发现提升为账号可用。
- 从该账号的“测试账号”打开固定账户面板，确认仅一个模型、一个用例、自动关闭后只运行一次。
  `2026-10-26T16:56:02.9319291Z` passed，回答 `OK`，耗时 583 ms，gradedCount=0、score=null。
  审计=`probe-a198d80e-68a0-4c68-b2b5-06a1c1d6821d`，上游 200，精确凭据归属一致。

以下四次普通请求均经 4300 Gateway 进入，没有使用直接官方生成代替网关协议验收：

| 入口 | 模式 | HTTP / 正常结束 | 最终回答 | 耗时 ms | request id |
| --- | --- | --- | --- | ---: | --- |
| `/v1/chat/completions` | 非流式 | 200 / stop | OK | 656 | `7b56d2c1-0aab-462e-9857-68ec13676c7e` |
| `/v1/chat/completions` | SSE | 200 / stop、DONE | OK | 516 | `1802968f-903f-49ff-b0ad-21465f142f42` |
| `/v1/responses` | 非流式 | 200 / completed | OK | 859 | `65050e0e-a8e7-4033-ade0-f9697bd4d1ea` |
| `/v1/responses` | SSE | 200 / response.completed | OK | 609 | `b7120580-5bb1-4539-bb1a-bd92c0b83da3` |

共 5 次网关生成、5 条 completed / 200 审计，全部归属唯一凭据，没有其他账号兜底。
四次普通请求各只有一次路由尝试；UI probe 的 trace 仅记录精确凭据，不伪造普通路由 trace。
Responses 复用网关已有桥接，不将其冒称为 Groq 原生 Responses 验收。
只从 message / output_text 提取最终回答，检查正常终止，不以空 200 或 reasoning 代替完成。
未取得逐请求官方账单，费用保持未知，不因本次成功推断所有模型免费或可调用。

## 构建与证据边界

仅通过官方脚本构建隔离用途的 API devtest，Jobs=4、依赖锁定，前后来源指纹一致：
`d546a7d2a042da1cb517cd03431defe25aee5bb40d8cf3ee874166d8129b3a37`。
冻结 EXE 为 80,892,928 bytes，SHA-256：
`fbdeb9f0016a2f6c4bf5c51c7914022237b932771941f5fa2c2d4b3cbf57a3a4`。
二进制与来源凭据、复制后哈希、独立 healthz=200 均已核对。完成标记在真实验收后才写入
前端 registry，不宣称该后续标记已嵌入先前构建的 API EXE。

公开脱敏证据位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-groq-template-acceptance-20261027/`：
目录预检、空池、账号导入、零生成断言、UI 结果、四份 wire、审计、完整性和
`acceptance-status.json`。原始响应、Key、管理 token 与运行数据仅在私有子目录。
`ui-catalog-model-editors.png`、`ui-account-success.png` 为真实源码 UI 截图。
started guard 保留，恢复时先对账，不删除后重放调用。

真实验收时 SQLite integrity=ok，仓库 routes、正式 `.ng` 和活动手测配置的内容哈希均
与本轮初始快照一致。没有向当前 4200 发送写入或重启命令。

定向验证和最终收尾见本目录证据；未执行全量 Cargo 回归、11 模型批量调用、长期稳定性、
其他服务商付费测试、桌面整包、正式发布、提交、推送或向当前 4200 导入该账号。

## 最终验证与收尾

- 前端最终定向回归：10 个文件、97 项通过，覆盖 Groq、Poe、OpenRouter、NVIDIA、
  账号双栏、默认健康策略、创建流程、完成标记和 Together 移除；TypeScript 通过。
- Rust 目录读取器 15 项、模型目录重载与路由投影 2 项、Groq 管理入口合同 1 项通过。
  定向 `rustfmt --check` 通过；未在验收后改动 Rust 或再次发送生成请求。
- 44 个 line manifest 校验通过；有效行检查器 31 项通过，最终 ratchet 和
  `git diff --check` 通过。受影响生产源码最多 435 有效行，没有新增超限文件；
  新 Groq 前端测试 81 行、管理入口合同测试 58 行。详细前后计数见 `hygiene-final.json`。
- 新增、修改源码和文档均核对 UTF-8 无 BOM；完成日志统一为 UTF-8 无 BOM。
  公开文本证据与本轮源码未包含 Key、管理 token 或 relay token。
- 只重启本轮自建的 UI helper 加载后写的 registry，确认 Groq 出现“新调用流程已验收”
  标记并保存 `ui-template-complete.png`，随后关闭未提交的表单，没有重复创建或导入。
- 北京时间 `2026-10-27 01:05:39` 完成收尾：4300 通过正常管理 drain 返回 202 后退出，
  1435 自建 UI helper 和临时浏览器标签均关闭。私有账号数据、冻结 EXE、started guard
  和原始证据均保留，没有删除后重放请求。
- 当前两个 4200 监听 PID 仍为 `392` / `4280`，本机 healthz=200；初始配置内容哈希
  全部不变。详见 `cleanup.json` 和 `drain-result.json`。
