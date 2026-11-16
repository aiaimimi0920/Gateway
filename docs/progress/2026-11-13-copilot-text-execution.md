# Copilot 单轮文本执行 owner

本块完成 SDK 执行侧，不宣称公共 Chat Completions／Responses 已接通。
已保存 Copilot 账号仍不可路由；真实 CLI、真实账号、流式协议与计费验收仍未完成。

## 实现

`CopilotRuntime::complete_text` 消费整个 owned runtime，接收明确 token、model、单段 prompt
及最大 120 秒的 SDK 等待预算。拒绝空白 prompt、超过 64 KiB 的输入和超出界限的身份字段；
不接受历史、工具、媒体、客户端 session ID 或任意 SDK options。

沿用 Empty、无工具／子代理、权限拒绝的 session 配置。创建前订阅，再使用固定 SDK
1.0.31 的 `send_and_wait` 处理请求关联、根消息、终态和超时，不另写平行事件 waiter。
Gateway 没有重试或重放循环。SDK 自带在 method-not-found 时转用旧 `session.send` 的兼容
协商，本轮仅验证支持 `session.sendAndWait` 的合成对端，不承诺旧 CLI 的能力或行为。

正常返回前等待 owned stop；结果失败也走关闭流程，不把关闭失败包装成成功。
调用方取消整个 future 时，消费进去的 owner 随之 Drop，复用 SDK force_stop，
不生成独立清理任务。该路径不是优雅关闭回执；进程退出需要单独核对。
调用方仍负责启动和整体 deadline，SDK 等待预算不等于启动／生成／stop 总预算。

只返回 SDK 选定的最后一条根 assistant 文本结果，不拼接整轮所有 root 消息；没有 chunk
标记也不能证明此前没有其他消息。缺失、畸形、工具、已标记多分片、秘密回显、超过 1 MiB
的结果及负 token 数均明确失败，不截断、不返回伪成功或空默认值。
`final_message_output_tokens` 仅保留最后消息报告的输出数，缺失为 None；
它不是整次请求用量、现金计费依据或 finish_reason。公共接线必须继续保留未知 usage，
不能补零，也不能把非 HTTP 执行伪造为上游 HTTP 200。

## 验证

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-text-20261113`。

- 首次编译误将 SDK `typed_data()` 的 Option 当 Result，修为 `ok_or`。
- 第二轮 1 项超时测试失败：合成对端没处理 SDK 的 `$/cancelRequest` 通知，日志确认后
  仅补齐夹具协议处理，没有禁用 SDK 取消或弱化测试断言。
- 最终独立 crate **11 passed / 0 failed / 0 ignored**，其中 4 项新文本测试覆盖 17 个
  owned child 场景，包括正常／未知用量、拒绝／缺失／aborted／子代理／错误类型／畸形／
  多片／工具／秘密回显／超限／负用量，以及无效输入、超时和外层取消。
- 连同原 owner／目录回归，最终 21 个合成 child 经 Windows OpenProcess／WaitForSingleObject
  核对全部退出；保留 `process-exit.json`，不只依据 future 返回。
- checker 测试 31 passed，ratchet 通过，5499 文件、governed source >700=0。
- 根工程原 target、jobs=2 的锁定离线 feature 编译通过；记录 111 条既有警告，未扩大清理。
  官方 formatter、scoped diff、UTF-8 无 BOM 和本轮 Cargo.lock 未变检查通过。
- 新文本 owner 81 有效行，文本测试 122 行、文本夹具 34 行，既有合成主程序 100 → 116；
  未增加依赖或更改 baseline。
- 独立只读复核未发现阻断当前最终单条文本原语的问题；错误 Debug／source 链仍须由上层
  脱敏，不能以固定 Display 安全为由直接输出完整错误链。

没有真实上游调用、凭据读取、公共路由解禁、提交、推送、出包或部署。

## 公共接线剩余责任

现有 UpstreamClient 没有可信 Local Key、冻结配置 revision、shutdown 或 caller cancellation；
不能把 token 塞回 HTTP payload 或从用户 metadata/header 推断这些身份。
下一步须在已有 attempt gate 内从 PipelineContext 取得可信执行上下文，绑定 provider、
credential、Local Key 和代次，接入单轮请求校验、SDK owner、canonical response 和审计。
后台 BackgroundTasks 每 Worker 只保留一个 live task，不能将现有目录 owner 冒充并发任务池。
聚合完成后包装 SSE 也不等于 SDK 实时流，需分别验收。
