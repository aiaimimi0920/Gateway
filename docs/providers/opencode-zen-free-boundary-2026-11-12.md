# OpenCode Zen Free 当前接入边界

2026-11-12 联网打开旧官方 Zen 文档地址后，跳转至 [OpenCode Console Models](https://opencode.ai/docs/console/models/)。当前 Endpoints 明确区分：免费层聊天模型要求 OpenCode，其他客户端使用付费模型或 Go。

因此工作板的 **OpenCode Zen Free** 不能用付费 Console API、Go 或其他平台同名模型代替完成；不通过伪造 OpenCode 客户端身份／设备信息来绕过该条件。现有“客户端资格／地域／数据条款独立约束”继续有效，现在补入具体官方依据。

这不等于所有 OpenCode API 都不可接入：同页明确 Console 可供其他 coding agent 使用，并列出对应端点。但这些是不同的费用／产品范围，需要单列，不能自动把原免费卡缩成付费 Key 集成。

状态：`external_client_restriction_confirmed`，不是完成，也不代表永久不可接入；官方条款或获准接入方式变化后重新核实。本批未调用推理、未创建账号或 API Key、未修改任何客户端身份头。全范围计数保持不变。
