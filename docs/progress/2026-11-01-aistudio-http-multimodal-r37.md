# AI Studio Web HTTP 多模态实现与真实公共 API 验收（r37）

## 结论与范围

本轮为 `aistudio-web`／`aistudio-web-reverse` 接通并真实验证当前账号可用的 inline
图片、音频、视频和 PDF 理解、单图生成／编辑、专用音频转写，以及 WAV／PCM／MP3 TTS。
这是 Playground 会话 HTTP 路线，不是官方 API Key、旧 Build app 或浏览器生成回退。
文本与 buffered SSE 的既有证据见 [r36](2026-11-01-aistudio-http-public-api-r36.md)。

**功能成功不等于所有请求稳定成功。** 首批 12 个公共请求中 10 项通过，两项返回固定
通用错误；随后只为失败的图片理解、MP3 各建立一个新的单项验收，均成功。合计 14 个
公共请求、12 次成功，覆盖全部 12 个验收场景。没有自动重试、改道或重放已消耗的 guard。
两次失败的根因未从脱敏结果中确定，不能以新请求成功宣称已修复稳定性。

## 公共 API 实测

时间为 2026-11-01 03:25—03:30（Asia/Shanghai）。使用正常导入的既有 Cookie 对象，
由三个随机 loopback 端口的独立 Gateway 实例验收。每个实例通过正常管理 API 创建临时
access key，经过公共鉴权、canonical pipeline、路由和生产 worker。数据库、路由、状态目录
独立；对象根显式指向正常凭据 owner，仅读取精确对象，不复制 Cookie，因此不是对象根完全隔离。

下表全部成功项均为 HTTP 200、路由证明 `v1`、provider line 为 `aistudio-web-reverse`、
请求 ID 匹配。聊天内容完成信号为 `finish_reason=stop`。

| 场景 | 公共端点 | run／耗时 | 成功输出检查 |
| --- | --- | --- | --- |
| 图片理解 | `/v1/chat/completions` | r2／6,831 ms | 精确读出 `R37_91D7C0` |
| PDF 理解 | `/v1/chat/completions` | r1／6,894 ms | 精确读出 `DOC_8931A9F6` |
| 音频理解 | `/v1/chat/completions` | r1／8,504 ms | 转写与已知句子匹配 |
| 视频理解 | `/v1/chat/completions` | r1／6,950 ms | 起止背景颜色 `red,blue` |
| 图片生成 | `/v1/images/generations` | r1／9,724 ms | JPEG 447,545 bytes，目视红方块和蓝圆 |
| 图片编辑 | `/v1/images/edits`，multipart | r1／9,770 ms | JPEG 204,854 bytes，红方块变绿、蓝圆保留 |
| 专用 ASR JSON | `/v1/audio/transcriptions`，multipart | r1／7,893 ms | `application/json`，`text` 匹配 |
| 专用 ASR text | 同上 | r1／8,408 ms | `text/plain; charset=utf-8`，内容匹配 |
| TTS WAV | `/v1/audio/speech` | r1／7,704 ms | `audio/wav`，201,644 bytes |
| TTS PCM | 同上 | r1／6,873 ms | `audio/pcm;rate=24000;channels=1`，205,440 bytes |
| TTS MP3 | 同上 | r3／7,256 ms | `audio/mpeg`，79,532 bytes |
| TTS→专用 ASR | `/v1/audio/transcriptions` | r1／8,846 ms | 将刚生成的 WAV 转写，仍与原句匹配 |

实际模型分别为 `gemini-3.5-flash-lite`、`gemini-3.1-flash-lite-image`、
`gemini-3.5-transcribe`、`gemini-3.8-flash-lite-tts`。音频校验原句：

```text
The blue river flows past seven green trees.
```

### 媒体内容与资源检查

- 图片生成及公共编辑输出均已亲自目视检查。编辑满足颜色修改，但原图文字存在字符丢失／
  排版变化；这不是无损编辑、文字精确保真或所有提示约束完全通过。
- ffprobe 与 ffmpeg 实际解码通过：WAV 4.200 秒，PCM 4.280 秒，公共 MP3 4.968 秒；
  均为 24 kHz、mono。PCM 按 s16le 解释；WAV 再转写闭环进一步验证可用语音内容与端序。
- 单独用生产 `formatAudio` 对既有 PCM 离线编码为 MP3 并成功解码，零网络请求；
  不把该本地成功当成公共 MP3 接口成功，公共接口另有 r3 证据。
- r1／r2／r3 分别采样 63／6／6 次，未观察到浏览器。r1 观察到 Node 与原生 proof helper。
  采样到的进程树工作集最高为 221,011,968 bytes（约 211 MiB），不是严格峰值、并发容量
  或低内存保证；短进程可能未被采样到。生产入口不导入浏览器模块，也不设浏览器回退。
- 三个隔离实例均正常 drain，exit 0；没有重启或替换用户运行的 r33 桌面包。

### 保留的失败

- 公共 r1 图片理解：698 ms，HTTP 503，`aistudio_http_request_failed`。
- 公共 r1 MP3：5,603 ms，同一通用错误。没有敏感阶段日志，不能确定是否到达生成端。
- 生产没有自动重试；后续 r2／r3 为显式单项复验，没有重跑已成功的其他场景或复用旧 guard。
  不能称首批 12/12 全绿。
- worker 较早的图片 403、TTS MIME 拒绝结果也保留，见同一证据目录；后续实现修正与新验收
  使用不同 guard，未删除或复用历史记录。

## 关键改动及真实故障定位

1. inline 图片／音频／视频／PDF part 有 MIME、base64、数量和合计大小准入。兼容 OpenAI
   `input_audio`、`file`／`input_file` 及 Gemini snake_case aliases；data URI MIME 为权威。
2. WAA binding 原来只含文本，带媒体时真实生成 403；改为按内容顺序绑定文本与媒体 base64。
   同一实际登录材料随后图片、PDF、音频、视频均成功，不需要 Drive 上传或额外 OAuth 授权。
3. 可选 `GenerateAccessToken` 的 `401 / unauthorized_client` 是缺少 Drive 授权，不是全部
   Cookie 失效。生产 inline 路线已移除此 RPC，也没有申请 Drive 权限或创建云端临时文件。
4. 生成配置映射图片、AUDIO slots，解析纯媒体响应、顺序 PCM chunks 和专用 ASR transcript；
   thought 内容不外泄，重复 transcript 不追加两次，完成帧缺失或截断语音拒绝交付。
5. 修复含空格的音频 MIME 解析；AI Studio 当前 PCM 实际为 little-endian，WAV 不交换字节。
   WAV／PCM 不需编码器，MP3 用可信宿主指定的 ffmpeg 管道编码，无 shell、网络协议或临时媒体文件。
6. Rust 路由接通 multipart ImagesEdits／AudioTranscriptions，保留 JSON／纯文本格式，
   images／speech 去掉旧官方 API 与浏览器回退分支，显式 fixture 合同继续保留。
7. 原生 proof helper 扩大有界媒体 binding 输入；PowerShell 5.1 builder 不再依赖
   `Path.IsPathFullyQualified`，仍拒绝 `C:relative`。未升级或重装锁定依赖。

输入媒体合计上限 6 MiB、最多 16 part，stdin 上限 9 MiB、可见文本上限 8 KiB；
HTTP 生成响应上限 32 MiB，解码媒体与文本合计上限 20 MiB，总调用最长 180 秒。
同一 Gateway 进程最多同时执行两个 AI Studio proof 请求。详细契约见
[AI Studio Web HTTP](../aistudio-web-http.md)。

## 定向验证与来源

- Rust AI Studio 定向测试 **43/43**；新增行为均有相应失败回归后修正，不使用 fixture 代替实测。
- Node AI Studio／manual-login 定向测试 **33/33**，最终媒体验收后再次通过。
- manifest validator **44 passed**，checker tests **31/31**、effective-line ratchet passed，
  governed source >700 为 0；Rust formatter 与 `git diff --check` passed。
- 后端 `devtest`、锁定离线依赖、2 jobs 构建通过，耗时 **13m56s**。此前 opt-level=0 的定向
  测试构建为 **5m36s**；不是同条件冷／热构建对照，不据此承诺提速倍数。
- 重点文件有效行数：`http.rs` 156、`images.rs` 84、`speech.rs` 86、`transcription.rs` 168、
  `aistudio_web.rs` 231、`passthrough.rs` 408、`request_compatibility.rs` 256。
  新 Node 模块均小于 100；Go helper 物理 104 行，当前 checker 不把 `.go` 视作支持语言，
  物理行数仅作上界，不冒充有效行数。未增长 >700 的既有文件或修改治理基线。

验收后端为仓库 `target/devtest/gateway.exe`，SHA256：

```text
1dc2c75fa81d3676e20089049a83984fc77ef0c2905ac3859e24ae6c72853788
```

新原生 helper SHA256：

```text
b8602f1cb1b5b37b3a7a4bbefca6466cfab8662b48ea1223412b773fb3b2f75c
```

安全证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-aistudio-http-20261101-r37`。
`public-media-r1.safe.json` 保留首批失败；`public-media-r2.safe.json`／`public-media-r3.safe.json`
为单项成功，`media-validation.safe.json` 为离线解码，`source-inventory.safe.json` 记录代码摘要。
没有把 Cookie、token、认证头、proof 或完整网页响应写入报告，也没有导出浏览器状态。

## 未交付及未验证边界

- 音乐 Lyria、视频当前网页入口标注 Paid，视频要求关联付费 API key；已只读核实，未关联密钥、
  未发送付费请求。这些生成能力本轮未实现／验收，不是断言平台不支持或 Web 不可实现。
- Live 实时双向会话尚未接入，不能把 HTTP `GenerateContent` 的成功当作 Live 支持。
- 当前 Playground 模型目录没有 embeddings 模型，不借 Build 或官方 API 冒充。
- 原生 JSON schema／函数控制、ASR timestamps／speaker、批量图像／mask、TTS speed 等
  未实现选项明确拒绝；普通多模态转写和专用 ASR 的选项边界不同。
- SSE 仍是 buffered SSE；长期 Cookie 刷新、`Set-Cookie` 持久化尚未完成验收。
- 正常管理 UI 的 AI Studio 专用导入交互、helper／编码器自动依赖打包不在本轮成功交付内；
  仍需要可信宿主配置依赖绝对路径。没有新桌面整包、正式 Release、提交或推送。
