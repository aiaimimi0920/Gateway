# Luma LLM Inference 独立渠道合同

2026-11-19 匿名联网核实 [官方 LLM Inference 指南](https://docs.agents.lumalabs.ai/guides/llm-inference/)，HTTP 200。正文、UTC、最终 URL 与 SHA-256 保存在 [来源记录](../evidence/luma/gateway-provider-resume-20261119/sources.json)。没有登录或调用真实推理。

官方明确这是与 Agents 图片／视频 API 分开的产品：销售开通独立 API Key，固定 `https://inference.lumalabs.ai/v1/chat/completions`，Bearer 认证，token 后付费；不使用 Agents 的 `luma-api-*` Key 或 credits，也不消费 App Web／MCP 授权。当前指南列出 `glm-5.2`，该名称不是账号已获模型资格的证明。

Gateway 的独立 preset 为 `lumalabs-inference-api`、profile 为 `lumalabs_inference`、auth mode 为 `luma_inference_key`，编译 feature 为 `line-lumalabs-inference-official-api`。模板只创建空账号池，不填默认模型、不创建路由、不自动发收费测试、不恢复健康。正常账号表单保存静态 Key 与手工模型，沿原 SecretPatch／revision 保存；Keep 绑定原产品及目标，暂存 replacement 也须在提交时复核目标。示例配置为[空池样本](examples/lumalabs-inference.routes.sample.json)。

本地文本范围是 Chat Completions，以及翻译为 Chat 的无状态 Responses／Messages；支持 JSON、SSE 和原生 function tools。不会把 Responses 原样发送到不存在的 `/responses`。拒绝 `previous_response_id`、conversation、background/store 状态、非文本输入及 hosted tools；不提供 models 或余额发现，不猜测官方未公开的 endpoint。请求只向固定官方目标发送精确账号 Key，清除 caller／共享 client 的其他认证头，禁止跟随重定向。

JSON 接收复用 64 MiB 有界正文 owner 和共享 Chat 结构准入；流复用严格 Chat completion guard，要求有效终态，截断或错误尾部不能产出成功完成。收到无效生成后使用固定诊断并禁止重放，token 用量只采用有效上游事实。公共次数预付与退款使用原结算链，不能据此推断真实 Luma 账单；未知 token 保持未知。

默认账号 probe 不发网；用户显式指定账号、模型和 prompt 后，复用同一 sender、原审计及健康 owner。probe 要求完整、非空、无工具／refusal 的短文本；取消释放原请求，不派生持有 Key 的后台任务。管理 grant、模型资格及账号状态继续由原边界校验。

本地定向验证及其结果见 [实施回执](../progress/2026-11-19-provider-channel-expansion-luma-inference.md)。真实销售账号、模型权限、上游限速／账单、Server／PG／Redis、跨进程及覆盖本轮源码的原生包尚待验收；该子批不关闭 R14 整卡，也不抵扣 Magpie 67 项。
