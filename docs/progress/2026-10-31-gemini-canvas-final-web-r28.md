# Gemini Canvas 音乐／视频最终网页核查 r28

> 后续状态由 [r29 最终收尾记录](2026-10-31-gemini-development-closeout-r29.md) 接续：预览已恢复，
> 实际单次点击视频生成返回 HTTP 400；按用户最新决定，本轮功能开发完成并保留已知限制。
> 下文是 r28 历史现场，不再代表仍在等待预览恢复或需要继续开发。

## 结果与验收边界

2026-10-31 使用既有已登录 Edge，通过真实 Gemini 网页选择 Canvas、新建
`Canvas Native Media Final Check`，并检查应用代码和 Google 注入的实际宿主声明。
没有将普通 Web 音乐／视频的既有成功计入 Canvas，也没有用官方 API Key 或模拟媒体替代。

| 项目 | 当前结果 | 能证明什么 |
| --- | --- | --- |
| Canvas 音乐 | `unavailable_in_observed_host_contract` | 当前已加载宿主没有 musicModelName／Lyria 通道，无法提供原生音乐生成合同 |
| Canvas 视频 | `blocked_unverified` | 宿主声明 Veo，但本次新应用预览受阻，没有取得可播放输出，不能断言 Google 普遍不支持 |
| 模块所有接口全通过 | 否 | Canvas 视频最终网页验收受阻；既有 Web 原文朗读失败也没有被消除 |

用户允许在确认 Canvas 两项不支持时按能力边界收尾。但本轮不能把预览／控制工具阻塞
等同于平台不支持，因此没有新增“两项均不支持”或“所有接口完成”的结论。
若按当前已证实可交付能力列范围，保留 r27 成果和上述例外；不得改写为全绿。

## 实际网页过程

1. 保留原 `Media Capability Check` Canvas；只读核查其 Google 注入脚本，未刷新、改写、
   点击执行、导出或分享。
2. 从真实工具菜单选择 Canvas，创建独立最终检查应用。第一次生成的代码探测
   `window.ai`／`window.googleCanvasMedia`，不是实际 Canvas 合同。没有点击这些按钮，
   更没有把应用自称缺少能力当成独立证据。
3. 根据真实宿主声明，通过一次正常对话修正应用：视频使用已声明的
   `veo-2.0-generate-001:predictLongRunning`，单次普通 fetch、sampleCount=1；禁止个人
   API Key、代理绕过、自动提交、重试、猜测轮询／下载路径及伪造成功。
4. 只读审查修正代码。音乐面板中的缺失提示是应用硬编码，独立证据仍是实际注入声明。
   视频完成 JSON 分支也不足以证明可播放媒体；即便页面标成功，仍须验证真实媒体。
5. 新应用预览持续为空，iframe 文档仍为 `about:blank`，没有出现生成按钮。尝试正常
   代码／预览切换、全屏、一次重载和重新打开同一已保存对话，结果相同。
6. Canvas 自带控制台无明确错误；只读 CDP 诊断提示 Browser Use 正在处理暂停的文档响应，
   无法继续该诊断。实际观察到的 shim 地址单独打开能显示 `SafeContentFrame`，所以也
   没有证据将问题归因于网络不可达。未修改浏览器安全配置或绕过该限制。

本次音乐与视频生成请求提交数均为 **0**；没有生成后重复提交、未知结果重放、付费升级、
发布、分享或会话导出。检查应用的创建／修正提示不计为媒体生成成功。

检查页（需要原账号登录）：
<https://gemini.google.com/u/2/app/4d5b07e8df22faae>。
原 Canvas 和该检查页均保持打开；两个纯诊断副本已关闭。

## 真实宿主只读声明

- text：`gemini-3-flash-preview`
- image：`imagen-4.0-generate-001`
- image edit：`gemini-3.1-flash-image`
- image transform：`gemini-3-pro-image`
- video：`veo-2.0-generate-001`
- tts：`gemini-2.5-flash-preview-tts`
- 没有音乐模型／Lyria 声明。视频允许路径包含 `predict`／`predictLongRunning`，未见
  operation GET／文件 GET 声明；提交入口存在不等于轮询、取回、可播放输出已闭环。

本轮联网重新打开的 [Google Canvas 官方帮助](https://support.google.com/gemini/answer/16047321?hl=en-GB)
描述文档、应用、slides、代码及应用中的文本／图像生成功能，也描述文档 Audio Overview。
Audio Overview 不是音乐生成；文档未列出某能力本身也不能证明永久不支持。

## 证据与保留项

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-canvas-final-web-20261031-r28/`。

- `host-readback.json`：本次只读提取的非敏感实际模型与通道声明。
- `final-web-check.json`：网页过程、零媒体提交数、具体阻塞和未通过边界。
- `canvas-preview-blocked.png`：真实全屏 Canvas 空白预览及控制台截图，不是“不支持”的证明。

r27 已有 14 项真实 Google 公共 API 成功记录不变，见
`2026-10-31-gemini-public-multimodal-r27.md`。其中普通 Web 音乐／视频成功不计入 Canvas。
Web `/v1/audio/speech` 的 `invalid_gemini_web_speech_response` 仍为失败、未验收，本轮未重放。

本轮只新增检查证据与本文；未修改生产源码、配置、凭据或数据库，未重启／切换 Gateway
实例，未构建新包、重跑生产代码测试、提交或推送。既有 r27 devtest 包保持不变。
