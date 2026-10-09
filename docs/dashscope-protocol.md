# DashScope 原生生成协议

Gateway 将 DashScope 文本生成和多模态生成分别记录为 `dashscope_text`、
`dashscope_multimodal`，二者属于同一协议族的不同生成表面，不代表支持整个百炼平台。

## 客户端入口

- `POST /api/v1/services/aigc/text-generation/generation`
- `POST /api/v1/services/aigc/multimodal-generation/generation`

使用 Gateway 的 Bearer API Key，继续经过现有鉴权、权益、额度和路由流水线。
请求为 `model`、`input.messages` 或 `input.prompt`、可选的 `parameters`。
流式请求设置 `X-DashScope-SSE: enable`，不要使用 OpenAI 的顶层 `stream`。
文本输出默认 `output.text`；`parameters.result_format=message` 返回 `output.choices`。
多模态采用内容块；流式默认累计，`parameters.incremental_output=true` 为增量。

```json
{
  "model": "qwen-plus",
  "input": {"messages": [{"role": "user", "content": "你好"}]},
  "parameters": {"max_tokens": 128, "result_format": "message"}
}
```

## 上游与探测

账号添加／刷新协议时探测两种 DashScope 生成表面。常规候选包括 `/api/v1`、
显式提供的 base 和 `/dashscope/api/v1` 别名。模型目录补充同源
`/compatible-mode/v1/models`；不会在每次推理中探测。
目录可见不等于模型调用已验证，仍保留成功／失败样本证据。

手工配置可使用 `dashscope_compatible` 或 `dashscope_multimodal_compatible` adapter。
无显式 `chat_completions_path` 时，根地址自动补 `/api/v1`；已有 `/api/v1` 不重复添加。
显式路径保持 base-relative 语义。使用 Bearer 认证，流式由上游发送器设置专用 SSE 头。

同协议请求保留原始 JSON，只替换路由后的 model；原生结果保留专属字段。
跨协议支持标准文本、图片 URL、function tools 和通用生成参数。
OCR 任务参数、专属内容块或未知原生字段不能静默转给普通 Chat 上游，必须匹配
原生表面；无法表达的结构化响应也会拒绝转换。工具桥接要求 message 输出格式。
同协议优先不能替代模型能力验证；一个简单文本探测不证明该模型支持 OCR 等能力。

不含 DashScope 应用工作流、异步图像／视频生成、实时语音、文件上传和本地文件读取。
客户端本地文件应由 SDK 上传或编码；Gateway 不读取客户端提供的服务器文件路径。

## 验证

```powershell
cargo test --locked --lib dashscope -- --test-threads=1
cargo test --locked --test dashscope_bridge_contract -- --test-threads=1
cargo test --locked --test provider_discovery_inventory_contract -- --test-threads=1
```

测试使用本地隔离上游，不修改持久账号或正在运行的服务。官方云端账号验收必须另行记录，
不得把模拟上游测试写成百炼实际调用成功。

协议依据：[DashScope API](https://www.alibabacloud.com/help/en/model-studio/qwen-api-via-dashscope)、
[Qwen-OCR API](https://www.alibabacloud.com/help/en/model-studio/qwen-vl-ocr-api-reference)。
