# Gemini API 模板复核：目录通过，真实生成待复验

> 后续结论：2026-10-27 17:48，用户授权更换模型后，`gemini-3.5-flash-lite`
> 已通过 UI、非流式与 SSE 验收，并登记完成标记。见
> [最终验收记录](2026-10-27-gemini-api-template-acceptance.md)。下文保留此前失败现场。

日期：2026-10-27。结论：**尚未完成真实调用验收，未登记模板完成标记**。

## 范围与修正

- 保持模板 ID、preset 与显示名 `gemini-api`。
- 修正模板 Base URL 为 `https://generativelanguage.googleapis.com/v1beta`。
  原值缺少版本前缀；本地真实 HTTP 回环合同先出现 404，修正后通过。
  现有生产请求规划器拼接 `/models/{model}:streamGenerateContent`，不自行补版本前缀。
- 创建空服务商时不预置模型或账号。账号导入只读官方目录，不执行批量生成发现。
- 新增固定官方源的 Gemini 目录读取器，使用 `x-goog-api-key`，禁重定向；
  分页 token 仅作为编码后的 query，限制 16 页、累计 2 MiB、2048 条、30 秒总时限。
  仅纳入支持 `generateContent` 的模型，去除 `models/` 前缀并去重。
- 目录管理接口复用管理鉴权、secret grant、账号精确定位和正常 revision 保存流程。
- UI prompt 测试接入现有生产 Gemini dispatcher，不回退到其他账号；60 秒限时，
  有界输出、凭据脱敏、空回复与长度截断均不判通过。不修改 S06 Gemini 执行器。
- 默认自动测试关闭，仅一项无标准答案的连通用例。
  根据本轮 Google 实际 404 推荐，将默认测试模型从 `gemini-2.5-flash` 改为 `gemini-3.8-flash`。
  历史 Rust 协议测试夹具仍保留原模型名，不做无关替换。

## 本地验证

- 前端相关 4 个测试文件、22 项测试通过；最后更改默认模型后 Gemini 4 项再次通过。
- TypeScript typecheck 通过。
- Gemini 官方目录单元测试 2/2 通过：分页、鉴权头、能力过滤、去重、官方源限制、
  cursor 循环、重定向、畸形/空/超大响应和错误脱敏。
- `console_gemini_catalog_contract` 1/1、`console_gemini_probe_contract` 1/1 通过。
  后者覆盖成功、401、429、长度截断、空输出、禁用账号、未知模型及无账号回退。
- rustfmt、`git diff --check`、有效行 checker 31/31 与 ratchet 通过。
  新目录模块 104 有效行，Gemini probe 119；原 console probe 326 → 334。
  没有修改有效行基线。

## 隔离环境和构建

证据根目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-api-template-acceptance-20261027/`。

使用官方 `build-gateway-devtest.ps1 -Edition api -Jobs 4`；源码前后指纹校验通过。
API 构建耗时 5m50s；冻结后 SHA256 校验通过：

`d0f0b7f3635e8a06c4d2c08b2628b8a865abe1f075fd515ce8103d3f169180f0`

冻结位置 `private/gateway.exe`；凭据为 `build-provenance.json` / `build-verified.json`。
API 后端无内嵌管理 UI，验收使用本轮 1435 源码 UI 代理到 4300。
编译后仅更新前端默认测试模型；没有重新出桌面包，也未将旧二进制描述为最终完整新包。

## 真实 UI 和调用证据

1. 正常 UI 创建空池，revision `r1-6f85c8dd2c8c`：0 账号、0 模型、0 路由。
2. 正常 UI 导入用户指定 Key，账号 `gemini-acceptance-20261027`，
   revision `r2-968bd0b447c3`：46 个官方生成目录模型、46 路由。
   `ui-import-no-generation.json` 证明导入时生成请求数为 0。
3. 只测 `gemini-2.5-flash` 的一次 UI 测试返回 404。
   一次同模型官方非流式诊断确认原文：
   `This model models/gemini-2.5-flash is no longer available to new users.`
   Google 推荐 `models/gemini-3.8-flash`；该模型存在于本次目录。
   旧模型普通非流式请求被健康状态拦截，routeAttemptCount=0，不算真实上游调用。
4. 经 UI 将唯一测试模型改为 `gemini-3.8-flash`，revision `r3-ac1b3e61ec07`，
   自动测试仍关闭。一次 UI 测试 60 秒超时；一次普通非流式请求客户端 90 秒超时。
   后端日志出现 ServerError 重试；未重放该请求，最终经隔离 runtime drain 取消。
5. 一次同模型官方非流式诊断返回 HTTP 503 / `UNAVAILABLE`：
   `This model is currently experiencing high demand. Spikes in demand are usually temporary. Please try again later.`
6. **流式验收未执行**：已有上游繁忙和超时证据，停止增加调用。没有扫描其余目录模型。

这不是余额不足特许验收，也不证明实际生成成功。仅确认目录读取可用和真实生成路径已发出请求。
后续只复验 `gemini-3.8-flash` 的精确账号 UI、非流式、流式，成功后再登记完成标记。
现有 Gemini 执行器为聚合后桥接下游 SSE；未来成功也不能称为逐 token 透明转发。
现有 accumulator 对缺失终止原因有默认 stop 行为，本轮未修改或证明畸形 EOF 的严格拒绝。

## 收尾和保护边界

- 仅本轮 4300 runtime drain；等待中的请求最终审计为 `cancelled`。
- 本轮 4300 和 1435 listener 均已关闭；临时浏览器标签页已关闭。
- 4200 原 PID 85612 和 4280 未变，原配置哈希均未变化，SQLite integrity 为 `ok`。
- 隔离账号、私有 Key、原始错误和失败 guard 保留，未复制到 4200，未删除数据。
- 未登记完成标记、未提交推送、未覆盖其他线程改动。

主要证据：`ui-empty-pool.json`、`ui-imported-account.json`、`ui-import-no-generation.json`、
`official-diagnostic.json`、`official-3.8-diagnostic.json`、`final-audits-sanitized.json`、
`final-integrity.json`、`ui-imported-account.png`、`ui-probe-3.8-timeout.png`。

官方资料（本轮联网核对）：
- https://ai.google.dev/api/models
- https://ai.google.dev/gemini-api/docs/api-key
- https://ai.google.dev/api/generate-content

## 用户授权再次尝试（2026-10-27 17:39–17:41，北京时间）

复用原隔离账号与已验证 API 后端，仅重试 `gemini-3.8-flash` 的一次非流式请求。
客户端约 15.1 秒收到 HTTP 503，审计 upstreamStatus 同为 503，原文仍为
`This model is currently experiencing high demand. Spikes in demand are usually temporary. Please try again later.`

审计 `gw-audit-b6d6c1b3-f8f1-4adf-8174-ea8af05c1c60` 确认精确账号
`gemini-acceptance-20261027`，candidateCount=1，routeAttemptCount=3：
一次下游请求触发现有策略内的三次路由尝试，均未切换服务商或模型。
不将其描述为只有一次上游尝试；未追加独立诊断、UI 或流式调用。

结论仍是上游暂时不可用，未获得生成成功，未登记完成标记。
本轮再次对 4300 发出认证 drain，未启动 1435 UI；4200 保持原实例。
证据：`chat-completions-buffered-retry2.json`、`retry2-error-sanitized.json`、
`retry2-audits-sanitized.json`、`retry2-cleanup.json`。
