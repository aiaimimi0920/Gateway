# Gemini 剩余模态：宿主模型兼容修复与执行边界 r22

## 本轮实际闭环

继续检查 Gemini Web / Canvas 的剩余音乐、视频能力时，发现 Canvas 准入会错误拒绝
Google 当前明确代理的兼容模型，包括已有默认图片模型 `gemini-2.5-flash-image`。
本轮已修复这个可证实的执行阻塞，但没有将音乐／视频的宿主缺口伪装成完成。

## 真实宿主证据与最小修复

在已登录 Edge 的保留 Canvas 预览中，只读检查 Google 注入的 wrapper 源码及公开配置。
除主模型外，Google 还明确扩展了以下四组兼容模型及操作：

| 配置字段 | wrapper 实际代理的操作 |
| --- | --- |
| deprecatedTextModelNames | generateContent / streamGenerateContent |
| deprecatedImageModelNames | predict / predictLongRunning / generateContent / streamGenerateContent |
| deprecatedGenerateImageModelNames | generateContent / streamGenerateContent |
| deprecatedImageTransformModelNames | generateContent / streamGenerateContent |

当前第三组包含 `gemini-2.5-flash-image-preview`、`gemini-2.5-flash-image`、
`gemini-3.1-flash-image-preview`。原 Gateway 准入只识别主模型字段，因此会在 fetch 前
将这些实际已声明模型拒绝为 `gemini_canvas_host_model_unavailable`。

修改 `scripts/gemini-canvas-browser-pool-no-key-fetch.mjs`：按当前宿主的精确字段和操作映射
接纳兼容模型，不添加任何猜测的音乐模型。仍限制 32 个注入脚本、512 KiB 源文本、每组
64 个模型和 120 字符模型标识；数组只解析有界 JSON 字面量，不执行注入源码。
同组多份声明内容冲突、非字符串、路径字符、非法 JSON 或超限均拒绝。相同列表的顺序或
重复项不构成冲突。原子的 document / wrapper 身份保护及失败不重放行为保持不变。

未修改 provider 默认值、所选账号、路由、凭据或数据库；兼容模型仍必须被当前宿主实际声明，
不是把旧模型硬编码为永久允许，也不是拉取了官方 API 模型目录。

## 当前音乐／视频边界进一步明确

- Canvas 音乐：当前配置和代理前缀仍没有 Lyria 或音乐模型。普通 Web 音乐、官方 Lyria
  GenerateContent 支持，均不等于该 Canvas 宿主暴露了同一能力。
- Canvas 视频：wrapper 声明 `veo-2.0-generate-001` 的 `predict` / `predictLongRunning`，
  但其代理匹配仅覆盖这些模型调用前缀，不覆盖 GET operation polling 或文件下载。
  因而不能拿官方 API 的长任务 GET 流程直接套到当前宿主，更不能允许其落入 native fetch。
- 同步 `predict` 是否可返回完整视频仍缺成功合同；此前内部 `[13]` / 空正文 401 的失败
  未被只读源码检查推翻。本轮没有重放这些请求，没有将失败归因为整站登录失效。
- 生产音乐／视频入口继续保留 `gemini_canvas_host_media_contract_unavailable`，没有回退
  普通 Web、官方 API Key、全局其他账号或修改 Google 注入设置。

本轮联网核对的官方资料：
[Canvas 帮助](https://support.google.com/gemini/answer/16047321?hl=en)、
[普通 Web 音乐帮助](https://support.google.com/gemini/answer/16901237?hl=en)、
[Gemini API 视频文档](https://ai.google.dev/gemini-api/docs/video)。
当前视频 API 文档已将 Veo 3.1 Preview 标为 2026-10-22 关闭，指向 Gemini Omni Flash；
这只说明官方 API 的变化，不能据此推断 Web / Canvas 的模型或故障原因，本轮未据此改动 preset。

## 已有 Web 音乐只读取回尝试

打开现成 `Calming Piano Piece` 历史结果，没有发送新提示、点击重做或生成。
确认实际控件为 `下载音乐作品`，菜单为 `纯音频` / `MP3 音轨`，与 production downloader
匹配；DOM 中的 cover video 已加载，duration 约 59.967333 秒。它是历史结果，不是本轮新生成。

一次点击纯音频后，浏览器控制工具在 20 秒内未收到 download 事件，未获得可验证的本地文件。
页面没有显示明确错误；不能凭此断定 Gemini 上游下载失败、账号失效或 production Playwright
downloader 有同一缺陷。没有重复点击、重新生成，或改为返回封面 MP4 冒充 MP3。
Web 音乐真实下载及公共 Gateway 返回仍未验收。

浏览器脚本会话曾超时重置；重新枚举后 Edge 的运行时 ID 已改变，已按实际 family/profile
恢复 Edge，而不是沿用数字 ID 操作其他浏览器。保留的 Canvas 应用继续打开，没有导出登录态。

## 验证与交付边界

- 新增兼容模型正例红测：39 项中 35 通过、4 失败，失败均为宿主已声明模型被提前拒绝。
- 修复后首轮 39/39；补充重复配置稳定性后，相关 Node 回归 114/114，包含最终 40 项 host owner
  合同、旧 no-key 路径、fetch、字节限制、超时／清理及 Web 音乐下载保护。两组有交集，不累加。
- 独立的真实离线浏览器／分发组 7/7，其中 4 项使用真实浏览器，新增图片兼容模型可执行及
  operation polling 零额外发送的承重测试；其余 3 项保护账号隔离与分发。不是真实 Google 生成。
- 嵌套 worker 包内容合同 1/1。三个修改的 MJS 文件 `node --check` 通过。
- 最终串行门禁已退出 0：effective-line checker 31/31、ratchet、`cargo fmt --all -- --check`
  和 `git diff --check` 通过。Git 的两条换行规范化 warning 不作为测试失败，也未据此改写无关文件。
- 官方 lexer 的最终有效行：no-key fetch 216→236、host-fetch tests 191→245、
  host-frame tests 57→71；三文件均经严格 UTF-8 无 BOM 和尾随空白检查。源码 SHA256 与计量
  保存到 `source-lines.json`，门禁结果及证据哈希保存到 `verification.json`。
- 本轮只修改 JS worker / 测试和本记录，没有改 Rust 源码，不用重复 Rust 编译代替宿主验收。
- 未替换在线 r20 PID 12444，未创建新 EXE 或正式发布包，未提交、推送或部署。

回执目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-modalities-20261031-r22/`。
包括 `before/`、`host-admission-red.log`、`host-admission-green.log`、
`host-admission-offline-browser.log`、`node-related.log`、`python-package.log`、
`music-readback.png` 和最终 `verification.json`。保留 r21 文档输入成果及其他未提交改动。

整个多模态目标仍未完成：Canvas 音乐需当前宿主允许的模型／传输，Canvas 视频需成功的同步
输出合同或允许的长任务轮询／下载合同；其他尚缺真实公共 API 验收的项目也不提升为通过。
