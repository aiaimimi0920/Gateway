# chatgpt-web 运行链修复与定向实测

## 范围和结论

使用用户授权的现有 ChatGPT 网页会话，在独立 4300 数据目录通过真实 Rust 网关入口完成单账号、单模型非流式及流式调用。两种模式均返回 HTTP 200、文本 `OK` 和有效结束标志。没有切换为 Codex OAuth，没有修改 4200 配置或停止其进程。

这份记录确认本轮运行链修复，不替代服务商模板的完整 UI 验收、模型目录验证、默认测试策略整理或正式发布。

## 修复

1. worker 不再把 `Accept-Language` 列表直接作为浏览器 locale。配置 owner 提取并规范化一个 BCP 47 标签，跳过通配符、无效值和 q=0 项；保留原 HTTP 语言头。限制候选输入长度，异常回退到既有默认值。
2. Rust prepare POST 不再发送空 body。实测无 body 返回 `Field required`，空对象返回 `Invalid conversation body`；最终 prepare 与随后 conversation 使用同一份已打包消息体，并保留相同 turn trace。缓冲和流式两条入口均已接线。

未修改账号令牌、挑战求解算法、浏览器策略、路由隔离或其他服务商实现。

## 回归证据

- 新增 worker 回归先失败（7 passed / 1 failed），错误为 locale 格式不合法；修复后的相关 Node 套件 31 passed。
- prepare JSON 断言先失败，证明旧请求 body 为空。最终 Rust `upstream::chatgpt::execution` 40 passed，包含缓冲、流式和响应体边界测试。
- effective-line checker 31 passed；ratchet、Rust 定向 rustfmt、Node 语法检查及 `git diff --check` 通过。
- 本轮涉及源码/测试有效行数均不超过 500：worker 467、configuration 147、生命周期测试 80、新 locale 测试 19、Rust execution 193、transport 172、execution tests 428、body targets 101。保留测试夹具原有 `call_flow` 改动。

## 真实调用证据

- 后端：`tools/build-gateway-devtest.ps1 -Edition web -Jobs 4`，最终构建成功，源码指纹校验通过。存在工作区既有 unused/dead-code warnings，不为本次修复扩大清理范围。
- EXE：`target/devtest-editions/web/gateway.exe`。
- SHA256：`a9b1386c66a9d2eaa78c783c4b265421469f2418e1c107d2d3d6a9eba1cb31dc`。
- 入口：`POST http://127.0.0.1:4300/v1/chat/completions`。
- 精确服务商/账号：`chatgpt-web` / `web-session-test`，独立数据内仅此一个服务商和账号。
- 模型：`gpt-5.4`，沿用 preset 默认值；本次已得到实际返回，不代表目录内其他模型已验证。
- 最终非流式：HTTP 200，`OK`，finish_reason 有值，31297ms。
- 最终流式：HTTP 200，增量文本拼接为 `OK`，finish_reason 有值且收到 `[DONE]`，25797ms。
- 上游路径日志：requirements、prepare、conversation 均为 200，conversation 返回 `text/event-stream`。

本轮先进行了故障定位请求和状态检查，再补充严格内容/终止事件验收；没有遍历模型目录。测试临时 Key 已撤销，测试网关退出。浏览器 worker 使用自身清理流程，不终止用户浏览器。

脱敏结果和原始本地运行证据位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-web-20261028/`，重点文件为 `rust-relay-final-result.json`、`rust-relay-final-runtime.log`。凭据及隔离数据不纳入仓库。

本轮只构建用于隔离验收的 web devtest 后端，没有制作新的桌面交付包，也没有登记模板全面完成标记。
