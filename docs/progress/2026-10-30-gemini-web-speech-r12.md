# 默认 Gemini Web 原文朗读入口（r12）

## 完成范围

本轮按“先完成容易完成项”只补默认 `gemini_web_compatible` 的语音入口及其本地合同。
原来的 selector 不接纳该入口；直接调用 binary dispatcher 则进入 generic planner，
得到 `unsupported_gemini_web_request_plan`。新增三个定向测试先获得这三项预期失败。

不能简单接入 modular 旧 TTS：它会先向模型提问，再朗读生成的回答，而非直接朗读输入；
旧链也没有本轮要求的单次导出和严格音频准入。新的默认入口不调用这条旧链。

## 改了哪些内容

- 默认 Web 的 `/v1/audio/speech` 有独立 HTTP owner，接入公共鉴权、模型/协议限制、
  call-flow 准备和 binary pipeline。没有把它改成 Canvas host、官方 API 或浏览器执行。
- 对成功、完整的当前 `/app` bootstrap，只向既有 `XqA3Ic` 合同提交一次原始 `input`。
  不发 `StreamGenerate`、不先让模型改写或回答、不刷新会话后重发、不下载任意返回 URL。
  不修改原始账号 payload；不要求 browser-state key，也不修改数据库、凭据或路由。
- 默认与 modular Web 的外层语音 provider retry 均设为 0。新默认 owner 的失败统一
  `retryable=false` / `Abort`，不自动换账号或回退浏览器。modular 旧内部执行链本轮未重写。
- 引导页和导出 POST 均禁止重定向；成功引导页必须含当前 token、build 和 session，
  不能用旧缓存补齐失效页面。错误保留安全的状态/分类，不复制私有正文、URL、Cookie 或 token。
- 输入上限 16 KiB UTF-8；引导页上限 8 MiB，导出正文上限 12 MiB，解码音频上限 8 MiB。
  长度声明及流式累计均检查，不依赖 Content-Length 单独限流。
- 音频必须来自精确 `XqA3Ic` 的完整、唯一成功 envelope。拒绝截断/错误长度/重复结果、
  无关 RPC、原始文本扫描、资产 URL 和只有魔数的伪音频。WAV 检查 RIFF/chunk、PCM 参数和
  样本对齐；Ogg Opus 检查页连续性、CRC、头/标签/数据包和结束页。
  这是容器级准入，不是完整音频解码或可懂度验收。

## 接口边界

```json
{
  "model": "gemini-web-chat",
  "input": "请原样朗读这句话。",
  "voice": "default",
  "response_format": "opus"
}
```

`gemini-web-chat` 是服务别名，不表示可以选择某个 Gemini TTS 模型。本轮不自动登记或
启用模型；现有账号和模型/协议 allow/block 规则继续生效，需要既有授权路由可选中该别名。

默认返回原生 Ogg Opus（`audio/ogg`），显式 `ogg` / `opus` 等价。`wav` 仅在上游确实返回
合格 WAV 时成功；不做隐式转码。MP3、PCM、具名 TTS 模型、非默认 voice、speed≠1、
朗读 instructions、SSE/流式和未知参数均明确拒绝，不静默忽略，也不把压缩字节冒充 PCM。
语言沿本次 bootstrap 的 locale（没有时使用配置回退），不按文字脚本猜测。

## 验证与交付

- 新语音合同 11/11 通过：含真实 loopback 发送顺序、原文/凭据保持、Ogg/Opus/WAV、
  格式错配、会话失效、重定向/HTTP 失败、大小上限、错误结果和公共 API binary 返回。
- 相邻公共入口合同 12/12 通过（默认 Web 媒体 5、Web 转写 1、Canvas 入口 6）。
  这些测试全部使用合成账号/本地上游，不是 Google 生成成功回执。
- provider retry 定向单测 25/25、协议路由 34/34 通过。连同上述公共入口，本轮 Rust
  定向验证共 82/82，没有 skip；没有执行全量回归或付费上游测试。
- effective-line checker 31/31、ratchet、formatter、diff check、UTF-8 无 BOM 通过。
  没有修改依赖、baseline 或 exceptions。既有 unused/dead-code/linker 告警仍存在，
  不声称零告警或 clippy clean。

新增生产文件有效行分别为：speech 输入合同 115、结果解析 94、容器准入 123、HTTP owner
138。既有文件：binary dispatcher 379→392、协议出口 51→54、执行器出口 37→39、请求兼容
253→254、surface 154→155、retry 266→267、policy tests 260→261、r11 媒体合同 211→217。
新测试文件为 193、127、106、119 行；没有新增超限文件。

本轮是源码及本地定向验证，不是新 EXE 交付、线上部署或真实账号语音验收。
没有重试先前被工具拒绝的真实 storageState 导出/隔离 worker 组合，也没有换入口绕过。
没有发起真实上游媒体生成、修改账号数据、替换在线 Gateway、提交或推送。

回执：`linshi/gateway-gemini-dual-mode-20261030-r1/r12/`。此前 immutable r10 包不包含本轮
或 r11 接线；不能拿旧包 smoke 充当新源码运行验收。

## 更新后的剩余项

默认 Web TTS 已从“未接入”推进为“源码与本地公共 API 合同通过，待真实账号验收”。
图片已接受任务的只读恢复仍缺实现，不能复用会触发刷新/重发的旧轮询；Canvas 音乐、
Canvas 视频仍缺可信可用宿主合同。跨重启 checkpoint、多 worker CAS、S3 writeback
以及 Web 多模态、专用转写/MP3 的真实账号公共 API 验收仍未完成。
