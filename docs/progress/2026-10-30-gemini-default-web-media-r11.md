# Gemini 默认 Web 媒体入口接线与剩余清单（r11）

## 本轮确证的问题

默认 UI 模板使用 `gemini-web-chat` / `gemini_web_compatible`，不是 modular adapter。
旧公共 selector 把图片等请求继续标为 `gemini_web_chat`；JSON dispatcher 又没有默认 Web
媒体分支，最终报 `unsupported_gemini_web_request_plan`。r10 的 modular 图片保护因此不能
直接算成默认模板已经可用。新增定向合同先取得 3 项预期失败，另 1 项非法配置保护通过。

## 改了哪些内容

- 默认 Web 的图片生成、图片编辑、音乐、视频接入既有 Web 媒体 owner。路由层保留默认
  adapter、所选凭据及显式模型/协议限制；执行层在校验原始 call_flow 后，仅为本次媒体请求
  投影 modular owner，保持这两条 Web 路径的单次提交、错误不换账号和音乐音轨准入一致。
- 内部投影同时使显式 `special_adapter=gemini_web` 与执行 owner 相容，不改持久化配置。
  保留 Cookie、会话源、URL、headers 和显式 `pureHttpMode` / `pure_http_mode`。
  wire family 中沿用的 `gemini_canvas_*` 名称是已有内部协议名，不表示切换到 Canvas host。
- 默认 Web 图片/编辑/音乐/视频的外层 provider retry 也设为 0；原有普通文字重试不变。
- 默认 Web 文字、多模态理解、转写仍走它们自己的 HTTP owner，不转入 legacy text。
  默认 Web TTS 尚未审查完其旧执行链，本轮不开放；selector 明确拒绝，不再进入错误的通用 planner。
- 不自动给现有账号加模型或启用路由，不把模型登记当权限/成功证明，不修改账号、凭据或数据库。

图片默认选择已有 HTTP 实现，音乐和视频仍使用已有 browser owner；显式 Required 的音乐/视频
以及 Disabled 的图片编辑在网络前拒绝。这不是所有多模态都可纯 HTTP 调用的承诺。

## 实际通过的验证

- 新增公共合同最终 5/5：默认 selector、请求局部身份与策略保留、dispatcher、非法 call_flow
  以及完整本地 HTTP 鉴权/路由。HTTP 合同覆盖四条媒体 URL 的鉴权与执行策略负例，合成上游
  计数为 0；它不是成功媒体生成。
- Rust 定向单测 72/72：协议选择、旧 Web 路径、call_flow、图片单次提交、远端失败及外层重试。
  默认和 modular 图片各收到一次 loopback 远端请求；坏结果返回不可重试和 Abort，不转本地。
- 最终公共 ingress 三目标共 12/12，含上述 5 项、新旧 Web 转写及 Canvas 相邻合同；无 skip。
- effective-line checker 31/31、ratchet、官方 formatter check、diff check 和 UTF-8 无 BOM
  检查通过。没有修改依赖、baseline 或 exceptions。既有 unused/dead-code/linker warnings 保留，
  不声称零告警或 clippy clean。

本轮修改源码有效行：mixed lane 311→321，request compatibility 249→253，surfaces 148→154，
retry feedback 264→266，policy tests 255→260，browser media tests 203→217；新合同 211、
HTTP 合同 153。全部小于 500，没有增加超限文件。

本轮是源码及定向验证闭环，没有打包 r11 EXE，也没有替换在线实例。r10 immutable 包仍保留，
不包含本轮新增接线；本轮不拿旧包的 native smoke 当新源码验收。回执与日志保存在
`linshi/gateway-gemini-dual-mode-20261030-r1/r11/`。

## 当前还没有成功的内容

| 剩余项 | 当前状态 | 不能混淆的证据 |
| --- | --- | --- |
| Canvas 音乐生成 | 无可信可用宿主合同，明确 unavailable | 普通 Web 界面下载 MP3 不是 Canvas 音乐成功 |
| Canvas 视频生成 | 既有宿主操作失败，无 operation/成品 | 普通 Web 界面 MP4、父 RPC HTTP 200 都不是 Canvas 视频成功 |
| 默认 Web TTS | 仍未接入，旧链需要独立审查与保护 | modular 旧实现及 Canvas TTS 成功不能替代默认 Web |
| 图片已接受任务的只读恢复 | 未实现；只有 progress/locator 而无最终资产时失败 | 不恢复携带原 prompt 的再次生成 |
| 会话长期持久化 | 跨重启 checkpoint、多 worker CAS、S3 writeback 未实现 | r8 只解决 seed freshness 与空闲 context 失效 |
| 普通 Web 多模态公共 API 真实验收 | 仍未完成，包括本轮默认入口与 r10 单次图片 | 合成 loopback、网页下载和包启动不能替代 |
| 专用转写及 MP3 真实模型验收 | Web/Canvas 已有执行/公共合同，真实成功回执仍缺 | 媒体理解成功不等于专用 transcriptions 已实测 |

历史真实通过仍为：普通 Web 文字；Canvas 文字、下游桥接 SSE、图片生成/编辑、TTS，
以及 r4 的图片/音频/视频理解。这些回执本轮只读核对，没有重复触发上游，不能称作 r11 真实回归。

此前真实会话导出/隔离 worker 组合遭到工具 `rejected: blocked by policy`，本轮没有重试该
组合或换入口绕过，未读取/导出真实账号状态、未消费媒体生成额度。现有 Gateway 和旧包保持不动；
未提交、未推送。当前不能宣称 Gemini 全部多模态完成。
