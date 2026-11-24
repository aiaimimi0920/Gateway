# Mistral Studio / Vibe 基础调用

新建时使用 `mistral-studio-api` 或 `mistral-vibe-api`，分别录入对应产品的 API Key 和已授权的模型 ID。两个模板创建独立空池，不导入旧池凭据，也不启动自动测试。已有 `mistral-api` Provider 保持原配置和 Studio 调用行为。

两种入口复用 Mistral 的 `POST https://api.mistral.ai/v1/chat/completions`、Bearer 认证和 JSON/SSE。Vibe preset 仅在配置为官方 HTTPS `/v1` 时附加实测的 `User-Agent: mistral-client-python/Mistral-Vibe/3.1.4`，流式调用声明 `Accept: text/event-stream`。源码快照 `e76235b203b8965af584e88e69284bc704c31a1f` 中的 `Mistral-Vibe/3.1.4` 是传给 SDK 的值；官方 CLI 3.1.4 发出请求时 SDK 添加前缀。

客户端标识不授予 Vibe 权限或额度。真实 Vibe Key 可调用的模型、配额及计费以 Mistral 实际响应为准；本轮不请求真实账号。

## 本地线级对比

`tools/mistral-vibe-wire-capture.py` 使用安装了 `mistral-vibe==3.1.4` 的 Python 环境执行，传入 `linshi` 下的隔离目录和官方 `vibe.exe` 路径；传 `--gateway-test <gateway-lib-test.exe>` 对比同一 CLI 请求的传输重放及 Vibe 通过 Gateway 公共入口的上游请求，另传 `--codex <codex.exe>` 增加公共 `/v1/responses` 路线。接收器不转发外部流量，只使用假 Key 和进程内临时 CA；不安装系统证书。`comparison.json` 区分 HTTP 版本、头名顺序和大小写、各头值、原始请求体字节、解析后的 JSON 字段；不比较 TCP/TLS 字节。

官方 Vibe Chat 出站使用 HTTP/1.1、`Accept-Encoding: gzip, deflate, br`、`Connection: keep-alive`，以及紧凑 ASCII JSON；`Content-Length` 由实际发送的字节得到，不复制入站值。Vibe 入口不再自动补 `stream_options`，但保留显式传入的值。仅向官方 `POST /v1/chat/completions` 发送 `X-Affinity`：已有合法的请求头优先，否则取 JSON 中已有的 `metadata.root_session_id` 或 `metadata.session_id`；缺失或不是 UUID 则省略。

同一份输入的传输重放与独立 CLI 调用是两种不同的验证。独立 Vibe 会话有各自的会话数据，而 Codex 有自己的系统提示、工具定义和历史，不应改写成另一客户端的内容。因此 `Content-Length` 可以在同一请求体重放时相同，但不应在请求体不同时强制相同。

本地验收（Vibe 3.1.4、Codex 0.155.1，均使用假 Key）：

- 固定输入重放：HTTP/1.1 方法、路径、所有请求头值（测试 Bearer 值脱敏）、头名顺序和大小写相同；body 字节相同，`Content-Length` 均为 `42601`。Gateway 只在解析值未变时保留 `messages`、`tools` 原键序。
- Vibe 独立运行经 Gateway 公共 Chat 入口：SSE 成功，原 `X-Affinity` 保留。相对于独立直连样本只剩 `Content-Length` 和 `X-Affinity` 的头值不同。夹具所用账号未声明模型原生工具能力，Gateway 的现有文本工具桥改写 `messages`、`tools`。
- Codex 独立运行经 Gateway 公共 Responses 入口：同一上游模型、Chat SSE 成功。无 Vibe 会话，未发送 `X-Affinity`；请求体不同，因此长度不同。未将 Codex 或 Gateway 自用的请求标识透传给 Mistral。

Nextest `cff70c6b-9877-4ba6-ae57-a23dd3163671`：8 项定向测试通过。上述为本机隔离接收器结果，未经 Mistral 线上或真实账号验证。
