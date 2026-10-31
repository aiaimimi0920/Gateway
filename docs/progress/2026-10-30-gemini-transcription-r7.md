# Gemini Canvas 转写与会话权威性修复（r7）

## 改了哪些内容

新增 Canvas browser adapter 的 `/v1/audio/transcriptions` 接入，复用公共 multipart、鉴权、路由、
审计与既有 Google 注入 Canvas host；不新增 API Key 身份、浏览器执行器或依赖。
普通 Gemini Web、官方 Gemini API 和 legacy program adapter 没有实现这条专用转写链，继续拒绝。

- multipart 文件的原始 bytes 以 Gemini `inlineData` 发送，不再只使用规范化后的文件名/prompt。
  不上传到普通 Web，不重编码；最多 16 MiB base64 encoded。MIME/大小/base64 准入不等于完整解码。
- 支持 MP3、WAV、FLAC、Ogg、WebM、M4A/MP4 音频 MIME；仅在 MIME 为 octet-stream 或空时，
  从受支持的扩展名推断。文件名不作为磁盘路径读取，也不进入转写提示。
- `response_format=json` 返回 `{"text":"..."}`，`text` 返回 UTF-8 `text/plain`。
  当前是模型提示式转写，不是强制对齐 ASR；不编造 duration、segments、word timestamps。
  `srt`、`vtt`、`verbose_json`、timestamp/include/chunking 参数、流式及非零 temperature 在生成前拒绝。
- `language` 仅接受两位小写语言提示；prompt 为有界参考上下文，不作为 system instruction。
  输出仅接受首个 STOP candidate 的非 thought、非空文本；超过 1 MiB 直接失败，不返回截断前缀。
- 公共上传入口保留重复文件字段供规范化拒绝，不再静默使用最后一个上传文件。
- 请求仍绑定所选 runtime-state/账号；host 故障和不完整文本禁止自动重放或换账号。

## 会话刷新边界

只读审查确认旧 dispatcher 会对 storage-state 和 clone 模式每次重新注入导入时的 Cookie；
profile 模式也会重复注入嵌入 JSON。现将这些内容限制为缺少 auth cookies 时的初始化种子：
已有浏览器 Cookie 始终优先，显式 force 开关也不覆盖它；完成首次 seed 后，不再把旧 header
传给嵌套 app/fetch 恢复分支，防止导航刷新之后再次回退。

这不保证服务端永不过期，也不等于 JSON session 自动持久化。当前 JSON 模式仍仅保留进程内刷新值；
跨重启 checkpoint、跨 worker CAS、远端对象回写和失效/重新导入后的缓存更新仍需独立闭环。
不能仅用无锁原子 rename 覆盖原始导入文件，或按固定超时强拆锁，声称并发安全持久化已经完成。

## 验证方法与证据

所有测试使用合成身份、无账号 fixture 和本地 loopback，不执行此前被策略拒绝的认证动作。
MP3 fixture 为本地 FFmpeg 生成的 440 Hz 音调，1196 bytes、16 kHz、mono；ffprobe 识别 MP3，
FFmpeg 完整解码通过。模拟文本“合成回执”只证明完整输入传递和响应接线，不是模型识别证据。

- 公共 selector 的 AudioTranscriptions 准入测试先红：Canvas 候选被拒绝；最小接线后恢复。
- 完整 multipart 测试先红：重复文件仍返回 200；入口改为保留重复字段后，已验证在上游前返回 400。
- 公共测试覆盖真实 local runtime 鉴权、JSON/text、无授权、未支持格式、重复文件与 worker 单次失败；
  验证上传 MP3 bytes 和 selected runtime-state 精确保持。worker 只是本地模拟接收端。
- 合成 cookie jar 连续调用测试验证：第一次 seed 后旋转的值不会被后续旧 header 覆盖；
  嵌入 state 不重复注入，嵌套 app/fetch 不获得旧 header。
- 测试脚手架初期参数数量、模拟回执缺少 operation、Windows 测试线程栈不足已分别定位；
  这些不算产品红测。完整 pipeline 测试沿已有测试先例使用 16 MiB 线程栈。

最终通过项、源码指纹、包哈希和运行 smoke 以
`linshi/gateway-gemini-dual-mode-20261030-r1/r7-delivery.json` 为准；不存在或未通过时不能称为交付成功。
普通桌面手测包目标目录是 `gateway-devtest-20261030-gemini-dual-mode-r7`，不是正式 Release。

构建前已通过：受影响 Node 90/90、Rust host/input/transcription 22/22、公共 ingress 5/5、
normalizer 与 provider policy 27/27、嵌套包合同 1/1；checker 单测 31/31 与 ratchet 通过。
测试目录通过进程级 TEMP/TMP 显式限定到本批 linshi 子目录；不在可移植测试代码硬编码本机绝对路径。
独立只读源码复核没有发现确认的转写接线/不重放缺陷；不替代真实上游验收。

## 尚未完成的总目标

真实 Canvas 转写/MP3 识别、普通 Web 全部媒体公共 API 验收、Canvas 音乐与视频、长期会话持久化
仍未完成。真实上游验证仍受 r6 记录的工具策略拒绝限制，不换脚本、入口、凭据或执行面绕过。
本批不将普通 Web 回退、官方 Key、pending、空媒体或合成回执当作 Canvas 多模态成功。

不修改/重启既有 Gateway，不关闭原登录窗口，不改在线凭据或数据库，不提交/推送。
