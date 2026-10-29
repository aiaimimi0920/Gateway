# Codex Key 复用与 ChatGPT 真实目录修复

## 已确认原因与修复边界

手测实例 `.ng-devtest-4300` 没有 serving Key，而两个 Codex HOME 使用的现有 Key 属于旧 `.ng`。
因此此前新包的 `/v1/responses` 在入站鉴权阶段返回 401，不是那次 ChatGPT 推理返回 401。
本次保留默认 4200、两个 HOME 的配置、旧实例及旧包，不关闭鉴权，不复制整个数据库。

新增显式、Secret Grant 保护的 local unlimited Key 导入操作：保留真实 token、owner/project/tenant、expiry、metadata，
在目标目录重新 seal，并在单 SQLite 事务内保存。普通创建和轮换仍随机生成 token；重复导入冲突，不覆盖旧记录。
实际复用前，操作脚本必须核实来源 Key 仍 active、未过期、无限额且无时段限制/未结算请求；不迁移预付费或现金账务。

原“现有 Access Token”入口只登记声明模型，没有获取完整目录；catalog 的 0.154.0 版本还落后于本机 0.155.1。
本次新增官方目录专用 API，手动导入在保存前查询；存量账号在编辑窗口点击“刷新模型目录”，再正常保存。
存量请求只接受 credentialId，不允许覆盖地址、token 或 Account ID。请求禁止重定向，网络前后验证 grant。
客户端核对会话、grant、草稿及 revision，关闭/切账号/失去授权时丢弃迟到结果。

目录普通模型加法式注册，隐藏/非 API 模型不自动加入，手工声明模型保留；不填造 `verified_models`，
不把“目录返回”当作“逐模型生成成功”。旧 CPA 专属参考 profile 不变。

## 本轮开发侧验证

- 回归先红：存量 ChatGPT 刷新没有发出目录请求；修复后转绿。
- Key 单元测试 14 项通过，含导入、重复拒绝、重启、scope、seal 失败回滚，以及既有轮换/额度/启停行为。
- Codex catalog 单元测试 2 项通过，含元信息、版本身份及禁止重定向。
- 新管理 HTTP 合同 2 项通过：grant、禁止覆盖、非 Codex 账号、非法材料、group 限制及响应不泄密。
- 原生 Codex fidelity 合同 4 项通过。
- 前端定向测试 5 文件 28 项通过，含新 token、失败不保存、关闭/授权失效取消、存量编辑及手工声明保留。
- TypeScript typecheck、scoped rustfmt、PowerShell AST、行数 checker 31 项及 ratchet、`git diff --check` 通过。
- 现有编译 warning 未顺手清理；没有跑全量/正式发布/批量上游测试，也没有提交或推送。

## 规模与交付证据入口

有效行：`access_keys.rs` 297→319；`codex_client.rs` 90→113；`CredentialDialog.tsx` 361→389；
`useAccountDiscovery.ts` 65→77；`ConsoleGlobalDialogs.tsx` 249→251。新责任独立成小模块，无超限豁免。

## 真实 Codex 验收补充：终止事件与 HTTP EOF

第一轮使用原 Key 的真实 Codex 0.155.1 已输出 `OK` 和 `turn.completed`，但审计记录为
`stream terminated before completion (Some(Cancelled))`。日志显示取消发生在 helper drain 之前：
客户端收到 response-level terminal 后主动停止读取，而 `TrackedStream` 原先只在 HTTP EOF 标记成功。

为 Responses 入口显式启用有界、字节不变的 terminal observer。只有完整且类型/状态一致的
`response.completed` 或 `response.incomplete` 允许随后 Drop 按已完成处理；item/text done、
单独 `[DONE]`、错误、冲突、观察超限不构成证明。传输错误仍优先；其他协议不隐式启用。
观察器不预读、不起后台任务，复用原 64 MiB/65,536 行帧界限，不改变 HTTP/SSE 转发字节。

新增测试覆盖 terminal 分片及大帧、伪终止/错误/超限、真正取消、错误后回调一次，以及非 Responses
原行为。相关文件有效行：`stream.rs` 146→175、`streaming.rs` 425→428；三个新文件分别 99、78、99。
回归先红（原实现将 terminal 后的关闭判为 Cancelled），修复后 `upstream::stream` 定向 47 项全部通过，
包括原有 usage、归档、类型化错误和 observation 测试。最终以新包真实 Codex 回执确认审计闭环，
不用第一轮客户端成功替代审计验收。

快速桌面包仍走 `package-gateway-devtest.ps1 -Jobs 4`，新目录不覆盖旧包，不携带 `.ng` 或真实 Key。
打包、目标目录备份/导入、真实 catalog、重启及有界真实 Codex 的最终结果保存在：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-diagnosis-20261025/`

- `build-result.json`：首包路径和构建退出码。
- `final-build-result.json`：包含 terminal 修复的最终新包路径和构建退出码。
- `fix-runtime-result.json`：真实目录、原 Key 认证/重启、账号保留、Codex 实际模型和结果、HOME 配置不变。
- `codex-acceptance-*.json`：各轮有界原生 CLI、同模型请求审计、数据库与正常 drain 的独立回执。

最终出包与真实 Codex 回执均成功前，不视为交付验收完成。私有备份和原始日志仅放用户私有数据目录，
公开回执不含明文 token、token 摘要或账号密钥。
