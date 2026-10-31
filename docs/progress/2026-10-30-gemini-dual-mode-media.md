# Gemini Web / Canvas 双入口与多模态扩展（2026-10-30）

## 范围与当前边界

用户要求保留普通 Gemini Web 和真实 Gemini Canvas 两个独立入口，并扩展图片生成、图片编辑、
语音、音乐和视频。普通 Web 的文字 HTTP 验收已在此前记录完成，Canvas 文字普通响应和 SSE
已通过前一轮真实 Gateway 验收；本记录不把这些结果提升为全部多模态通过。

本轮登录现场是用户配合登录的独立 Edge profile；已成功复用，不要求再次登录，不关闭日常
4200、此前 Web 4300 或其他既有测试实例。凭据、storageState、Cookie、token 和媒体正文只留在
`linshi/gateway-gemini-dual-mode-20261030-r1/private/`，不提交、不打包。

## 改了哪些内容

- 新建 `gemini-canvas` 模板，实际 preset 为 `gemini-canvas-browser-relay`；普通 Web 继续使用
  `gemini-web`。卡片依据实际 preset 显示，名称只兼容无 preset 的旧记录，不把两者混成同一入口。
- 新增 `special_adapter=gemini_canvas`，与 `gemini_web` 分开校验；前端 managed session owner
  同步锁定站点会话指纹。旧无 call_flow 配置仍可读，不改存储版本或迁移现有数据库。
- Canvas 登录导入仅为当前选中 provider/family 生成一个 draft，不采集官方 API Key 代替会话。
  缺省手工登记文字、图片和 TTS 模型并添加路由，不冒称拉取了模型目录；保留已有显式选择。
- 新增 host GenerateContent owner，文字、图片生成/编辑、TTS 都在当前账号的 Google 注入
  Canvas preview 内执行；编辑保留原始 inline 图片，不走普通 Web 上传或隐式 JPEG 重编码。
- 请求前校验模型路径、图片数量/输入、mask 与输出格式；响应只接受 STOP 的非 thought、
  非空且类型匹配的 inline 媒体。PCM 参数有界，压缩音频不能冒充 PCM。
- 补 PCM 完整 sample/frame 校验和 WAV/Ogg Opus 最低容器结构准入；不把任意非空 bytes
  冒充成功音频。这不是完整媒体解码器。仅在 Canvas host 内将 Google 的
  `audio/L16;codec=pcm` 按官方 cookbook 的原始小端样本封装，不改变共享 API/Web 的标准 L16 行为。
- 宿主请求拒绝 query、header 和 body.apiKey 中的显式 API 身份，防止 Google wrapper
  因该参数切换到 native fetch。失败不重放到其他账号、官方 Key 或普通聊天。
- 发现旧 Canvas 音乐/视频会进入普通 Web 媒体 composer，现改为固定
  `gemini_canvas_host_media_contract_unavailable`，在浏览器/远端执行前拒绝；保留原有输入校验。
  这是修正错误的模式回退，不代表音乐/视频已经实现。
- 包内容合同补充新 host-fetch 模块；不新增运行依赖、不升级 lockfile。

## 真实上游诊断

所有生成都有独立 create-new guard；不原样重放已消费的 guard。

| 能力 | 结果 | 证据边界 |
| --- | --- | --- |
| Canvas 图片 | 宿主 HTTP 200、STOP，下载并解码为 1024×1024 RGB JPEG | 宿主能力诊断，不是本轮 Rust Gateway 整链 |
| 首次 TTS | parent RPC HTTP 200 外壳，内部 `[13]`，无音频 | 失败；不把外层 200 当成功 |
| 正常创建的 Canvas TTS 应用 | 一次点击生成有效 WAV；24 kHz / mono / 16-bit，1.850958 秒 | 已保存并用 Python wave 读取；还不是 Gateway 整链 |
| 音乐 | 当前共享应用和新建应用的 `proxiedDomains=[]` | 仅准入检查，未发音乐生成请求 |
| 视频 | 一次宿主 predictLongRunning 调用得到 TypeError，无媒体 | 失败，不算 accepted 或完成 |

图片 SHA256：`156245a202057fae369bac8cd6ef4756968e00c37932d9a7bad1410e9c7288df`，338588 bytes。
正常 Canvas 应用语音 SHA256：`d27c4dc34707b3fb8417eee2c29d64052000737c0fffc87dcd836328da7d6c79`，88890 bytes。

新建 Canvas 应用通过正常 UI 的 Canvas 工具完成，未发布/分享、未自动生成媒体。应用生成的
TTS 合同在 URL 和 body 中都包含模型名；Gateway host TTS 据此对齐 body.model。此前失败与
这次成功还存在应用上下文、voice 和文本差异，因此不能单独断言 body.model 是唯一根因。
新应用仍没有允许音乐 WebSocket 域，应用自己将音乐、视频标为 unavailable；这不等于所有
账号或未来上游版本永久不支持。没有改写 Google 的注入配置或绕过平台准入。

Google 官方 Canvas 帮助描述可加入文字、图片等 Gemini 功能，并未承诺每一种 Gemini API
能力都可从 Canvas 应用调用。官方 API 文档支持某模型，不构成 Canvas 宿主支持证据。
来源：[Canvas 帮助](https://support.google.com/gemini/answer/16047321?hl=en)、
[Google TTS cookbook](https://github.com/google-gemini/cookbook/blob/main/quickstarts/Get_started_TTS.ipynb)。

## 定向检查与剩余验收

- 前端 typecheck、双模板/卡片/导入等 7 文件 59 项测试通过。
- Rust call_flow 5 项、登录导入 19 项、媒体 presets 9 项通过；host generation 首轮 7 项、
  增加模式保护与完成状态检查后 8 项通过，最终音频完整性及字节序测试 9 项通过。
- Node 受影响 181 项通过，含两项真实离线 Edge document identity 测试。
- 新 host body.apiKey 拒绝测试先失败，再修复至 23/23；不输出或记录实际 Key。
- 新 worker 的嵌套包合同 1 项通过。
- PCM 不完整 frame 红测确认旧代码会返回成功 WAV，随后补数据准入与字节序样本测试。
- effective-line checker 31 项通过；本轮计量 host owner 396、relay 442→450、登录 owner
  500→500、JS host owner 117；最终音频修改后的 ratchet、formatter check、MJS syntax 和
  `git diff --check` 通过。
- 本轮源码的隔离 Gateway 媒体验收仍待补齐。

## 新源码隔离验收与重放保护

第一版 API devtest 构建成功，隔离后端 4312 与 worker 43141/43142 启动检查通过。
首个 buffered 请求未通过：`page.goto` 导航到 Gemini `/app` 时返回
`net::ERR_CONNECTION_CLOSED`，审计错误为 `gemini_canvas_browser_worker_failed`。
该请求的 `routeAttemptCount=3`，未记录到 host fetch 完成事件，不能称为模型调用成功。
随后现有登录浏览器的独立导航得到 HTTP 200，worker 的无 prompt、无 reset、无生成 readiness
检查也识别到已登录 Gemini；这说明导航后来恢复，不足以证明首次断开的根因已被永久修复。

补充两项先失败的回归：Canvas provider retry 仍为 2，以及音乐/视频不可用错误仍允许重试。
现将 browser-relay 的外层 provider retry 限为 0；host 请求失败和不可用媒体错误保留原始
错误归因，但设置不可重试和 Abort，禁止自动换账号重放。文字解析失败与无完整媒体也不重放。
普通 Gemini Web 与 legacy program relay 的既有策略不随此修复改变。

本次增量有效行：host owner 396→406、relay 450→453、retry feedback 255→256、
retry policy tests 213→234、host tests 237→272；未触碰 500 行的登录 owner。
checker 31 项和 ratchet 再次通过。后续用新的冻结二进制和独立 4313 后端验收，串行复用
43141 worker 与它的隔离对象目录，不修改 4200、4300 或已登录浏览器现场。

本轮未提交、未推送；没有生成桌面手测包或正式发布包。后续运行必须使用本轮源码编译的
隔离二进制，不能使用旧冻结 EXE 声称新 Rust 接线通过。

## 4313 最新整链实测与图片编辑接线修复

第二版 API devtest 已构建成功，构建前后源码指纹一致；冻结二进制 SHA256 为
`2c9c7ae620b765c02de9195e161894c3f6425b47d12c052f49556f538af3fb83`。
继续复用用户已登录的独立浏览器，将它仍有效的 r3 快照复制到隔离对象目录；没有要求用户重登，
没有覆盖旧快照，也不能据此声称长期会话自动刷新已完成。

| 4313 能力 | 实际结果 | 审计与业务边界 |
| --- | --- | --- |
| Canvas 普通文字 | HTTP 200，精确算式答案，stop | 选定账号一次，`routeAttemptCount=1` |
| Canvas 下游 SSE | HTTP 200，精确答案、stop、DONE | 2 个事件；先聚合后桥接，不是逐 token 透传 |
| Canvas 图片生成 | HTTP 200，307383 bytes，有效 1024×1024 JPEG | Pillow 解码通过，选定账号一次 |
| Canvas TTS | HTTP 200，104250 bytes，有效 WAV | 24 kHz、mono、16-bit、2.170958 秒，完整非静音帧，选定账号一次 |
| Canvas 图片编辑 | HTTP 400，未发上游，路由尝试为 0 | 执行器已实现，但公共路由兼容性遗漏 ImagesEdits |
| Canvas 音乐和视频 | HTTP 503，固定 unavailable code | 各一次路由、无错误回退；这是保护验收，不是媒体生成通过 |

图片 SHA256：`95a22a6083464e9bb5e0a256469007c7b68ebb7134436553292f8c5b2c8f407e`。
TTS SHA256：`010ff99fc8e4b89408c9c3da34fab47a277bdb65e671a449736faf36983ce3d1`。
安全回执位于 `linshi/gateway-gemini-dual-mode-20261030-r1/*-v2.json`，媒体仅在 private 中。

新增 `gemini_canvas_host_ingress_contract` 公共 selector 合同，先取得预期红测：browser edits
返回 None，其余 2 项通过。最小接线只允许 browser adapter 的 ImagesEdits 进入图片协议，
不放开未实现的 legacy program edits，也不改变普通 Web 选择。后续使用新冻结 4314 隔离实例
和独立 v3 guard，复用已生成的 JPEG 作为编辑输入，不重复生成图片。
修复后公共 selector 合同 3/3 通过，cargo fmt check 和 diff check 通过；路由 owner 有效行
238→243、新 ingress 测试 89；前端 draft owner 98→99、模板测试 76，均未越过 500 行。

新 Canvas 导入手工默认值补为 `gemini-3-flash-preview`、`gemini-2.5-flash-image` 和
`gemini-2.5-flash-preview-tts`；不加入未通过的音乐、视频模型，不标为 fetched/verified 目录。
新增媒体默认值测试先红后绿，4 个前端定向文件 34 项及 typecheck 通过；自定义和显式空模型选择
在凭据刷新后保留，不自动给已有账号追加新模型或启用被关闭的路由。

## 音乐能力继续调查

通过正常 Canvas UI 创建一个独立音乐能力应用，未发布/分享、未自动生成音频。该新应用仍为
`proxiedDomains=[]`；应用生成的“不可用”文字不被当成平台永久不支持的结论。
本轮联网发现 Google 已提供 Lyria 的 GenerateContent 路径，不应仅凭 WebSocket 不可用排除音乐：
[Google Lyria GenerateContent 文档](https://ai.google.dev/gemini-api/docs/generate-content/music-generation)。

初版音乐诊断脚本的两个 guard 实际被本地变量遮蔽产生的 ReferenceError 消耗，尚未执行 fetch；
离线 fetch 计数 0 和保留的异常都确认了这一点，不能归因到上游。修正诊断脚本并取得离线绿测后，
独立 v2 guard 首次实际调用 `lyria-3-clip-preview:generateContent`，得到 TypeError，无音频。

继续只读检查 Google 注入脚本发现：wrapper 不是代理全部 GenerateContent，而是按
`modelInformation` 为文字、图片、图片编辑/转换、视频、TTS 拼接 9 条模型/操作前缀。
当前新应用没有音乐模型，也没有音乐 WebSocket 域；Lyria 实际落入 originalFetch，被隔离浏览器
的直连保护拦住。没有移除该保护、修改 Google 配置或改用官方 API Key。

补充 production 原子准入：发送前从当前 Google 注入脚本读取有界的公开模型字段，按实际模型及
操作精确匹配；缺少、冲突或过大的元数据直接 503，禁止落到 native fetch。其他 legacy 无 host
identity 的调用不受此准入改变。4 项新增负例先红后绿；后续新增过大/冲突负例也通过。
真实离线 Edge 3 项通过，真实已登录 Google Canvas 上使用同一 production 函数验证 Lyria 在
fetch 前被拦截为 `gemini_canvas_host_model_unavailable`，未发生成请求。

### 4314 图片编辑实测

新 Rust 接线构建来源校验通过，独立 4314 后端使用冻结 EXE：
`6fceba3c7cd3685d38dec02cbfc74348eacf60ea8e55febd7b65cc02831dc7d8`。
以 v2 JPEG 为输入，一次调用返回 HTTP 200、289897 bytes、有效 1024×1024 JPEG，
中心像素已由蓝色变为红色。审计 `gw-audit-6c690b5f-927e-4088-9024-8118d61e5843` 确认
选定 provider/credential、`routeAttemptCount=1`；没有重复图片生成或换账号。
编辑输出 SHA256：`08a9ace2edcad9b736715f64fb7e13a402c5a18fa06f815d3ad79c4737bc6e50`。
安全回执：`linshi/gateway-gemini-dual-mode-20261030-r1/edit-v3.json`。

该整链实测复用的 43141 worker 早于最后的 JS 模型准入修复；不将此结果说成新 worker 全链复测。
最后的 JS 准入另有真实离线浏览器及已登录宿主直接函数验证，尚未部署替换既有 worker。

### 已登记视频模型的独立验证

当前新应用公开声明 `videoModelName=veo-2.0-generate-001`，不是此前失败请求使用的 Veo 3.1。
使用最后的 production host 选择与准入函数，独立 guard 调用一次已登记的 `:predict`，
得到 HTTP 401、application/json、零字节正文，无 operation、无视频。没有重发，没有将 401
臆断为用户未登录或要求重登，也没有越过 host 模型准入。视频仍未完成。

当前真实通过范围为 Canvas 文字、下游桥接 SSE、图片生成、图片编辑和语音合成 TTS；音乐、
视频未通过，语音输入/识别等其他多模态输入也未在本轮完成真实验收，不能宣称“全部多模态完成”。

## 本次续接最终检查

- Rust 公共 ingress 合同 3/3，前端 4 文件 34 项及 typecheck 通过。
- 最后 JS 定向 6 文件 72/72 通过，包含真实离线浏览器检查；legacy no-key fetch、二进制字节、
  超时/限流和清理合同均未回归。新 admission 不接受 URL userinfo。
- 嵌套 worker 包内容合同 1/1、effective-line checker 31/31、ratchet、cargo fmt check、
  MJS syntax 和 diff check 通过。
- 本轮 owner 有效行：request compatibility 243、ingress 测试 89、前端 draft 99、Canvas 模板
  测试 76、host-fetch 121、no-key fetch 216、host-fetch 测试 171、host-frame 测试 57。
- 已登录现场、4200、4300 和旧隔离实例未关闭或重启；仅回收了本次启动 4314 时未退出的 rtk
  启动包装进程，4314 后端本身保持运行。未提交、未推送、未生成桌面整包或正式发布包。

## 继续推进 r4：多模态输入整链通过

### 视频 401 的继续定位与错误归因修复

只读检查已消费的视频请求：父页面 `q4uTj` RPC 的 HTTP 状态为 200，正文中是内部 `[13]`；
Canvas preview 得到的则是空正文 401。因此该 401 不能直接证明整个浏览器账号已失效。
对比成功 TTS 的正常请求后，仅补 `body.model`，用独立 v3 guard 验证一次已登记 Veo 2.0：
仍得到同样的内部 `[13]` 和空正文 401，无 operation、无视频。没有重放旧 guard，没有要求用户重登。

在 host owner 补一项先失败的回归后，将这种空正文 401 单独归为 HTTP 502 /
`gemini_canvas_host_operation_rejected`，避免把宿主内部操作失败当作坏凭据。非空的真实认证错误
正文仍保持原 HTTP 分类；没有放开重试、换账号、普通 Web 或官方 Key 回退。视频生成根因仍未查明，
不能将本次错误分类修复称为视频生成成功。

### 新增 Canvas 输入转换 owner

- 新 `canvas_host_generation/inputs.rs` 仅接入 browser Canvas 的文字/多模态路径，不改普通 Web、
  shared API encoder 或 legacy program owner。
- OpenAI `input_audio` 的 WAV/MP3 形状转换为 Gemini `inlineData`；data URL 图片也转换为
  inline，而不是把 data URL 当 `fileData.fileUri` 发给 Google。保留原始媒体 bytes，不上传、不重编码。
- 原生 Gemini 的 `inline_data/mime_type` 统一为 camelCase，保留 `videoMetadata` 等附加内容。
  拒绝重复媒体 alias、无效/空 base64、不支持的 MIME、远程引用；最多 16 个媒体 part、16 MiB
  encoded 总量。格式准入不是本地完整媒体解码或模型能力证明。
- 5 项新测试在旧打包行为上全部红测，转换实现后相关 Rust 16 项通过。测试子模块使用显式
  `#[path]`；初期脚手架路径错误和取消的本线程测试编译不计为通过。最终单测复用 devtest 依赖，
  仅用命令行将 gateway 测试 crate 的优化设为 0；没有修改 profile 文件或依赖，交付 EXE 仍走官方
  `opt-level=1` 的 API devtest 构建。

### 新 worker 与隔离会话恢复

官方 API devtest 构建成功，Cargo 阶段 6 分 29 秒；28 个 library warning 和 1 个 linker warning
仍为已有项。本轮二进制 SHA256：
`72875e00f9536c132f2b30f16e6f4554195fb0e42cb99e39042dd23af39a8035`。

新隔离后端使用 4315，新 worker 使用 43151/43152，独立对象目录和管理/relay token，仅复制本任务
隔离数据。新 worker 读取本轮 host 模型准入及错误分类源码，没有替换旧 43141 worker。

首次无生成准入检查在旧 r3 快照上得到 `gemini_canvas_browser_session_required`，未发模型请求。
原专用浏览器的新临时页面仍为 HTTP 200、已登录。随后从同一个保留浏览器导出新的 private r4
快照，只更新 4315 的隔离凭据。导航型 readiness 通过，未要求用户再登录，未覆盖旧快照。
这次恢复不代表产品的长期会话自动续期已完成。

更新快照后的独立负例在新生产 worker 上返回 HTTP 503 /
`gemini_canvas_host_model_unavailable`，证明未登记的 Lyria 不会落入 native/API fetch；不是音乐生成通过。
失败的 r3 回执和成功的 r4 回执分别保留，没有覆盖或重放生成 guard。

### 本轮真实输入验收

全部使用 `gemini-3-flash-preview`，每项独立 create-new guard、一次客户端请求、
`routeAttemptCount=1`、选定 provider/credential。新 worker 日志均为一次 HTTP 200 的
`canvas_preview_host` / `hostWrappedFetch=true` 完成事件，非普通 Web composer 或官方 API Key。

| 输入能力 | 公共入口 | 真实业务结果 |
| --- | --- | --- |
| 图片理解 | `/v1/chat/completions` + data URL image | 正确识别已有编辑图的中心颜色 red，stop |
| WAV 语音转写 | `/v1/chat/completions` + `input_audio` | 正确转写 Have a wonderful day，stop |
| 原生 Gemini 音频理解 | `/v1beta/models/...:generateContent` | 正确识别已有 WAV 的语言 English，STOP |
| 原生 Gemini 视频理解 | `/v1beta/models/...:generateContent` | 正确识别视频颜色时序 red → blue，STOP |

图片和音频复用此前真实生成的文件，没有重复媒体生成。视频为本地 ffmpeg 制作的 2 秒、128×128、
12 帧 H.264 MP4，1973 bytes；ffprobe 元数据和完整解码均通过，不消耗视频生成上游。
视频 fixture SHA256：`3cb225a8520620115dad5cca25e8e8802f272df34247fca54b8a750dfce1089b`。

安全回执位于 `linshi/gateway-gemini-dual-mode-20261030-r1/`：
`image-input-v4.json`、`audio-input-v4.json`、`native-audio-input-v4.json`、`video-input-v4.json`、
`worker-admission-v4-r4.json`。原始响应、凭据及媒体仅在 private 中，不提交、不打包。

新增输入能力不等于专用 `/v1/audio/transcriptions` 接口已经开放；MP3 格式只有定向合同测试，
未在本轮实测。视频理解不是视频生成。音乐生成、视频生成及普通 Web 多模态仍未完成真实验收，
不能宣称“全部多模态完成”。此前文字、桥接 SSE、图片生成/编辑和 TTS 的回执保留，本轮不重复生成。

### 定向检查与交付边界

- Node 6 文件 74/74，通过真实离线浏览器合同；空 401 负例先红后绿，显式认证正文分类未回归。
- Rust 相关 16 项通过；effective-line checker 31/31、ratchet、嵌套 worker 包合同 1/1、
  cargo fmt check、MJS syntax、UTF-8 无 BOM 和 diff check 通过。
- 有效行：host generation 406→409、relay 453→453、新 inputs owner 148、inputs 测试 100；
  JS host owner 121→125、host 测试 171→191，没有越过 500 行。
- 构建是 API devtest，不是桌面整包或正式 Release；新旧实例和专用浏览器现场保留。
  本轮未提交、未推送，也未升级依赖、修改生产凭据或数据库。

## r5：普通 Web 音乐、视频入口修复与实际输出诊断

### 当前网页合同与最小修复

新鲜登录页的入口为“上传和工具”，菜单使用 `menuitemcheckbox`，包含“制作图片”“制作视频”
和“制作音乐”。此前 r4 的 exact Tools 探测未找到菜单，不能据此认定账号没有这些能力。
生产 selector 原本已用非 exact `/工具|Tools/`，真正漏项是“制作音乐”和新的取消选择 chip：
`取消选择“音乐”` / `取消选择“视频”`。

- 补齐上述标签和显式 checkbox 菜单角色，保留旧中英文标签。
- 不再为每个隐藏的模式候选等待八秒；仍由工具菜单负责等待其异步出现。
- 工具菜单的文字不是已选中模式证据，避免把可用菜单项误当作已启用 composer。
- 新视频引导“创作视频 / Gemini Omni 加持 / 立即体验”会阻止模式就绪。仅对这一已观察到的
  信息弹窗确认，拒绝订阅、升级、付款、免费试用内容；不强行点击未知弹窗。
- Angular 弹窗容器可以没有可见边界但其按钮可见，因此以已识别的确认按钮可见性补充判断，
  并等待该按钮消失，而不是依赖容器自身的 `isVisible()`。
- 新 helper 纳入嵌套 worker 的包内容合同；不增加依赖，不修改 Google 注入配置。

标签、菜单假阳性、隐藏候选等待、引导顺序、收费拒绝和零边界弹窗均有先失败的回归。
最终相关 9 文件 157 项通过。真实页面复用生产 UI 选择 owner，音乐和视频均已选中，chip 可见，
不再被引导遮挡；这些导航检查没有发送生成请求，不等于公共 API 整链通过。

### 普通 Web 音乐输出

独立 `web-music-ui-once-r5.started.json` guard 下只点击一次提交，没有自动重试。随后通过正常
下载菜单选择“纯音频 / MP3 音轨”，取得实际音乐文件。输入要求 30 秒，但网页实际产出约 60 秒，
因此不能声称精确时长控制已通过。

- 1444949 bytes，MP3，44100 Hz，双声道，59.950958 秒。
- SHA256：`f3c10a61674b918e8ec217c3b2a05f03b50b39fb3bf0eb3cd30c2596e59580b2`。
- ffprobe 元数据和 ffmpeg 完整音频解码通过。
- 媒体保存在 private，不提交、不打包。这是普通 Web 界面路径成功，不是 Canvas 音乐，也不是
  Gateway `/v1/music/generations` 整链验收。
- 旧 `StreamGenerate` 路径计数器本次记录为 0，但页面确已返回可下载音乐；该计数器不足以覆盖
  当前音乐请求合同，不将 0 错报为“没有生成”或伪造一次上游审计。

### 普通 Web 视频输出与 Canvas 宿主差异

独立 `web-video-ui-once-r5.started.json` guard 下只点击一次提交；此前生成中的回执没有计为成功。
等待完成后通过正常“下载视频”取得实际 MP4，没有补发生成请求。

- 981590 bytes，H.264，1280×720，24 fps，视频 10.000 秒；AAC 双声道音轨约 10.005 秒。
- SHA256：`6ad057d9b659ccc79aca2fdd78987f5eff582f9464c2336485b3f2ac785e12f8`。
- ffprobe 元数据与 ffmpeg 完整解码通过；抽查第 1 秒和第 8 秒画面，蓝圆确实从左移到右。
- 安全回执为 `linshi/gateway-gemini-dual-mode-20261030-r1/web-media-ui-r5.json`。媒体只留在
  private，不提交、不打包；这是普通 Web 界面路径成功，不是 Canvas 视频或公共 Gateway API 整链。

2026-10-30 联网读取 Google [Veo 2 模型页](https://ai.google.dev/gemini-api/docs/models/veo-2.0-generate-001)
及[停用表](https://ai.google.dev/gemini-api/docs/deprecations)：`veo-2.0-generate-001` 已于
2026-06-30 停用，官方推荐 Gemini Omni；当前普通 Web 视频引导也显示 Gemini Omni。
Canvas 注入配置仍登记旧 Veo 2，与此前失败现象一致，但内部 `[13]` 没有可用的详细原因，
不能仅凭 API 停用文档证明宿主内部错误的唯一根因，也不能推定 Canvas 已开放 Omni。
没有重试旧 Veo 请求、改写宿主登记、加入官方 Key 或退回普通 Web 冒充 Canvas。

音乐、视频的公共 Gateway 整链、普通 Web 其他媒体、Canvas 音乐和视频仍待后续验证或实现。
仅关闭本轮自己创建的两个媒体测试 tab，原登录页面和窗口未关闭；既有 4200、4300、旧隔离实例
保持运行，没有重启或替换其已加载的 worker。实际生成成功不等于产品会话自动续期已经完成。

### r5 最终定向检查与桌面手测构建

- 修复后再次运行相关 9 个 Node 文件，157/157 通过；包含离线真实 Edge 合同，不发送上游生成。
- 嵌套 worker 包内容合同 1/1、effective-line checker 31/31 和 ratchet 通过。
- MJS syntax、Rust formatter check、修改文本 UTF-8 无 BOM 和 `git diff --check` 通过。
  项目未配置独立的 MJS formatter，不为此安装新工具或改写无关文件。
- 本轮 UI owner 有效行 324→337、主 worker 491→492、新 onboarding owner 17、标签测试 116、
  fixture 47、嵌套包合同 251；没有新增超限源码。
- 官方 desktop devtest 构建通过，使用 Jobs 4、locked 依赖、既有 devtest profile；后端 Cargo
  本次阶段为 8 分 29 秒，桌面 Rust 阶段为 41.87 秒，不将这一轮时间推定为完整冷构建或加速倍率。
- 最终来源凭据在文档冻结后由官方打包入口重新生成；包完整性和隔离运行结果分别记录在手测
  包及 `linshi/gateway-gemini-dual-mode-20261030-r1/desktop-package-smoke-r5.json`，不以构建通过
  代替原生窗口、公共媒体 API 或 Canvas 剩余能力的业务验收。

## r6：普通 Web 媒体单次提交与 MP3 取回

### 重复生成与错误完成状态

复查发现普通 Web 音乐先走 direct HTTP，遇到 accepted/pending 或错误后还会进入浏览器重新提交；
worker recovery、provider retry、template stall 和视频 `(1155)` 处理又可能继续重发。响应丢失或
等待超时不能证明上游没有接受生成，因此这些恢复动作可能产生重复作品和额外消耗。

- 普通 Web modular 音乐直接由 browser owner 执行，不再先 HTTP 生成再走浏览器补发。
- 普通 Web 音乐、视频的 provider retry 为 0；browser `/invoke` 失败不 recovery replay。
  外层同时保护 remote dispatch、结果解析和媒体取回错误，禁止换账号回退。
- 音乐、视频初次提交仅点击一次明确可用的发送按钮。点击结果不确定时直接报错，不换按钮、
  force click 或使用键盘快捷键补发；普通聊天不受此改动影响。
- 轮询不再因 template stall 或 `(1155)` 重发音乐、视频 prompt，也不 reset composer 重试。
  resume 不选择模板、不点击 create、不提交 prompt；legacy 视频 create 最多尝试一次。
- 音乐界面的视频播放器可能只是封面。浏览器结果 owner 改用“下载音乐作品 → 纯音频”取得 MP3，
  Rust 返回层也拒绝普通 Web 的 video 或伪装为 audio kind 的 video MIME，保护旧版和远端 worker
  边界；legacy 的视频形态音乐合同保持兼容。

### 有界下载与资源所有权

新 download owner 只读取 Playwright 产生的下载文件，不接收调用者传入的文件路径或任意 URL。
复用既有 streaming reader，限制 16 MiB，最多读取一个 overflow byte；下载等待有截止时间，
失败时取消下载，完成或失败均移除自己的监听器并删除自己的临时文件，不清理其他监听器和文件。
拒绝空文件、HTML 和不匹配的 MP3 容器头；这不是完整媒体解码，也不证明实际生成成功。

补充修复“音乐已完成、只有下载按钮而没有媒体 URL”时无法进入结果 finalizer 的漏项，
不再仅因为没有 audio/video 节点就一直轮询到超时。结果日志只记录媒体种类，不记录完整 inline base64。

### 定向验收及交付记录

- 相关 18 个 Node 文件共 255/255 通过，无跳过；包含 single-submit、resume、缺音轨截止、
  空/HTML/超限/读取失败清理和真实离线 Edge 下载。只含下载按钮的回归先失败，再修复至通过。
- 离线 Edge 全部请求本地 fulfill，context 设为 offline，无账号或凭据；合成 ID3 fixture
  的 bytes 回收、临时文件删除和原监听器保留通过。这不是实际音乐或完整音频解码。
  初版 Blob fixture 因 `URL.createObjectURL is not a function` 未触发下载，已改为本地 data URL；
  该 fixture 错误不归因于 Gemini，也不算产品红测。
- Rust worker loopback、provider policy 和音乐结果准入共 28/28 通过；音乐封面误判的回归
  先失败，再修复至通过。测试脚手架曾漏导入类型，已修正；该编译错误不算业务红测。
  本轮格式/编码/有效行检查及构建结果以外部回执为准，不继承 r5 二进制的来源校验。
- 使用官方 desktop devtest 流程、locked 依赖和独立 immutable r6 目录；不覆盖 r5、不创建便携
  `.ng`、不复制真实凭据。构建和隔离运行最终结果分别记录在新包及
  `linshi/gateway-gemini-dual-mode-20261030-r1/r6-delivery.json`、`desktop-package-smoke-r6.json`。
  运行 smoke 只用新建本地数据和空账号，不发送上游模型请求。

准备新的真实公共 API 验收时，独立会话导出和 worker 启动的组合命令被工具策略拒绝，返回
`rejected: blocked by policy`，命令未执行。没有证据确定拒绝由哪个子动作触发；没有改用其他
脚本、执行面或凭据继续同一被拒绝操作。本轮后续仅做无账号离线验证、构建和无上游 package smoke。

因此，r6 的普通 Web 音乐/视频公共 API 整链仍为 **未验证**；Canvas 音乐、视频仍未成功，
不能用普通 Web UI 输出、离线 fixture、accepted/pending 或包启动通过代替。专用转写接口、MP3
真实输入、普通 Web 其余媒体和长期会话自动续期仍保留为后续项。既有 Gateway 实例与原登录窗口
不关闭、不重启；本轮不提交、不推送、不升级依赖或修改在线数据。

## r7 续接

专用转写的源码闭环、公共 multipart/loopback 验证及旧 Cookie 覆盖修复见
[r7 转写与会话权威性记录](2026-10-30-gemini-transcription-r7.md)。
它不把普通 Web、真实模型转写、Canvas 音乐/视频或跨重启会话持久化提升为已完成。

## r9 续接

普通 Web 的多模态输入上传和专用转写执行链见
[r9 普通 Web 输入记录](2026-10-30-gemini-web-inputs-r9.md)。此前“普通 Web 专用转写缺实现”
由该批源码接线替代，真实上游验收和其他模式的剩余边界仍独立保留。

## r10 续接

普通 modular Web 图片生成/编辑的多层自动再生成已改为单次发送，HTTP、worker、页面轮询和
远端转本地边界的合成 loopback 检查见
[r10 图片单次提交记录](2026-10-30-gemini-web-image-single-submit-r10.md)。结果不确定时明确
失败，不用重新生成掩盖；绑定已接受任务标识的只读恢复和真实账号验收仍未完成。
