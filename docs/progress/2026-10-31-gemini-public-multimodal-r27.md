# Gemini Web / Canvas 公共 API 真实多模态验收 r25–r27

> 后续收尾：用户明确要求最后再试一轮 Canvas 视频，若失败也将本轮功能开发算作完成。
> r29 已实际单次点击，返回 HTTP 400，无视频输出；现按 `completed_with_known_limits` 收尾。
> 见 [r29 最终收尾记录](2026-10-31-gemini-development-closeout-r29.md)。下文保留 r27 当时的事实，
> 其中失败／未通过项目没有改写为 passed。

## 结论

本轮新增 14 项真实 Google 公共 API 成功验收，包含此前优先要求的 Web 音乐／视频、
文档理解、专用 MP3 转写，并补齐 Web 和真实 Canvas 宿主的原生音频／视频输入。
不能再把这些项目写成“只有实现或本地合同”。整个多模态总目标仍不标完成：Web 原文朗读
本次失败，Canvas 音乐／视频在当前宿主边界下仍无法交付。

证据根：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-public-acceptance-20261031-r25/`。
汇总为 `final-verification-r27.json`；每次请求都有独立 guard、业务回执和完成后的审计 ID。

## 真实 Google 结果

| 能力与公共入口 | Gemini Web | 真正的 Canvas host |
| --- | --- | --- |
| PDF → `/v1/chat/completions` | passed | passed |
| TXT → `/v1/chat/completions` | passed | passed |
| PDF → `/v1/responses` | passed | passed |
| MP3 → `/v1/audio/transcriptions` | passed | passed |
| 原生音频 inlineData → `generateContent` | passed | passed |
| 原生视频 inlineData → `generateContent` | passed | passed |
| `/v1/music/generations` | passed，取得原生 MP3 | 当前无可用宿主音乐合同，未完成 |
| `/v1/videos/generations` | passed，取得 MP4 | 当前无成功输出闭环，未完成 |
| Web `/v1/audio/speech` 原文朗读 | failed，HTTP 500 | 本轮没有重验 Canvas TTS |

上表为 14 项成功请求，不包含失败重测、只读核查或本地测试。成功审计均为
`routeAttemptCount=1`、指定 provider／credential、`status=completed`，没有换账号或官方
API Key 回退。Canvas 六项成功使用 `gemini-canvas-browser-relay`、`special_adapter=gemini_canvas`，
不是普通 Web composer。Web 音乐／视频使用 `gemini-web-chat`、`special_adapter=gemini_web`；
兼容模型别名虽含 `gemini-canvas-*`，不将这些 Web 结果计入 Canvas。

### 文档与转写不是占位答案

- 有效单页 PDF 为 719 bytes，文本文件为 100 bytes。验收随机码
  `GW25-10990159346E` 和箱数总和 `43` 只在文件内；请求没有透露答案。Web 与 Canvas 的
  Chat／Responses 均正确读出。
- MP3 输入为 27,116 bytes、24 kHz mono、2.170958 秒；输入已完整解码。两条专用转写链和
  两条原生音频理解链均正确返回 `Have a wonderful day.`，没有伪造 segments 或时间戳。
- 视频理解直接上传本轮生成的原始 MP4。问题没有透露目标物；Web 和 Canvas 均识别出
  白色纸帆船、平静水面、涟漪和晨昏光线，原生返回 `finishReason=STOP`。

### Web 音乐／视频已有可验证媒体

| 项目 | 音乐 | 视频 |
| --- | --- | --- |
| 回执 | `accept-media-music-r27.json` | `accept-media-video-r27.json` |
| HTTP / completed | 200 / true | 200 / true |
| 文件 | `result-music-r27.mp3` | `result-video-r27.mp4` |
| 字节 | 1,550,903 | 4,101,619 |
| 容器／编码 | MP3，44.1 kHz，双声道 | MP4，H.264 1280×720，AAC 48 kHz 双声道 |
| 时长 | 64.365667 秒 | 10.026667 秒 |
| 完整解码 | passed | passed |

音乐 SHA256：`1102e6764f5a87b46edfbc0de9d077b31e05a138c5edeae220f0b627b4a089dd`。
视频 SHA256：`10ab2bf6591576b87f0bc3ef731474ad181211b80261fe327fa7d79774f03fb4`。

音乐提示要求“约 30 秒”，上游实际交付约 64 秒。本轮只关闭基本生成／取回／可解码闭环，
不声称精确时长控制通过，也没有把 ffprobe 当作听感、无歌词或音乐风格的完整人工验收。
视频中点帧已人工视觉核对为水面上的纸帆船，预览为 `result-video-r27-preview.jpg`。

## 修复的真实机制

1. r25 音乐停在模式选择：小 viewport 将音乐入口移入“更多工具”二级菜单。
   新 `gemini-canvas-browser-pool-operation-menu.mjs` 只打开精确命名的二级菜单，不发送提示。
2. r25 视频停在发送前：视频模板路由先就绪，Gemini Omni 信息弹窗随后异步挂载。
   onboarding owner 现在有界等待精确介绍按钮，并继续拒绝订阅、付款和试用弹窗。
3. r26 音乐已选中模式，但旧手工 DOM 赋值／键盘路径没有形成可用发送状态。
   在同账号检查副本里，正常 locator fill 明确产生“发送”按钮。新增 media-submit owner
   对 single-submit 媒体使用原生 fill，并有界等待已启用的实名发送控件；不使用未知图标或
   Enter 回退，任何点击异常直接传播，绝不再点第二次。

原失败回执和 guard 均保留。r25／r26 失败均有明确 pre-submit 证据，修复后使用新的 r27
guard；r27 每个媒体操作只提交一次。没有在 accepted、结果丢失或不确定点击之后重新生成。
检查副本中的草稿已清空并关闭；原 `Media Capability Check` Canvas 应用保持打开，未刷新、
未改写、未点击其生成／导出／分享按钮。

## 源码与定向检查

新增生产 owner 为 operation-menu、media-submit；修改 operation-ui、media-mode-onboarding、
composer。新增 media-navigation 回归并更新 media-mode-labels／composer-single-submit fixtures。
新导航回归先得到 4 项预期失败；新 composer 回归先得到 2 项预期失败，再修复到绿。

- 媒体相邻套件 185/185；composer 套件 15/15，其中 4 项与媒体套件重叠，不相加冒充独立总数。
- effective-line checker 31/31；ratchet passed；Node syntax、严格 UTF-8 无 BOM、diff check passed。
- 没有为本轮安装或升级依赖，也未修改 baseline／exceptions。仓库没有配置独立 JS formatter，
  未把 syntax check 冒称为 formatter 通过；构建仍有既有 Rust unused／dead-code／linker 告警。
- 生产有效行（r25 包 → 当前）：operation-ui 337→339、onboarding 17→21、composer 109→103；
  新 operation-menu 9、新 media-submit 17。三个相关测试文件为 57、117、43 行，均未超限。

## 新手测包及运行边界

包目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-public-r27/`。

- `buildKind=devtest`、`edition=web`，不是桌面壳或正式 Release。
- 使用官方 `package-gateway-devtest.ps1`，locked dependencies；构建前无 Cargo 任务且可用内存约
  9.2 GiB，Jobs=4。Cargo 6m36s、整条构建打包约 8m43s，不能据此推断冷构建提速倍数。
- locked worker import、打包、staging／发布目录完整性 smoke 均通过；未覆盖 r25 或旧版本。
- EXE SHA256：`f3fab64a49d685b2800acde70ef6a3c8c6b618e9e5982aa761d2761ef0d6766f`。
- 构建冻结指纹：`824172592c088c1388d6a8e79d2fffdb49e1c87256fcf6e9a5e140603d626b07`。
  本文为出包后追加的验收文档，不声称当前整个工作树仍等于该冻结快照。

真实 Google 回执产生在 r25 后端＋r27 source-worker 隔离阶段；五份包内 worker 文件已逐字节
核对与验收脚本哈希一致。随后只将本任务 4365 隔离实例切换到完整 r27 包，复用其自身持久化
数据，不复制在线账号、不再让用户登录，不重复生成已有作品。

新实例 PID 48272，`http://127.0.0.1:4365`，health 200，三个验收 provider 和既有完成审计均保留，
自动探测／刷新／库存等仍关闭；公共默认文本模型路由已恢复 Web。TLS 运行文件显式放在
私有隔离数据目录，避免新包目录生成运行状态。切包后仅做启动和只读持久化核查，未将其冒充
第二轮真实上游验收。详见 `runtime-packaged-r27.json`。

本任务未停止、替换或升级在线 4200。最终观察到本机 4200 仍由旧 r20 包运行（PID 31540，
桌面壳 PID 37104）；它们与开始时的历史 PID 不同，本任务不推断其他会话或用户做过何种操作。
本轮未提交、推送、正式发布、运行全量回归或修改在线凭据／数据库。隔离导入走程序正常流程，
没有输出 Cookie、token、Google API Key 或浏览器会话文件。

## 尚未闭环

### Web 原文朗读：失败，未验收

单次 `/v1/audio/speech`（`voice=default`、`response_format=opus`）返回 HTTP 500，
`invalid_gemini_web_speech_response`，没有取得可解码输出。回执为 `accept-speech-r26.json`。
本次证据不能判断具体是引导／RPC 形状还是容器准入失败，更不能据此称平台不支持朗读。
没有自动重放、换账号、隐式转码或以 Canvas TTS 冒充 Web。MP3 输入转写的成功不等于 MP3 TTS 输出支持。

### Canvas 音乐／视频：当前宿主边界下无法完成交付

本轮原页只读核查再次取得公开模型声明：text `gemini-3-flash-preview`、image
`imagen-4.0-generate-001`、edit `gemini-3.1-flash-image`、transform `gemini-3-pro-image`、video
`veo-2.0-generate-001`、tts `gemini-2.5-flash-preview-tts`。宿主代理脚本没有音乐／Lyria 合同，
视频仍只见 `predict`／`predictLongRunning`，没有已确认允许的 operation GET／文件下载闭环。
详见 `canvas-host-readback-r27.json`。

没有新证据支持重放此前的同步内部 `[13]`／401 请求，未修改 Google 注入配置、猜测新白名单，
也没有回退 Web 或官方 API Key。继续保留 `gemini_canvas_host_media_contract_unavailable`。
结论是当前环境下两项无法交付，不是断言 Google 永久不会支持；整个 Gemini 多模态任务仍未全完。
