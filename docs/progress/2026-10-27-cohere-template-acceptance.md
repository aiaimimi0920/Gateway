# Cohere 模板复核与隔离验收（2026-10-27）

## 当前状态

已完成标准流程：官方合同核对与修正、正常 UI 创建空服务商、正常 UI 导入账号、隔离 4300
精确账号 UI 测试，以及 Chat / Responses 非流式、流式四种普通调用验收。
全部真实验收及审计对账通过后，才在 `2026-10-26T20:43:42Z` 登记 `cohere-chat` 的
`flowVersion: 1` 完成标记。目录发现不等于调用成功或账号权限。

## 方案与边界

- 沿 `cohere-chat` preset、Cohere v2 编码器和标准 API 传输，不新增独立网络执行器。
- 删除模板中的单一 seed，先创建显式空凭据池，再经正常 UI 导入账号。
- 官方目录为 `GET https://api.cohere.com/v1/models?endpoint=chat&page_size=1000`，使用
  Bearer API Key；解析 `models[].name`，保留 Chat 且未明确废弃的条目，支持分页。
- 目录按固定官方来源绑定；不跟随重定向，限制 16 页、总计 2 MiB、2048 条、30 秒，
  continuation token 只作为编码后的查询参数，检测循环；失败不替换 last-good。
- 接入正常账号双栏、精确账号刷新与 revision/CAS；目录不是调用成功或账号权限证据。
- 站点健康默认启用 `cohere-status`；自动生成测试关闭，显式测试只选择
  `command-r7b-12-2024` 和一个无标准答案的连通用例，不批量调用目录。
- 原生上游使用 `/v2/chat`；独立解码 `message-start`、`content-delta`、工具增量及
  `message-end`，不再依赖可被关闭的 Accio 解码器。缺失终态、错误或空白输出不伪造成功。
- 读取 `usage.tokens` 的实际 tokens，不拿 `usage.billed_units` 当实际 token 数。
  Responses 输出上限转换为 Cohere `max_tokens`；不会把 Responses 桥接称为官方原生接口。

## 实际发现与修复

- 原来的原生 SSE 解析未正确接收 Cohere 连字符事件，含 `OK` 的官方格式响应也会得到空文本。
  已接入独立解码器，不依赖可关闭的 Accio feature；工具增量、UTF-8 切分、实际 tokens、缺终态和
  错误路径有本地合同保护。固定 4096 字节处理批次，单帧 256 KiB、输出队列 2 MiB，避免行为依赖 HTTP 分片。
- 分页客户端追加 query 曾覆盖旧查询，已改为 URL query pairs，保留 Chat 过滤和分页大小。
- UI prompt test 入口原先拒绝 Cohere。已接入精确账号的生产 dispatcher 与同一解码器，
  不路由、不回退、不自动重试；限制 60 秒与 256 KiB，并沿既有审计、健康恢复 owner 记录结果。
- 普通非流式强制流聚合曾发送 `stream: true` 却携带 JSON Accept，真实请求返回
  `Invalid or incomplete Cohere response`。Responses 流式也可能透传入站 JSON Accept。
  已在 Cohere 发送层强制 SSE 协商，纯回环捕获验证四种模式全部使用 `text/event-stream`。
- Responses 路由原先没有声明 Cohere Chat 兼容族，已在对应端点接入桥接，不扩大到其他端点。

## 正常 UI 流程与真实验收

1. 正常 UI 选择 Cohere，创建 `cohere` 空池：revision `r1-79645bd8dbb5`，账号、模型、路由皆为 0。
2. 正常账号表单添加 `cohere-acceptance-20261027`，点击“拉取模型”后保存：revision
   `r2-f4204176fa35`，官方目录 17 项、手动栏为空，账号池、服务商池、精确路由均为 17。
   Key 回读一致，导入阶段生成审计为 0，自动生成测试始终关闭，站点健康默认开启。
3. 在最终冻结的 API devtest r3 上，正常 UI 对该账号执行一个无标准答案的连通用例。
   `2026-10-26T20:41:14.4948033Z` 返回 `OK`，passed，1035 ms，gradedCount=0、score=null。
4. 四条普通请求均使用这个精确账号与 `command-r7b-12-2024`，限制输出 32 tokens：

| 入口 | 模式 | HTTP | 完成 | 回复 | 客户端耗时 |
| --- | --- | --- | --- | --- | --- |
| `/v1/chat/completions` | 非流式 | 200 | stop | OK | 500 ms |
| `/v1/chat/completions` | 流式 | 200 | stop 与 `[DONE]` | OK | 422 ms |
| `/v1/responses` | 非流式 | 200 | completed | OK | 437 ms |
| `/v1/responses` | 流式 | 200 | `response.completed` | OK | 391 ms |

四条普通请求各尝试一次，无隐藏重试；最终五项真实验收均通过。
每条成功请求返回的实际 token usage 为 500 input + 4 output = 504，不以 billed_units 或估价代替。
只验证了 17 项中的一个模型；其余 16 项保持目录发现状态，没有批量试调用，也不据此宣称免费或有额度。

## 失败保留与调用对账

- 第一次 UI 测试被本地 unsupported gate 拒绝，没有发送生成请求，原始证据保留。
- 修复 UI gate 后的 r2 UI 生成成功一次；随后旧普通非流式请求失败一次，现有重试器内部尝试 3 次。
  此失败请求 ID 为 `3285c87c-2474-42bd-b13f-9d9f6cdab78c`，审计与 started guard 均保留。
- 其后诊断全部使用合成凭据和本地回环，直到本地四模式及精确 probe 合同通过才继续真实验收。
- 最终共 7 条网关生成请求审计：2 次 UI 成功、4 次最终普通请求成功、1 次历史失败，记录的上游尝试合计 9 次。
  这不是只有 5 次调用，也不是 9 次均生成成功或全部已计费的证明。
- SQLite integrity=ok，受保护的当前配置 hash 全部不变；隔离 4300 的账号、目录与路由保持原 revision。

## 构建、检查与交付边界

最终运行产物为官方 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4` 构建并冻结的
`private/gateway-r3.exe`，81,190,400 bytes，SHA256：
`aa107335f66beb62a21195b0771d94b9c7173e6a5ab72323fde539c086b29b70`。
构建 source fingerprint 为 `edd7ec2e0f54a4590ed881373d0e263c2bc72d2fde2c124d70e586e80adf7b01`，
builtAt 为 `2026-10-26T20:40:02.7032716+00:00`。该二进制早于验收后登记的前端完成标记，
不宣称标记已嵌入该 API 产物。

本地 Cohere 相关单测、目录重载与路由投影、管理目录合同已通过；新增公共 relay 四模式合同 4/4、
精确账号 probe 合同 1/1 通过。前端相关 9 文件在登记前 99 项通过，登记后含完成标记回归的 100 项全部通过；
TypeScript、scoped rustfmt、有效行检查器 31 项、ratchet、44 项 manifest 验证通过。
最终源码有效行、secret scan、UTF-8 无 BOM 与 scoped diff 检查记录于证据目录。
仅重启本轮 1439 UI helper 并 reload 后，已在真实界面确认 Cohere 蓝色完成标记；没有再次建池或生成。

当前 4200 实例及数据未修改、未重启；没有向其导入 Cohere，没有生成新桌面手测包，也没有提交或推送。
仅临时 4300 后端及 1439 UI helper 用于验收，完成后停止并保留隔离数据与失败证据。
真实工具调用、其他模型、全部 Cohere 参数及长期稳定性不在本次验收范围。

官方资料均在本轮联网读取：
[List Models](https://docs.cohere.com/reference/list-models)、
[Chat](https://docs.cohere.com/reference/chat)、
[Chat Stream](https://docs.cohere.com/reference/chat-stream)。

证据根目录：`linshi/gateway-cohere-template-acceptance-20261027/`。重点文件为
`acceptance-status.json`、`audits-sanitized.json`、`ui-test-r3-success.json`、四个 `*-r3.json`、
`build-verified-r3.json`、`integrity.json` 和 `ui-final-request-list.png`。
Key、管理与调用 token、原始响应及隔离运行数据只保存于私有子目录。
所有 started guard 保留，恢复时先对账，不直接重放。
