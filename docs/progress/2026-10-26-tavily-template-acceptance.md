# Tavily 标准模板真实验收（2026-10-26）

## 结论与边界

Tavily Search 已按 You、Exa 的新流程完成真实验收，登记 `flowVersion: 1` 蓝色完成标记。
不是只检查静态模板或 mock：通过真实源码 UI 创建空凭据池、导入一个用户提供的账号，
再执行该账号的搜索连通测试和普通 Gateway 搜索。此次不验收 Chat/Responses、Extract、
Crawl、Map、Research、全部搜索参数、自动轮询或长期稳定性。

使用隔离后端 `127.0.0.1:4300` 和前端 `127.0.0.1:1435/ui/`；未停止、接管或修改用户的
4200 实例。密钥只进入受限的私有导入文件和隔离数据根，没有写入源码、截图或公开证据。
原 Tavily provider 和 routes 保持一致；原账号中的占位符未被拿去请求真实上游。
本轮没有生成桌面 EXE 分发包、正式发布、提交或推送 Git。

## 问题复现与最小修复

原模板继承聊天测试的 `expectedAnswer: OK`，并且后端没有将 Tavily 接入搜索 probe；
定向红测分别捕捉默认策略错误和 Unsupported。修复只扩展现有搜索链：

- 前端默认 Selected + `tavily-search` + 无标准答案的“搜索连通”，不计算聊天 IQ 分数。
- 后端增加 Tavily preset 分派，模型映射后仍只接受精确 `tavily-search`。
- 复用 prepare/planner/headers/profiled-send，Bearer 认证、`POST /search`、`max_results: 3`、
  `search_depth: basic`、`auto_parameters: false`，关闭答案、原始网页和图片；本地路由 model
  在 planner 中移除，不发给上游。没有新增重试、跨账号或跨服务商 fallback。
- 复用 60 秒超时、1 MiB 响应上限；必须有非空标题及 HTTP(S) URL，空结果、错误 JSON、
  错形状、超限或含错误对象的 HTTP 200 不会被判为成功。432/433 使用安全的额度错误摘要，
  不回显上游正文、网页片段或密钥。
- 保留固定 Base URL、三适配器和健康源技术项；模板只建空池，账号通过独立界面导入。

本轮联网核实官方合同：https://docs.tavily.com/documentation/api-reference/endpoint/search 。

## 真实 UI、请求和归属证据

UI 路径：添加服务商 → 标准模板 tavily → 添加 → 添加账号 → 保存 → 测试账号 → 运行测试。
空池 revision 为 `r1-f9502b7605c8`，导入后为 `r2-b641e5525e02`；回读密钥与用户提供材料一致。
没有用 SQL 或直接管理 API 代替界面创建/导入。

| 验收项 | 实际结果 | 精确归属 |
| --- | --- | --- |
| UI 搜索连通 | Passed，3 条有效网页结果，0 失败，0 未执行；无评分，healthRecovery=applied | tavily / tavily-cred-0 / tavily-search |
| POST /v1/search，query | HTTP 200，1 条带有效标题和 URL 的结果，1765 ms | 同一账号、同一模型 |
| POST /v1/search，q | HTTP 200，1 条带有效标题和 URL 的结果，391 ms | 同一账号、同一模型 |

UI probe 审计：`probe-a6d9d8fd-43db-4de6-bb79-7463e9eb2750`，checkedAt 为
`2026-10-25T17:45:00.5285774Z`。两次普通请求 ID 分别为
`1f537e23-14cf-4b09-a1b5-521784cad1e5`、`f5fa18c2-f1fd-47c1-9010-3ccd2c56eef7`；审计分别为
`gw-audit-928bec7b-a8e3-4e99-b227-01521acd1994`、`gw-audit-d154b992-1d7a-465c-b9dd-5907091f05a1`。

三条审计均为 `search/search`、upstream 200、`realCredentialRef=tavily-cred-0`。两次普通请求的
`routeTrace.routeAttemptCount=1`、`candidateCount=1`、`attemptedProviderIds=[tavily]`，adapter 为
`search_api_compatible`、profile 为 `tavily`、执行模式为 `direct_http`，没有借其他账号兜底。
UI 刷新后显示 1 个可用账号、3 次调用、成功率 3/3。两次普通响应各报告 `usage.credits=1`；
这不是包含 UI probe 的总消费，UI probe 未记录额度消耗，不能据此推断总费用。

## 定向验证和构建来源

- Rust 的 Tavily、You、Exa 搜索 probe 合同均通过。Tavily 覆盖 16 次精确 loopback 出站，
  认证、请求体、401/402/403/429/432/433、空/坏/超限响应、错误模型及 alias、禁用账号、
  不评分、精确账号审计和健康恢复。默认 CI 测试不请求付费上游。
- 修复后前端 6 文件、23 项通过；TypeScript、定向 rustfmt、31 项有效行 checker 测试通过。
  首次前端存在一个资源竞争下的既有用例超时，单 worker、15 秒上限复跑通过，未改源码掩盖失败。
- `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4` 通过，耗时 13m20s，保留既有 warning；
  buildKind 为 devtest，API 默认端口 4200，此次明确以环境变量隔离绑定 4300。
- 验收 EXE：`target/devtest-editions/api/gateway.exe`，SHA-256 为
  `9478be57b27e7b8002346f8b3b7a6a8bd27611cbfeec8e242aa8a75f5a3e38b0`。
  源码指纹为 `5066cb708280bfb2b2886f56586f67ae9d1b073f09d102fe17762eb0df6531c8`。
  来源前后检查及二进制哈希通过；蓝色登记和文档在真实成功后才更新，该凭据不冒充后续桌面包。

有效行：共享 probe 182→202，前端 workflow 94→94，registry 36→41；新增 Tavily UI 测试 40、
后端合同测试 244。完成列表测试 92→93；无新增超限文件，也没有修改超限历史文件或申请例外。
未执行全量 Cargo 回归、其他服务商付费上游测试或正式发布安全审计。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-tavily-adaptation-20261026/`。
包含 UI 截图、真实请求摘要、精确账号审计、完整性、构建凭据、红绿测试和定向验证记录。
隔离数据和私有备份保留，不能将 private 目录作为可公开交付内容。

## 收尾复核

完成登记后，前端 6 文件共 23 项、TypeScript、定向 rustfmt、31 项 checker 测试、有效行
ratchet 和 `git diff --check` 再次通过。真实源码 UI 已显示 Tavily“新调用流程已验收”，
截图为 `ui-template-complete.jpg`；调用成功总览为 `ui-live-success-final.jpg`。
机器回执 `acceptance-verified.json` 复核三条成功审计、精确凭据、无评分、普通调用单次路由
和原数据完整性。审查仅涉及本轮窄改动，保留工作树中其他未提交内容。

隔离后端经管理 drain 返回 HTTP 202 后退出；按端口、PID、EXE/脚本身份核对后关闭本轮
前端 helper。`2026-10-25T17:51:21Z` 复核 4300/1435 均无监听，4200 `/healthz` 为 HTTP 200。
没有清空测试数据、迁移用户凭据或覆盖用户正在使用的桌面包。保留本次 API devtest 构建凭据，
不把源码完成等同于用户已打开的旧 EXE 自动升级。

最后源码安全检查确认本轮 8 个源码/文档文件均为 UTF-8 无 BOM，且这些文件与公开证据
未包含本轮 Tavily key、隔离管理 token 或调用 token；只检查受限 private 目录以外的证据。
