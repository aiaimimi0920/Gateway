# Gemini Web / Canvas 文档输入与宿主只读复核 r21

## 范围与结论

本轮继续 Gemini Web + Canvas 多模态任务，补齐 PDF／文本文件输入的准入和执行接线。
Web 与 Canvas 仍为独立执行入口，不修改官方 Gemini API encoder，也不回退普通 Web
冒充 Canvas。此记录不是“全部多模态完成”或新手测包交付凭据。

## 改了哪些内容

- 新增 `src/protocol/gemini/document_input.rs`，仅共享文档形状、MIME 和最低字节准入。
  支持 PDF、UTF-8 plain text、Markdown、CSV、HTML、XML、JSON；不解压、渲染或执行文件。
- 接受 Chat Completions 的 `type=file` + `file.file_data`，以及 Responses 的
  `type=input_file` + `file_data`，内容必须为内联 base64 data URI。原有 Gemini
  `inlineData` / `inline_data` 也可携带上述文档。
- 不解析 `file_id`、不抓取 `file_url`、不读取调用方 filename 指向的路径。拒绝混合 alias、
  不支持的额外字段、空数据、非法 base64、非 UTF-8 文本及缺少最低 PDF 包络的数据。
- Web 使用固定内部上传名 `input-N.pdf/txt/...`，先完成所有输入准入，再执行
  bootstrap、顺序上传和生成。上传域不携带 Cookie / Authorization，prompt 不包含文件 base64。
- Canvas 将文件转换为宿主 `inlineData`，保留所选会话、模型和 user role，不改为普通 Web 上传。
  两条执行路径继续限制 16 个输入和合计 16 MiB base64；不放宽重试或失败回退。

PDF 检查仅要求 `%PDF-` 起始和尾部 1024 bytes 内的 `%%EOF`；这是最低包络准入，
不是完整 PDF 解码、有效页面认证或真实模型理解证据。测试中的最小合成 PDF 同样不称为
可渲染文档。文本只做非空 UTF-8 / NUL 检查，不承诺 JSON、XML 等格式语义校验。

## 验证

先前 `document_inputs_` 红测日志记录 4 项中 3 项失败：Web 拒绝文件、Canvas 原样转发
OpenAI 文件形状、Canvas 未拒绝远程 ID / URL。实现后新增共享准入和 loopback 承重测试，
当前 8 项全部通过；其中 2 项使用真实本机 HTTP 验证上传与生成顺序、文件名和身份隔离，
以及无效／超限文档在 bootstrap 前拒绝。

相关 Rust 回归 65/65 通过，包含既有图像／音频／视频输入、Canvas host、转写和错误重放保护。
这组筛选与 8 项文档测试有交集，不相加作为独立测试总数。

公共 HTTP 到 Canvas worker 合同 7/7 通过，其中新增文档合同在 `/v1/chat/completions` 与
`/v1/responses` 各执行 7 种情况：两种合法文件形状各触达 worker 一次；远程 URL、file ID、
损坏 PDF、非 UTF-8 文本和未授权请求全部拒绝。4 次合法请求均同时携带 PDF 和 plain text，
验证 inlineData、所选会话、模型及 user role。worker 返回合成文本，只证明接线，
不是 Google 模型成功读取文档。同一可执行文件中的既有转写整链及路由合同也通过。

检查器单测 31/31、effective-line ratchet、formatter check 和 `git diff --check` 已通过。
新增／修改源码经严格 UTF-8 解码及 BOM 检查，无 BOM；最终文件计量见证据目录中的
`document-lines.json` / `verification.json`。

定向 Rust 测试使用 locked/offline 的 devtest profile，并仅对 gateway 测试 crate 设置
`opt-level=0`。这不是官方 opt-level=1 的交付构建，不将测试二进制当作用户手测 EXE。
首轮文档测试构建用时 4m37s，带 25 项既有 warnings；没有使用 `cargo fix` 改动无关源码。
公共 HTTP 合同的 library / integration / bin 构建用时 5m14s；library 带 28 项既有 warnings，
另有 Windows linker messages。未把这些定向构建当作全目标静态检查或无 warning 构建。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-multimodal-20261031-r21/`。
主要日志为 `rust-document-red.log`、`rust-document-green.log`、`rust-related-regression.log`、
`rust-canvas-document-http.log`。旧红测日志是 UTF-16LE；本轮新增日志为 UTF-8 无 BOM。

## 当前已登录 Edge / Canvas 复核

桌面托管浏览器连接可用，已读取保留的 `Media Capability Check` 会话，打开
`Canvas Music Contract Check` 代码后恢复预览。预览已正常显示 `Execute Contract Check`
按钮，本轮没有点击该按钮或其他生成／重新生成按钮，没有发送新提示、发布或分享。
页面保持打开，截图为证据目录下 `canvas-readonly.png`。

只读检查当前 preview DOM 中 Google 注入脚本的公开模型字段（有界 512 KiB，实际约 32 KiB）：

| 字段 | 当前读取值 |
| --- | --- |
| textModelName | gemini-3-flash-preview |
| imageModelName | imagen-4.0-generate-001 |
| imageEditModelName | gemini-3.1-flash-image |
| imageTransformModelName | gemini-3-pro-image |
| videoModelName | veo-2.0-generate-001 |
| ttsModelName | gemini-2.5-flash-preview-tts |

这份模型元数据没有 musicModelName 或 Lyria token。读取范围内也没有声明 proxiedDomains，
不能把“字段缺失”说成“已读到空数组”。应用自身使用假设的 `lyria-preview` URL，
其生成的文字和按钮不构成真实宿主音乐合同。本轮没有发请求验证该 URL。

视频模型名存在只证明宿主声明该字段，不能推导操作成功。此前真实调用的 parent RPC
内部 `[13]` / 空正文 401 失败未被本轮只读检查推翻；现有
`gemini_canvas_host_media_contract_unavailable` 保护保持不变，不将它解释为整站登录失效。
没有导出 Cookie / storageState、重复被拒绝的会话导出操作，或通过修改 Google 配置、
native fetch、官方 API Key、普通 Web 回退绕过宿主准入。

## 尚未完成的范围

- 文档输入的真实 Google 模型理解尚未验收；本轮只完成源码和本地合同。
- Canvas 音乐缺当前宿主允许的模型／传输合同，视频缺成功响应和 operation / poll / download 闭环。
- Web 多项媒体能力仍需真实公共 API 成功验收；历史普通 Web UI 的现成音乐／视频不能替代 Gateway 验收。
- 未请求打包，本轮未交付新 EXE，未停止或替换在线 r20 实例，未修改其凭据、数据库和配置。
- 保留其他未提交改动；未提交、未推送、未部署。
