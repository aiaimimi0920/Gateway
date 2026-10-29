# Exa 标准模板真实验收（2026-10-25）

## 结论与边界

Exa 新调用流程已通过本轮验收，登记 `flowVersion: 1` 蓝色完成标记。真实 UI 创建服务商，
再导入用户提供的一个账号，完成搜索测试及普通 Gateway 搜索、网页正文抓取。不是仅模型列表
或 mock 通过；不宣称支持 Chat/Responses、Research、全部搜索参数或长期可用性。

所有操作使用独立 `127.0.0.1:4300`，UI 为 `127.0.0.1:1435/ui`。4200 未停止、重启或修改。
旧 Exa 配置已私有备份；旧账号保存的是 `${EXA_API_KEY_1}` 占位符，未拿占位符调用上游。
新密钥只保存在受限的私有导入文件与 `.ng-devtest-4300-exa-20261025` 数据目录中，不进入源码、
截图或公开证据。没有改写生产账号、发布正式包、提交或推送 Git。

## 本轮最小适配

- 将原 You 专用搜索 probe 提取为 `search_model_probe.rs`；两者共用统一调用流程、发送链、
  60 秒超时、1 MiB 响应上限、脱敏、精确账号审计和模型健康恢复，不新增重试或跨账号 fallback。
- 显式区分 You 的 `count/results.web` 与 Exa 的 `numResults/results`；成功必须有非空标题和
  有效 HTTP(S) URL，错误对象、空结果、非法 JSON 或超限不会因 HTTP 200 而误判通过。
- Exa 标准认证头与官方示例对齐为 `x-api-key`。官方 OpenAPI 也声明支持 Bearer，不能把
  此调整解读为已证明旧 Bearer 认证必然失败。
- 标准模板固定 Base URL、三项适配器和站点健康技术项。只创建空凭据池，不嵌入首个账号。
- 默认测试为 Selected + `exa-search` + 无标准答案，不计算 IQ；模型映射后仍拒绝将
  `exa-fetch` 当搜索测试。普通抓取能力和 `exa-fetch` 路由保留。

官方文档于本轮联网核实：

- https://docs.exa.ai/reference/search
- https://docs.exa.ai/reference/get-contents

## 真实 UI 与请求证据

通过 UI 完成：添加服务商 → 标准模板 exa → 添加 → 添加账号 → 保存 `exa-cred-0` → 手动测试。
保存后的 `call_flow` 为三个 `auto`，由 Exa preset 解析为搜索 API、自动原生指纹、`direct_http`。
没有通过编辑数据库或直接管理 API 代替 UI 创建/导入。

| 验收项 | 真实结果 | 归属 |
| --- | --- | --- |
| UI 搜索连通 | Passed，3 条有效结果，未评级，healthRecovery=applied | exa / exa-cred-0 / exa-search |
| POST /v1/search | HTTP 200，1 条 Rust Documentation 搜索结果 | exa / exa-cred-0 / search |
| POST /v1/fetch | HTTP 200，Rust Programming Language 正文 1000 字符（请求主动限长） | exa / exa-cred-0 / fetch |

普通搜索 request ID：`db1cf403-2995-4b56-926b-73e95ba62fd9`。
普通抓取 request ID：`5d44b0bc-db58-4aa0-a91f-d2531f2bbc11`。
审计分别为 `gw-audit-e2460fbb-8bd3-4761-b6a6-8161796e9427`、
`gw-audit-81d49900-08cc-41de-b304-e4a02c58663a`。
UI probe 审计为 `probe-f6a5a5bd-244c-4d59-acfc-92d9ea96b98c`。

两次普通请求均为 `routeAttemptCount=1`、`attemptedProviderIds=[exa]`，实际凭据
`exa-cred-0`，adapter `search_api_compatible`，profile `exa`，执行模式 `direct_http`。
没有借助其它账号或服务商得到成功。普通响应同时校验有效结果/正文，而非只校验 HTTP 状态。
上游两次普通响应报告费用分别为 USD 0.007 和 USD 0.001；不是包含 UI probe 的总费用。

## 本地验证与构建

- 前端先红后绿：旧默认聊天用例、All 模型选择及 `expectedAnswer: OK` 被新测试捕捉。
- 后端先红后绿：旧 Exa probe 返回 Unsupported；新增合同覆盖固定请求、认证头、顶层结果、
  401/402/403/429、空/坏/超限响应、禁用账号、错误模型、fetch alias、精确审计与无评分。
  默认无 options 用例使用现有单账号 probe 入口；测试过程中纠正了原夹具的非法请求形状。
- `console_exa_search_probe_contract` 与 `console_you_search_probe_contract` 均通过；
  前者断言 13 次精确 mock 出站及审计，后者保护原 You 合同。本轮未重跑付费 You 上游测试。
- TypeScript typecheck、定向 Rust formatter、有效行 checker 的 31 项测试、ratchet、
  scoped diff 检查均执行。没有运行全量 Cargo 回归或正式发布审计。
- 使用 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4`，构建及来源/哈希检查通过；
  API devtest 默认端口 4300。没有打包新的桌面 EXE 分发包。
- 本次用于验收的 API EXE SHA-256：
  `95ab7776dc94a488ae6ac90c33ae3705705232c5c5fd86377fab5207cb424725`。
  蓝色标记与文档在真实验收后更新；上述 API 构建凭据对应验收时源码，不冒充随后前端标记的新包。

有效行：共享 probe 182，Console 分派 299→308，preset 207→207，planner 418→418；
未增加超限文件或申请例外。新测试及前端文件详见证据目录 `lines-after.json`。

本轮公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-exa-adaptation-20261025/`。
包含 UI 截图、普通请求摘要、审计、原数据完整性、构建凭据与定向验证记录。

## 最终复核

蓝色登记更新后的 5 个前端定向测试文件共 25 项通过，TypeScript、31 项 checker 测试、
ratchet、`git diff --check` 再次通过。源码及公开证据未检出本轮上游 key 或管理/调用 token，
原 Exa provider 和原 routes 哈希保持一致。

收尾检查时首次隔离 API 进程已退出，stdout/stderr 均为空；本轮未确定其退出原因，
不以短次调用验收宣称长时间稳定性通过。成功请求及审计已在退出前保存。随后按 PID 和
脚本身份核验关闭本轮 UI 服务；最终 4300/1435 均无监听，4200 `/healthz` 为 HTTP 200。
保留隔离数据与私有备份，未清理或覆盖用户原数据。
