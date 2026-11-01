# AI Studio Web：Playground HTTP 调用

`aistudio-web` 是独立的服务商模板，preset 为 `aistudio-web-reverse`，目标为
`https://aistudio.google.com/prompts/new_chat`。它不合并 Gemini Web、Canvas，
也不把官方 Gemini API Key、旧 Build app 或 CodeAssistant 当作 Playground 调用。

## 调用过程与运行依赖

生产路径为 Gateway canonical 请求 → Node HTTP worker → 网页 bootstrap、
模型目录、偏好与日志上下文 → WAA challenge 与经摘要校验的解释器 → 原生 proof
helper → HTTP `GenerateContent` → 终止帧校验 → Gateway 公共 API 响应。

图片、音频、视频和 PDF 输入通过受限的 inline part 发送。WAA binding 按内容顺序包含
文本和媒体 base64，而不是仅绑定可见提示词；专用 ASR 可只含音频。无需 Drive OAuth、
上传、持久云端附件或清理文件，不读取用户 Drive，也不回退浏览器上传。

调用时不启动 Chrome、Edge、Firefox、Playwright 或 CDP。SpiderMonkey 位于原生
helper 子进程内，不是直接嵌入 Gateway Rust 进程；网络通信仍为 HTTP。每次生成
使用 fresh proof，没有官方 API 或浏览器回退。缺少 helper 在发送认证网络请求前失败。
该路径不是“完全不执行 JavaScript”的 HTTP 客户端。

helper 优先使用可信宿主设置的绝对路径 `AISTUDIO_WAA_HELPER_PATH`；未设置时从 worker
相邻的 `aistudio-http/runtime/aistudio-proof.exe` 定位包内依赖，不依赖工作目录，也不从 PATH
搜索 proof helper。Node 使用宿主已安装的版本，可由 `AISTUDIO_HTTP_NODE_BIN` 指定。
helper 的可复现构建入口为
`tools/build-aistudio-proof.ps1`；引用依赖固定到 AIStudio2API 的
`9b694a6b7c6439db1767a577994dfcc87a3cf588`。独立构建 helper 不等于完成发行打包；
正式分发还须包含依赖、来源及 MIT/MPL 等适用许可说明，不能只复制 Gateway EXE。

TTS WAV、PCM 格式化无需 ffmpeg；MP3 编码优先使用可信宿主设置的绝对路径
`AISTUDIO_FFMPEG_BIN`，未设置时可从宿主 PATH 的绝对目录解析已安装的 ffmpeg；
忽略相对／当前目录 PATH 项，不分发编码器。编码器仅通过管道处理 PCM，禁止访问网络协议，不写媒体临时文件；
缺失依赖在发送认证网络请求前失败。调用者不能通过请求体指定可执行文件。

## 凭据与生命周期

浏览器仅在选择网页登录时用于用户正常登录、随后导入必要 scoped Cookie；也可通过
专属网页会话 JSON 粘贴／文件入口离线导入自己的最小材料。账号的
`runtime_state_object_key` 指向 schemaVersion 1 的 `aistudio-web` Cookie 对象。
对象由 Gateway 正常存储 owner 读取，不接受调用者在请求中指定可执行文件或任意目标。
不导出完整浏览器状态，不把 Cookie、认证头、proof 或网页正文放入日志／验收报告。

Cookie 材料上限 32 KiB、64 条；请求媒体合计上限 6 MiB、16 个媒体 part，输入管道
上限 9 MiB，可见内容文本上限 8 KiB。HTTP 目标固定、禁止重定向；解码后生成响应上限
32 MiB，输出媒体及文本合计上限 20 MiB。总调用 deadline 最长 180 秒，原生 proof
有独立 65 秒外层超时，MP3 编码最长 15 秒且输出最多 8 MiB。
proof 准入上限为同一 Gateway 进程内 2 个；通用进程树监管 owner 负责取消及回收。
这不是多实例全局并发限制，也不构成未经测量的内存峰值保证。

管理 UI 的“添加账号”及账号库“手动添加”现已接入 AI Studio 专用网页登录导入。
可填写账号名称；普通登录阶段无自动化／调试连接，用户进入对话界面后只关闭本次专用
窗口，程序随后短暂重开导入必要 Cookie。该导入会话单进程最多并行 1 个，外层超时
25 分钟，浏览器／Node 沿现有进程树 owner 回收。导入仅执行只读 bootstrap／ListModels，
不执行生成、Drive 授权或 WAA proof，不要求先配置 proof helper。

私有子进程管道中的 Cookie 由 Rust 正常对象存储 owner 写入；普通管理响应和路由配置
只保留对象引用，随后沿现有 revision/CAS 合并保存。仅将目录中存在、当前实现支持的
四项模型纳入账号：`gemini-3.5-flash-lite`、`gemini-3.5-transcribe`、
`gemini-3.8-flash-lite-tts`、`gemini-3.1-flash-lite-image`，不伪造 `verified_models`。
网页登录导入入口限 Windows 本地桌面浏览器，尚未接远程宿主；新管理入口的真实重新
登录整链尚未验收，已存账号不因本轮 UI 修改而重新导入。

“添加账号”的网页会话 JSON 分支不启动浏览器，也不执行 bootstrap／ListModels；只做
格式、作用域、大小及必需签名 Cookie 校验，然后交给同一存储和 revision/CAS 保存链。
默认模型是手动路由提示，非已发现目录或 verified models。详见
[三家 Google Web JSON 格式](google-web-json-import.md)。该分支的合成回归不代表新材料
已通过真实上游认证；调用仍复用已有 HTTP-only 执行链。

bootstrap 重定向到 Google 登录时返回固定 `aistudio_http_login_required`，不自动跟随登录页。
当前没有完成长期 Cookie 刷新及 `Set-Cookie` 持久化验收，可能需要用户正常重新认证；
不能把一次成功解释为永久免登录。生成没有幂等键，失败不自动重试或改道重放。

`GenerateAccessToken` 的 `401 / unauthorized_client` 仅表示缺少可选的 Drive 授权，
不是整个登录失效。该 RPC 已从生产 inline 路线移除，诊断分类仍有回归保护。

## 支持的公共入口

| 入口 | 范围 |
| --- | --- |
| `/v1/chat/completions` | 文本及 inline 图片、音频、视频、PDF 理解 |
| `/v1/images/generations` | 单图生成，原生图片模型，`b64_json` 或 data URI 响应 |
| `/v1/images/edits` | multipart 图片编辑；不支持 mask、批量、quality/style 等控制 |
| `/v1/audio/transcriptions` | multipart 音频，专用 ASR 或普通多模态模型；JSON 或纯文本 |
| `/v1/audio/speech` | 原生 TTS voice；WAV（默认）、PCM、MP3 |

图片可用 OpenAI `image_url` data URI；音频可用 `input_audio`（例如 wav/mp3）；PDF 可用
`file`/`input_file` 的 `file_data` data URI。视频可用 Gemini `inlineData` 或 `fileData`
data URI part。兼容 `inline_data` 等 snake_case 别名。不解析远程 URL、本地路径或外部 file ID。

`gemini-3.5-transcribe` 当前只支持自动语言、无 prompt hint 的基本转写，不伪造时间戳、
说话人或 verbose JSON；普通多模态模型可以接受受限语言／上下文提示。TTS 不静默忽略变速
或额外控制；截断语音不作为完成音频交付。图片 `size` 按现有宽高比映射，不保证精确像素尺寸。

## 已验收结果

2026-11-01，Gateway 公共 `/v1/chat/completions` 使用 `gemini-3.5-flash-lite`，
非流式文本与 buffered SSE 各一次真实调用成功：HTTP 200、随机文本精确匹配、
`finish_reason=stop`，SSE 含 `[DONE]`。调用进程树采样未发现浏览器，独立实例正常 drain。
这不是官方 API 回退或 fixture 成功；详情见
[r36 真实验收记录](progress/2026-11-01-aistudio-http-public-api-r36.md)。

同日新增多模态公共 API 验收：图片、PDF、音频、视频理解，图片生成／multipart 编辑，
专用 ASR JSON／text，以及 TTS WAV／PCM／MP3，均已有真实成功证据；生成的 WAV
再经专用 ASR 转写与原句匹配。三个隔离实例均正常 drain，进程树采样未观察到浏览器。
首批 12 项中有两项返回通用错误，后续仅针对这两项分别新建单项验收并成功；保留失败，
不声称首批全绿或稳定性问题已解决。图像编辑的颜色修改成功，但文字不保证保真。
详细格式、耗时、失败边界与证据见
[r37 多模态公共 API 记录](progress/2026-11-01-aistudio-http-multimodal-r37.md)。

## 能力边界

- 生产 HTTP 实现保留多轮顺序、system instruction 和基本采样参数，兼容 canonical 的
  `system_instruction` 字段。图片、TTS 可返回纯媒体，不强行要求附带文本。
- 完成帧必需；重复文本 delta 必须追加，不能误当累计前缀丢弃。PCM chunks 按序合并，
  thought 文本及媒体不返回给调用者。专用 ASR transcript metadata 不重复追加。
- SSE 当前在完整响应后转换并返回，属于 buffered SSE，不是真实逐 token 流式透传。
- 当前网页音乐和视频生成入口标注 Paid；视频入口要求关联付费 API key。本轮未关联密钥、
  未启用付费请求，也未交付这些生成接口。这是账号前置条件及实现边界，不是平台不支持。
- Live 实时双向会话尚未接入；当前 Playground 模型目录未发现 embeddings 模型，
  不借用 Build 或官方 API 来冒充 Playground 支持。未实现的原生 schema／函数等控制仍明确拒绝。
- 显式 fixture 合同保留；fixture 通过不作为真实上游证据。
- 正常管理 UI 的 AI Studio 专用导入已接线并有合成回归；真实交互尚未完成整体验收。手测包可通过构建宿主的
  `GATEWAY_AISTUDIO_RUNTIME_DIR` 显式选择已校验的 proof runtime；打包验证当前 Go 源码摘要、
  固定上游版本、许可清单与逐文件哈希，所有文件纳入总包 manifest／checksums。
  不因此宣称 Node／编码器已自动打包，也不等同正式发布依赖审计完成。
