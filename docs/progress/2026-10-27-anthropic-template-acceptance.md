# Anthropic API 模板验收（2026-10-27）

## 结论与授权边界

`anthropic-api` 按用户明确允许的“无余额但调用链可用”标准完成，验收后登记模板完成标记。
内部 template ID / preset 仍为 `anthropic`，不迁移既有服务商、凭据或数据库。
`generationPassed=false`：没有成功生成文本，没有建立成功的 SSE 内容流；完成标记不是账号可用或生成成功证明。

## 本轮修正

- 标准模板显示名与新建默认 Provider ID 改为 `anthropic-api`；固定 `https://api.anthropic.com`，原生 Messages 调用适配器。
- 新建空池无账号、模型和路由；自动测试关闭，手动默认仅 `claude-sonnet-4-6` 和一个无标准答案的连通用例。
- 官方目录读取器使用 `GET /v1/models?limit=1000`、`x-api-key`、`anthropic-version: 2023-06-01`。
  cursor 仅编码为固定源的 `after_id`；禁止重定向；总时限 30 秒，最多 16 页、2 MiB、2048 条，拒绝循环 cursor。
  管理认证、secret grant、精确账号、官方源和 revision owner 沿既有链路，不回退到批量生成发现。
- 修复 Anthropic 通用连通性 probe 的路径为 `/v1/models`，不把 base URL 改成 `/v1` 而破坏 Messages 路径。
- 账号 UI 提示词测试原先被本地 Unsupported 拦截；现在接入 `execute_stream_with_wire` 生产 dispatcher，
  保留精确 payload、原生 Messages 鉴权及传输，不重新选路或回退账号。
  UI probe 限 60 秒、256 KiB，使用现有 Anthropic SSE parser，必须同时有 stop 原因、完成事件及非空文本才 Passed；截断和错误均失败，结果脱敏。
- 官方文档已联网核对 Models、Messages 和认证合同：
  <https://platform.claude.com/docs/en/api/models/list>、<https://platform.claude.com/docs/en/api/messages/create>、
  <https://platform.claude.com/docs/en/api/overview>。`x-api-key` 仍受支持，本轮未升级现有 API version。

## 正常 UI 与精确账号

后端 `127.0.0.1:4300`，源码 UI `127.0.0.1:1435/ui/`，数据为独立证据目录内 `private/runtime`。

- 正常 UI 新建空服务商：revision `r1-3ddd74bd9b9d`，0 账号、0 模型、0 路由。
- 正常 UI 导入用户 Key，账号 `anthropic-acceptance-20261027`，官方目录成功返回 **13 个模型**；
  保存 revision `r2-0015d9be215b`，13 条路由，手工模型 0，无 discovery job、无生成审计。
- 全程只有该服务商、该账号；只调用 **`claude-sonnet-4-6`**，没有批量调用其他 12 个模型。
- UI 固定账号测试于 `2026-10-27T07:20:51.9652798Z` 完成：HTTP 400 / `InsufficientQuota`，
  status failed、callable false、score null；审计 `probe-66605173-2b35-458a-925c-b60840c6a929` 精确归属该账号。
  该 probe 审计沿既有 canonical 入口记录 `openai/chat_completions`，实际上游由 Anthropic dispatcher 发 Messages，不能把审计入口字段误称为上游协议。

## 普通 Messages 入口验收

| 请求模式 | 实际结果 | Gateway request ID | audit ID |
| --- | --- | --- | --- |
| `/v1/messages`，`stream:false` | HTTP 400 JSON / `InsufficientQuota`，812 ms | `6ef9f646-0111-4cd3-8922-94fdb6d1a8b4` | `gw-audit-499df6a2-1fd6-4a82-9cec-5f7e525d0ac7` |
| `/v1/messages`，`stream:true` | HTTP 400 JSON / `InsufficientQuota`，703 ms；上游在 SSE 建立前拒绝 | `891237b6-3794-450c-9f4f-83d2b0e8eeb8` | `gw-audit-c70f4279-980e-4c7a-9eeb-81447ff7033b` |

两次均为同一精确账号、单 candidate、单 route attempt。错误正文为：
`Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.`
没有可用的独立 error code（null），不伪造 quota code；Gateway errorKind 为 `InsufficientQuota`。
外部非流式入口沿既有 Anthropic 强制流式上游聚合路径，并非证明向上游发送了 `stream:false`。

共保留 **3 条真实生成尝试审计**：UI 一次、普通非流式一次、普通流式一次；没有其他真实生成尝试。
为从 UI quota cooldown 进入下一项隔离测试，通过正常 UI 调度关闭/开启一次，每步均确认持久化：
关闭 `r3-ee2fc3f97433`、开启 `r4-0015d9be215b`。未改 SQL、未删除审计，之后不再重置状态或发送请求。

健康语义边界：UI probe 曾记录 `quota_exhausted`/cooling；普通 HTTP 400 审计虽有正确的 InsufficientQuota，
其既有 failureClass 仍为 `client_request_invalid`/client_request，不会重新把账号置入 quota cooldown。
因此调度/UI“可用”状态不能解释成有余额，实际失败记录和未评级结果仍保留。本轮未扩展共享健康分类修复。

## 构建、验证与隔离证据

本轮最终 devtest API 构建耗时 **8m25s**，前后源码 fingerprint 一致；不是正式发布或桌面整包。

- builtAt：`2026-10-27T07:16:59.5267168+00:00`
- source fingerprint：`aa57f1f98ee6ce129b2c8df816985ed0d68621f08ee98e53951503eddfd4bbee`
- EXE：78,405,632 bytes；SHA256 `a449fafab260847f5e749038f9a8e54f3fce0ebee6e2f111c335f68cb7148ddb`
- Rust 定向 **94 passed**（92 单测 + 目录 contract 1 + 精确账号 probe contract 1）。
- 前端相关 3 文件 **18 passed**，TypeScript typecheck、定向 rustfmt、`git diff --check` 通过。
- effective-line checker **31 passed**，ratchet 通过；新增生产模块分别 88/178 有效行。
- 暂停前两次构建虽编译完成，均因并行源码变化被门禁拒绝，未用作真实验收；早期 Rust 夹具编译失败和 stream 断言失败均如实保留在停点记录，修正后重新验证。
- 用户要求其他线程继续时曾暂停，之后用户明确恢复。恢复时重建数据保护基线，未覆盖原基线记录。
- 完成标记与本文在实际验收后追加；冻结 API EXE 不含 UI 标记，不声称已更新当前桌面实例。

证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-anthropic-template-acceptance-20261027/`。
主要证据：`ui-empty-pool.json`、`ui-imported-account.json`、`ui-import-no-generation.json`、`ui-test-results.json`、
`messages-buffered.json`、`messages-stream.json`、`audits-sanitized.json`、`integrity.json`、`r1/build-verified.json`。
用户 Key 仅在隔离正常账号数据中保存，不写入源码和公开报告。格式检查不是完整 Key 字符串相等验证。
SQLite integrity 为 ok，恢复验收后的原配置哈希不变；4200 后端 PID45580 和另一个4200监听 PID4280 未操作。

未验：充值后的成功生成、SSE 文本/终止事件的真实上游成功路径、其他12个目录模型、Chat Completions/Responses桥接、工具调用、长期稳定性及桌面整包。
未提交、未推送、未部署当前4200。隔离进程与界面的最终关闭结果见 `cleanup.json`。

## 最终收尾

完成标记登记后，重新运行前端 18 项、typecheck、checker 31 项和 ratchet，全部通过；最终 `git diff --check` 通过。
源码与本文通过 UTF-8 无 BOM 和 Key 模式扫描；Markdown 仅检查编码和敏感信息，不参与源码有效行计数。
已在刷新后的隔离 UI 确认 anthropic-api 蓝色完成标记及“新调用流程已验收”可访问名称，保存 `ui-template-complete.jpg`。
截图中默认 Provider ID 为 anthropic-api-2，因为首个服务商已经存在；没有提交创建第二个服务商。
最终只读复核仍为 3 条生成尝试审计、SQLite integrity ok、原配置哈希不变。
于 `2026-10-27T07:31:53Z` 确认 4300 正常 drain 退出、1435 helper 停止、自建浏览器页关闭；两个 4200 监听 PID 未变。
保留隔离数据、凭据和全部证据；`acceptance-status.json` 已记录用户批准的额度受限完成状态，旧 RESUME.md 仅保留为暂停时历史记录。
