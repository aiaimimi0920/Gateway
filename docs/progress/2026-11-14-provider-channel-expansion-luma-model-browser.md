# Luma 模型浏览：已保存账号到模型与参数

日期：2026-11-14。承接已通过真实验收的本地授权刷新流程；不再重复注册、授权或刷新。本批完成创作前的模型浏览入口，不声称已经实现提交生成、取消或费用结算。

## 用户可走通的本地流程

已保存的 Luma MCP 账号 → 明确点击“读取模型” → 选择返回的模型 → 查看公布费率 → 明确点击“读取模型参数”。没有默认模型或推测价格，未知费率显示未知，不解释为免费。不自动读取模型，不把工具目录中的工具名当模型名。

- `POST /v1/internal/gateway/console/luma-mcp-models` 只支持 `list` 和 `describe`；固定调用 `luma_model_list`／`luma_model_describe`，不提供通用工具执行入口、任意 endpoint、session 或 token 输入。
- 复用已有账号材料、敏感访问确认、管理身份与配置 revision 前后校验、并发名额、固定 MCP transport、累计响应限制和会话 DELETE 清理。只读操作不写模型健康、不启用 disabled 推理池、不修改账号。
- list 校验来源、模型 ID、重复项及基本字段；describe 绑定原模型 ID 和 image/video 类型。支持 MCP structuredContent 或单一 JSON text，拒绝错误工具结果、资源引用和非 JSON 正文；不执行供应商说明。
- UI 切换账号、权限、配置或连接即废弃旧数据并取消旧读取；错误正文不回显。不自动重试，也不因敏感权限后来到达而自动调用。
- 当前界面展示模型、公布费率和参数 schema；尚未提供生成参数表单、任务提交或产物预览。模型列表是 session 创建前的账号目录，不能保证未来 session 的最终权益。

## 证据与验证

协议依据为 11 月 13 日已经获准取得的官方工具目录 `tools-directory.json`，本批没有重新联网调用真实账号模型工具。调用真实模型列表／详情的权限已单独询问，未将一般“继续推进”扩大成生成或消费批准。

- 新界面测试先以缺失“读取模型”按钮失败；实现后相关前端 **3 文件、32 项通过**。初次绿色检查中一项使用了不受 matcher 支持的断言，改为精确 JSON 文本断言后通过；未放宽产品行为。
- TypeScript typecheck 通过。
- Rust MCP 定向测试 **32 passed**；模型管理接口合同 **2 passed**。覆盖固定 tool、JSON text、上游错误无重试、会话清理、模型返回绑定、重复模型拒绝、输入拒绝和敏感权限。
- checker 测试与有效行 ratchet 通过；新增源码均低于 250 有效行。只读独立审查未发现具体阻断缺陷。
- Web devtest 包构建与两处包完整性检查通过：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-luma-models-20261114`。
- `gateway.exe` SHA256：`422f61d391818bd3a7f97dde09c72869a02423e244060136e59f293f39df04de`；实际 devtest 构建耗时 14m50s。本包不是正式 Release，也不包含独立桌面 launcher。
- 最终包新空数据目录启动成功（SQLite ready）；新模型 API 对陈旧 revision 返回预期 `409 console_revision_conflict`，证明原生路由已接线且在上游请求前拒绝。随后正常 drain，未读取既有账号、未发送真实 tools/call。
- 证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-models-20261114`，含 `rust-tests.log`、`frontend.log`、`typecheck.log`、`checker-tests.log`、`ratchet.log`、`package.log`。本文在包构建后追加，不据此重新编译或更改已有包。

## 下一用户流程与未验边界

下一条是明确选择模型／prompt／额度上限 → session_open → generate_image → 持续查询同一个 run → 展示结果并查询实际用量。根据已有目录，等待超时不应重新 generate；省略 workspace_key 会落入团队默认 workspace，不能充当隔离；run_await 没有实际费用字段，不能按成功状态臆造账单。

生成任务尚缺正常持久任务接线、提交后响应丢失处理、结果与费用 UI；不得将本批只读 tools/call 开放为任意生成调用。真实模型读取、真实生成／取消／费用、完整网页点击、Server／跨机器仍未验收；不减少供应商剩余计数。
