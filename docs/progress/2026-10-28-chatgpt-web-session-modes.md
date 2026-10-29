# chatgpt-web 浏览器依赖与续期对照测试

## 范围

使用前一轮已修复的 web devtest 后端，用户授权的同一网页账号和 `gpt-5.4`。仅通过独立 4300 数据目录及正常管理接口创建服务商、导入凭据并进行定向测试；不修改或停止 4200。本轮不修改产品源码。

## 结果

| 场景 | 结果 |
| --- | --- |
| 仅 access token 和真实到期时间，冷启动，浏览器禁用 | 上游浏览器挑战，返回 503；不视为调用成功 |
| 无头浏览器辅助，故意不填到期元数据以触发 preflight | 首次在 prepare 遇到 SendRequest 网络错误；一次重试后 buffered/SSE 均为 HTTP 200、文本 OK、正常结束 |
| 先由无头 worker 准备材料，退出浏览器；将材料正常导入全新目录，再禁用浏览器 | buffered/SSE 均为 HTTP 200、文本 OK、正常结束 |
| 对比 worker 前后的 token 与实际有效期 | token 相同，JWT exp 相同，无 refresh token；没有证明真实续期 |

严格纯 HTTP 测试同时设置 `GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled`、`CHATGPT_WEB_REVERSE_REQUEST_TIME_BROWSER=false`，并将 `CHATGPT_WEB_BROWSER_NODE_BIN` 指向不存在的测试路径。不是仅关闭发送阶段回退。新建测试数据不复制旧 runtime 缓存。

无头测试显式设置 `CHATGPT_WEB_SESSION_WORKER_HEADLESS=true`；进程只读观察确认测试临时 profile 的浏览器根进程带 `--headless`。Chromium 子进程并非每个都会重复该启动参数。

## 会话材料与实际到期时间

源文件仅包含 accessToken 和网页响应 expires，没有 refresh token 或导入的登录 Cookie。

- JWT exp：2026-11-06T03:46:15Z。
- 网页会话 expires：2027-01-25T20:33:49.459Z。
- 两者不是同一有效期，不能拿后者延长前者。
- 无头材料探测成功，账号探测 HTTP 200，目录返回 9 个模型；没有批量调用这些模型。
- worker 返回 token 与输入相同、exp 不变。返回 Cookie 和辅助材料不等于重新签发了登录凭据，也不证明可长期无人值守。

纯 HTTP 的准备材料包括同一 token 的真实到期时间、Cookie header、User-Agent、device/session 标识和 worker 返回的 bootstrap 元数据；Cookie 通过 secret grant 和 secret patch 导入，不在普通配置 JSON 内嵌或日志中输出。

## 精确调用结果

- 无头辅助重试：buffered 25906ms，SSE 25547ms，均返回 OK。
- 准备材料后严格纯 HTTP：buffered 7500ms，SSE 6469ms，均返回 OK。
- 两种模式均检查 finish_reason，SSE 另检查 `[DONE]`。
- 上述耗时是本次样本，不是长期性能承诺。
- 没有验证跨网络地址、跨机器或过期后仍可复用这些材料。

## 发现的元数据兼容性问题

当前 `SessionAuthConfig` 的时间解析器不能正确处理本次使用的无小数秒 `+00:00` 时间形式，将其视为未知并触发保活。将同一真实时间规范化为 `Z` 后，纯 HTTP 请求越过前置保活检查并到达上游。这是现存兼容性问题，不是 token 已经过期；本轮仅规范化隔离测试输入，没有修改生产解析器或伪造未来时间。

## 证据与剩余边界

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-web-20261028/`。

- `pure-http-z-relay-result.json`：冷启动挑战。
- `headless-retry-result.json`：无头辅助双模式成功。
- `headless-material-report.json`：token 未更新、有效期未延长。
- `pure-http-prepared-relay-result.json`：准备材料后纯 HTTP 双模式成功。
- 各模式 `metadata.json` 和 `runtime.log`：策略与运行路径。

准备材料测试前的几次本地配置提交被接口校验拒绝，分别为 headers 值类型、secret grant 缺失、敏感字段应移出普通 JSON；均在发送上游调用前失败，修正测试脚本后通过正常 secret patch 完成导入，没有绕过校验。

结论：已验证按需浏览器初始化加纯 Rust HTTP 调用的可行性，以及不显示窗口的辅助流程。自动续期尚未验证，缺少实际刷新凭据，不能将保活成功登记为续期成功。没有新桌面出包或模板全面完成标记。
