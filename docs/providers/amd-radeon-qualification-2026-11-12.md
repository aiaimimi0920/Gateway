# AMD Radeon Cloud：资格核实与当前接口边界

核实日期：2026-11-12。工作项 52 保留，未完成接入，不计新增通过项。

## 结论与推进边界

本轮联网读取官方文档，并固定到 AMD-AIM/radeon-cloud-docs 提交
`9ea469e709fbfc7b7906dd56c48cbc0383b7f833`。
旧参考插件的 MIT 许可只涉及代码，不授予上游服务使用或再分发资格。

[官方条款](https://amd-aim.github.io/radeon-cloud-docs/api/terms/)明确：

- Token Factory 仅供试用和开发，不允许 production workload。
- 不允许向第三方提供访问，无论收费与否；明确包含 proxy、gateway、aggregator。
- 不允许通过账号或 Key 池提高群体可用容量；一人一个 API key。
- 不得发送机密或敏感数据；服务记录提示与输出，当前对话内容保留窗口为两天。

因此，**通用共享网关、对外服务及账号池接入存在资格阻碍**。本轮不新增默认可路由
产品卡、不接入共享池、不以免费额度或 MIT 插件许可代替上游许可。
这不等于禁止个人开发用途：仅本人使用的本地开发客户端仍可另行设计，但需要先确认
产品范围和访问隔离，不应将通用 Gateway 配置默认视为已经满足这些边界。
继续通用接入需要覆盖实际用途的上游授权；改做个人开发限定入口也不是整卡验收。
这些条款只针对 Token Factory，不可泛化成对付费 GPU 实例或自建 model service 的结论。

## 已核实合同及旧参考差异

| 项目 | 当前官方合同及实现要求 |
| --- | --- |
| 服务 | China site Token Factory；`https://developer.amd.com.cn/radeon/api/v1`，不是全球 Platform API |
| 身份 | 审核通过账号的 `rc-` Key；Bearer 或 Messages 的 `x-api-key`；没有已核实 refresh grant |
| 身份隔离 | CLI 的 `rcc_`、浏览器 session、自建实例的 `sk-` 均不可混作 Token Factory Key |
| Chat | `POST /v1/chat/completions`；须进一步按官方字段及逐模型能力准入，不假定完全透明转发 |
| Messages | 当前官方已声明 `/v1/messages` 和 `/v1/messages/count_tokens`，旧“仅 Chat”描述不完整；不是 Claude 模型服务 |
| 参数保真 | Messages 会丢弃 `top_p`、`top_k`、`stop_sequences`；`thinking.budget_tokens` 当前拒绝；不能静默降级后称完整兼容 |
| 模型目录 | `GET /v1/models` 返回 `{data:[...]}`，没有标准 OpenAI `object`、`owned_by`、`created`；能力与价格须逐模型解析，不能把语音/OCR 全部当 Chat |
| 用量 | `GET /v1/usage` 的 `daily_cost_*_usd` 实际单位为 pts，非现金，也不是 GPU credits；不能直接导入现金余额或现金定价 |
| 重置 | 当前官方说明北京时间零点；固定参考插件宣称 24h 滚动且 timezone 为 null。尚无本轮真实账号响应，差异保留，不硬编码任一窗口冒充实测 |
| token count | Messages count_tokens 是估算，不用于结算；生成响应 usage 才是该请求用量证据 |

固定参考插件：`aqiu817/magpie-amd-cloud@feb76d9136a74e01b418b7f61f2668b623b6c922`。
本轮在线读取其 `lib/platform.mjs` 和 `lib/usage.mjs`，未复制实现。
后续若获得适用范围，应保留未知额度，不照搬 `Number(null)` 或缺值默认零的映射。

## 证据及验证边界

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-amd-qualification-20261112`。
`manifest.json` 记录七份固定提交文档的原始 URL、HTTP 200 和落盘 SHA256：
overview、authentication、terms、usage、messages、chat-completions、models。
`*.pinned.md` 为 UTF-8 无 BOM 原文，`docs-commit.json` 为官方提交查询结果。
首次猜测 `.mdx` 扩展返回 404；改为实际 `.md` 后读取成功。
首次 overview 快照请求连接失败，固定提交版本随后成功，不把失败当成功。

本轮仅修改资格/范围文档，未修改生产代码，未运行 Rust/UI 回归或构建 EXE。
未读取真实凭据、调用推理、查询账号用量、部署、重启、提交或推送。
公开接口合同核实不是实际账号、原生实现或产品验收通过。
