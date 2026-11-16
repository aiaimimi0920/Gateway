# LobsterAI 普通目录模型直选 Messages（2026-11-11）

本批修复目录中 `apiFormat:anthropic` 的普通模型被错误拒绝的问题。现可直接选择该模型，经固定 Messages 目标完成本地公共推理；不再要求借助 yd 虚拟模型。整体仍为 67 个工作项未完全关闭，本批不等同真实账号或正式交付。

## 改了哪些内容

- 普通目录模型与虚拟模型解析结果共用 Chat／Messages 的准入规则；继续拒绝未知格式、特殊 runtime profile、Kimi、reasoning／媒体及无法无损转换的请求。
- 直选 Messages 在请求内记录目录 selection，供原生流识别、用量口径、现金结算与审计使用；不访问 resolve，不声明虚拟 capability。审计记录 `source:catalog`、modelId、apiFormat。
- sender 返回计划中的响应类型，JSON 和缓冲 SSE 按真实协议解析。这样没有预先写 selection 的直接 runtime 调用也不会误用 Chat reader。
- 共用已有 Messages 编解码、严格结束校验和有界工具聚合。外部 cookie、账号头和管理令牌不进入请求；本人 bearer、固定端点、无重试和无跳转策略沿用。
- 冻结 quote 的模型保持原直接选择模型。原生审计保留 exclusive input 与 cache 分项；公共 OpenAI 和现金账单的副本将 cache read/create 加入输入，公共 Messages 保留原生口径。
- 未知最终用量保持未知。推理已发出后失败或取消，现金 reservation 按既有规则保持 unresolved；消息次数余额退还。不能把未结算当作零费用成功，也没有改变现有现金失败规则。

## 官方依据与 Kimi 后续

匿名联网核对固定提交 `5d6621a0aed22adb53439e290234cc2d64318caf` 的模型配置和代理实现。普通 Anthropic 目录项按其 apiFormat 使用 `/api/proxy/v1/messages`，不要求虚拟 resolve。具体来源、Kimi 请求合同和当前推理历史缺口见 [Kimi 合同核对](../providers/lobsterai-kimi-contract-2026-11-11.md)。

Kimi 仍拒绝；本批没有宣称 Kimi 运行资格、推理历史、多模态或 thinking 控制已实现。

## 验证

最终定向 Rust：**142 passed / 0 failed / 0 ignored，其中名称含 LobsterAI 的 79 项**。命令：

```text
cargo test --locked --lib -- lobsterai protocol::commandcode:: protocol::openai::chat_stream protocol::anthropic::to_anthropic cash_billing:: routing::config::refresh_handoff_tests --test-threads=2
```

编译及测试总计 821.06 秒；使用锁定依赖、2 个 Cargo jobs，未同时启动其他 Gateway Cargo。

新增 8 项测试覆盖三入口 × public stream × upstream Content-Type、两回合函数工具的 ID／参数／结果、无 selection 直接 sender、缓存现金结算与输出上限别名、JSON／SSE 最终用量未知、真实 401 唤醒既有 worker 且仅后续请求使用新 token、非法 JSON／截断／结束后 error、取消的消息及现金余额语义。没有执行请求中改价的并发实验，冻结报价继续使用已有持久化机制。

旧实现的 3 项新回归测试先失败，确认目录直选被拒绝；未放宽测试断言。代码行检查器 31 项、ratchet、定向 rustfmt、UTF-8 无 BOM 及 scoped diff 检查按本批回执核对。测试源码快照和最终 SHA-256 需一致；全工作树来源不作声明。

首次联合回归为 141 passed / 1 failed：新增坏流测试误用只适合成功正文的 collect／unwrap 辅助函数，在预期的 `invalid_lobsterai_stream` 错误处提前 panic。已改为逐帧读取并明确检查错误类别、敏感内容不泄漏、没有成功终态、失败审计和消息余额返还；生产逻辑未因此修改。最终重跑保留同一组联合回归，失败日志与每轮源码快照独立保存。

## 未验证边界

真实账号／订阅／费用与自有身份头接受性、跨请求持久虚拟模型绑定、特殊 profile／Kimi、reasoning／媒体、企业、Server／Redis、独立 feature 构建和原生 EXE 均仍开放。没有读取真实 `.ng`、真实登录、付费调用、出包、提交、推送、部署或重启。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-declared-messages-20261111`。基准 HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；大量继承 dirty edits 保留，`wholeWorkingTreeProvenance=false`。
