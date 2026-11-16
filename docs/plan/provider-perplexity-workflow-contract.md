# Perplexity workflow 最终文本观察合同

观察日期：2026-11-12；整理日期：2026-11-13。仅记录用户授权的两轮官网文本问答，不是 Gateway 真实账号调用验收。

## 授权和证据边界

用户批准两次真实提交，可能消耗额度；不读取既有凭据、不上传文件、不购买套餐。官网已有登录状态，没有读取或导出 Cookie。第一问要求记住“蓝色纸船”，页面回答“已记住。”；第二问询问该词，页面回答“蓝色纸船”。两次额度已用完，不追加请求。

首轮 SPA 资源使 CDP 事件缓冲截断，只有页面答案，没有完整首轮 wire。第二轮捕获 `POST https://www.perplexity.ai/rest/sse/perplexity_ask`，HTTP 200、`text/event-stream`。`Network.getResponseBody` 返回 `No data found for resource with given identifier`，改用 `Network.streamResourceContent` 得到 19840 字符的缓冲正文：10 个 message 帧及一个 `end_of_stream` 空对象帧。应用终态出现时尚未收到 socket loadingFinished。

原始正文和私有 token 仅在临时浏览器工具内存中用于字段比较，未落盘；结束后关闭 Network 观察并 reset 工具内存。以下为脱敏人工结构记录，不是可独立复验的原始抓包，测试数据为合成重建，不得伪称完整录制夹具。

## 续接观察（尚未实现）

第二次请求包含 `params.last_backend_uuid` 和 `params.read_write_token`（均36字符）、新 `frontend_uuid`、`query_source=followup`、`followup_source=link`、`mode=copilot`、`model_preference=turbo`、`version=2.18`、`use_schematized_api=true`、`send_back_text_in_streaming_api=false`、`skip_search_enabled=true`、空 attachments。`frontend_context_uuid` 和 `dsl_query` 未发送。

本次所有返回帧的 token 与该请求 token 相同；返回 backend_uuid 不同于请求的 last_backend_uuid。这不证明 token 永久不轮换，也不证明可跨账号复用。Gateway 现有请求仍为单轮 concise/turbo，不因观察改为 copilot，不接受调用方任意私有 token。

## 本次响应形状

- 初始 PENDING 含 `answer_tabs`、`pending_followups` UI blocks。
- 后续 PENDING 的 `workflow_root.diff_block.field=workflow_block`；patch 为根 replace、replace `/steps/0/title`、`/steps/0/completed_at`、`/status`、`/started_at`、`/completed_at` 和 add `/steps/1`。
- 第9个 message 仍为 PENDING，虽 `final=true,text_completed=true` 且已有答案，不是成功边界。
- 第10个 message 同时 `status=COMPLETED,final=true,text_completed=true,final_sse_message=true`，携带完整 `workflow_root.workflow_block`。
- workflow 的 `version=1.0,status=WORKFLOW_COMPLETED`；steps 均完成。第一步骤只有诊断 title，items 空；第二步骤有一个 `type=WORKFLOW_ITEM_TEXT,variant=answer` 的 item，`payload.text_payload` 为 `text=蓝色纸船,chunks=[蓝色纸船],variant=answer,is_streaming=false`。snapshots 为空。
- 同事件还有 answer_tabs / pending_followups；没有 markdown 或来源引用。最后 `event:end_of_stream,data:{}`。

## 有限实现契约

仅完整 materialized 终态中的单个明确 answer 可输出。中间已观察 patch 只校验外壳和操作路径，不应用、不输出诊断标题或 partial、不积累状态树。PENDING、单个 final 标志、EOF、end_of_stream 不能代替完整终态。拒绝混合 markdown/legacy、重复答案、非文本 item、未知 workflow 版本、未完成步骤、text/chunks 冲突与引用扩展。

沿既有 decoder 的4MiB总响应、512KiB帧、4096事件预算；额外限制128 blocks/steps、4096 patches/chunks及256KiB答案。成功仍走同一 Update::Complete 释放 HTTP body，无新任务、重试、账号轮换或持久状态。错误不回显原始事件。未实现通用 JSON Patch、多答案、Research、多模态、引用绑定、会话保存和账号绑定续接。
