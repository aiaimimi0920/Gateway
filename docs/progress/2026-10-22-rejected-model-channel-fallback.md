# CLI 工具请求的模型通道拒绝切换修复

## 根因

请求 `47994627-165e-4cc7-a02f-1d5a3c8873ca` 的六个候选已包含 ChatGPT，
但仅实际尝试 `custom-api-provider-5`。其 HTTP 503 携带 `model_not_found`
和 `No available channel for model ... (distributor)`。
HTTP 分类原本允许切换；含工具请求的单次发送预算随后耗尽，保留上游错误但改为 Abort。
因此简单无工具请求成功，并不能覆盖 CLI 请求。

## 修复边界

- 仅 Responses / ChatCompletions 在外层 `AttemptError::Next` 边界处理。
- 精确匹配 503、结构化 `model_not_found` 及 distributor 的通道拒绝文案后，
  授予一个下一候选发送额度；不清零实际次数，不超过六次总上限。
- 不改变同候选重试策略、账户授权、候选顺序、截止时间或已停止状态。
- 网络结果不明、普通 5xx、无效输入、非会话创建、已交付流不因此获得重放许可。
- 本轮不扩展到其他错误格式或任意 adapter 的同文合成错误。

## 验证

- `pipeline_send_runtime_contract`：31 项通过，包括新增真实 loopback HTTP
  流式/非流式拒绝切换，以及带 tools/tool_choice 的 `/v1/responses` 完整流。
  两个上游各收到一次请求，Responses 收到 `response.completed`。
- `request_budget_contract`：12 项通过。
- `request_budget_rejected_channel`：5 项通过。
- checker 测试：31 项通过；有效行数 ratchet 与 scoped diff whitespace 检查通过。
- 新增预算测试曾错误地在消耗最后槽位后才进入顶层 run，已改为模拟真实调用顺序，
  最终复跑通过；运行期间旧编译快照曾导致缺少新方法的编译错误，最终源码编译已通过。

未执行全量回归，未更改正式数据库、路由优先级或运行进程。
本次生成 debug 编译产物，没有生成或部署新的 release；运行中的旧版本尚无此修复。
