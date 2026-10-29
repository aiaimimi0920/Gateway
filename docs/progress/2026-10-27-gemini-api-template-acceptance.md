# Gemini API 标准模板验收通过

2026-10-27 17:48（北京时间），在用户授权“试试其他模型”后，使用原隔离账号
`gemini-acceptance-20261027` 和官方目录中的 `gemini-3.5-flash-lite` 完成验收。
本轮第一个替代模型成功后即停止换模型，未调用候选 `gemini-3.7-flash`，没有扫描目录。

## 验收链路

此前已完成模板核对、Base URL 修正、空池创建、正常 UI 导入和 46 个官方模型的目录读取。
导入阶段生成请求数为 0。此前失败与修正详情保留于
`2026-10-27-gemini-api-template-review.md`，不以本次成功改写旧模型的 404/503 事实。

| 检查 | 结果 |
| --- | --- |
| 正常 UI 创建空服务商 | 通过：0 账号、0 模型、0 路由 |
| 正常 UI 导入指定 Key | 通过：46 个目录模型，无批量生成 |
| 非流式 `/v1/chat/completions` | HTTP 200，回复 `OK`，stop，客户端 2234 ms |
| 流式 `/v1/chat/completions` | HTTP 200，`text/event-stream`，回复 `OK`，stop 与 `[DONE]`，891 ms |
| 精确账号 UI 手动测试 | passed，回复 `OK`，1448 ms，无标准答案、不生成 IQ 分数 |

三条审计均为 `completed`、upstreamStatus=200、
realCredentialRef=`gemini-acceptance-20261027`。
两条普通 relay 审计 routeAttemptCount 均为 1；无服务商或模型回退。
成功时活动 revision 为 `r4-f6f455e9ae72`，唯一测试模型为 `gemini-3.5-flash-lite`，
自动测试保持关闭。

审计 ID：
- buffered：`gw-audit-94c102a4-750f-434e-ae56-b69d471b07d1`
- SSE：`gw-audit-17b379fe-54ed-49f4-91b8-67d2018d73ca`
- UI：`probe-4d78ecf5-0a03-4a33-a886-2570d1086864`

## 登记与验证范围

实际验收通过后，才在 `providerTemplateAdaptation.ts` 为 `gemini-api` 登记完成标记，
并将模板默认连通测试模型改为本次成功的 `gemini-3.5-flash-lite`。
本轮生产源码仅修改前端默认模型与完成登记及保护测试，不改 Rust 执行器或历史协议夹具。
复用此前通过来源与 SHA256 校验的隔离 API devtest 后端；未出新桌面包、未覆盖旧 EXE。

流式成功仅证明**下游 SSE 合同**：现有执行器先聚合再桥接 SSE，不声称逐 token 透明转发。
只验收该账号的该模型连通性，不声称其余 45 个目录模型全部可调用，也未证明计费扣额、
工具调用、多模态、长对话或畸形 EOF 的完整行为。

## 本地检查与收尾

- 定向前端测试：14/14 通过；TypeScript typecheck 通过。
- effective-lines checker 测试：31/31 通过；ratchet 通过。
- `git diff --check` 通过。
- 已在最新源码 UI 确认 `gemini-api 新调用流程已验收` 标记。
- 4300 后端已正常 drain，1435 UI helper 已按进程身份核对后停止；两端口均无监听。
- 原 4200 两个监听进程（85612、4280）未变，测试数据与失败历史均保留。
- 边界：收尾时凭据池总览仍显示 1 个失效账号；本次三项成功证明指定模型实际可调用，不等同于账号聚合健康状态已全面恢复，该总览状态未在本轮继续诊断。

## 证据和保护

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-api-template-acceptance-20261027/`

- `lite-acceptance.json`：自动断言三种模式、精确账号、唯一测试模型、原配置哈希与 SQLite integrity。
- `chat-completions-buffered-alt-3.5-lite.json`
- `chat-completions-stream-alt-3.5-lite.json`
- `ui-probe-3.5-lite-passed.png`
- `ui-template-gemini-complete.png`
- `lite-cleanup.json`

原 4200 配置哈希未变化，数据 integrity=`ok`；本轮 4300 与 1435 只用于隔离复验。
凭据仅保留在私有隔离数据中，不写入源码或公开报告。没有向 4200 添加 Gemini 账号，
没有提交、推送、删除数据或覆盖其他线程的改动。
