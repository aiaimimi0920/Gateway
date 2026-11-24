# Udio 渠道与当前公开限制（2026-11-08 复核）

本文区分官方公开产品事实、既有内部 Web wire 和本地离线证明，不代表真实 Udio 账号、音乐播放或下载验收。

2026-11-08 再次匿名联网读取下方 public API 与 UMG partnership 官方文章，均 HTTP 200，正文仍分别明确无公共 API、audio/video/stems 下载停用。原始 HTML、文本、最终 URL、时间及 SHA256 保存于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-20261108-readiness/public-sources.json`；此复核不新增调用/下载许可，不据旧 song URL 开发绕行下载或转存播放流。平台内有效播放、精确 probe/费用/UI 仍需合法产品合同与集中录制。

## 1. 认证与产品拆分

本次联网读取官方 [Udio public API](https://help.udio.com/en/articles/10756277-udio-public-api)，页面日期 2025-03-12，当前正文仍为：

> We know there's keen interest, but we don't currently offer a public API.

这证明当前公开说明没有提供公共 API，不证明永远不存在私有合作/企业渠道。尚未取得适用于本项目的官方普通 Key 或 OAuth 客户端/消费合同，不因第三方使用“Udio API”名称就新增官方 Key/OAuth 模板，也不把第三方账号和额度合入 Udio Web。

保留独立 `udio-web / udio`，实际为 Web session/Supabase SSR 分片 Cookie 与可选 browser runtime state。登录成功、Cookie 可解析和旧执行器存在，都不等于当前生成协议、刷新、权益或额度已验。与其他平台或第三方代理的额度关系不推定。

## 2. 生成、播放和下载必须分开

官方 [Changes associated with the UMG partnership](https://help.udio.com/en/articles/12683565-changes-associated-with-the-universal-music-group-umg-partnership)，页面日期 2026-02-17，当前明确说明 audio、video、stems 的下载已停用。

因此：

- Web 生成、平台内播放、资源元数据与下载/导出是不同能力，不因发现 `song_path / audio_url` 就宣称允许下载或已得到有效音频。
- 不增加绕过下载限制、录制播放流转存、第三方下载替代或隐式付费操作；保留 Web 调查，不用第三方 API 的成功冒充官方 Web 可用。
- 本批离线测试返回的 `https://asset.invalid/not-audio` 是假资产，只证明任务元数据处理，不是音频验证。
- 旧 image/video 入口的存在也不是当前 cover/video 产品能力证明，适用操作、播放/嵌入、权益与费用仍需单独核实。

## 3. 当前 Web wire 的证据边界

匿名 GET `https://www.udio.com/create` 实际返回登录页 `https://www.udio.com/login?redirect=%2Fcreate`，未登录、未发生成请求、未求解 challenge。

本批沿用仓库已有 POST `/api/generate-proxy` 与 GET `/api/songs?songIds=...&readOnly=false&checkIfReadyToStream=true`，不凭旧代码或合成服务器宣布它们是当前已证实平台 schema。`readOnly=false` 的实际副作用也未核实，不宣称完整只读合同。现有模型/参数、当前 session/challenge、Supabase 受众与 refresh、精确消费与额度均保留待接。

本地 [Udio 任务子批](progress/2026-11-06-provider-channel-expansion-udio-tasks.md)仅在这些旧入口上修复一次提交、原 ID 完整终态、错误/超时、native 响应准入和正常断开；不能据此标整卡 `offline_ready`。

## 4. 固定公开来源与接力

原始官方页面、获取时间、HTTP 状态、最终 URL、字节数与 SHA256 固定于：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-20261106/public-sources.json`

三次匿名获取均有 200 回执；两份帮助页分别约 72/74 KiB，create 获取结果约 163 KiB，实际内容以回执 byte/hash 为准。采集单文档上限 1 MiB，无凭据、登录、真实媒体或平台调用。

## 5. 路由模型与原生提交的离线绑定（2026-11-07）

[模型绑定子批](progress/2026-11-07-provider-channel-expansion-udio-model-binding.md)修复 builder 忽略 resolved model、caller aliases 改选模型，以及借用页面只填 prompt 却没有模型准入的问题。`udio-music` 仅为旧操作别名→旧默认 `udio32-v1.5`；其他配置值按 opaque identity 进入 canonical wire，不是官方当前型号目录。四个 model aliases 如出现须逐个一致，坏类型/非对象 gen_params 拒绝；需要其他模型应配置模型路由，而非附加 body 绕过许可。

借用页面的匹配 POST 先校验实际模型，不符 abort，不改写 native body/auth、不重提；只卸载自己的 handler，已知 Service Worker controller 在 click 前拒绝。真实隔离 CDP 已证三个错模型场景零生成 POST、原在途响应 identity 缺失拒绝及正常卸载。`route.continue()` 对本次精确 POST 的最终 ownership、SW 新注册、迟到 UI POST/远端撤销等边界详见子批；不声称任意浏览器流量被完全控制。seed/lyrics/数量等完整 UI 参数尚未保真，精确 probe/费用/UI/Keep与有效音乐/播放仍未完，不能把 metadata 当音频可用性成功。

本批重新联网读取官方 public API、[model updates](https://help.udio.com/en/articles/10756068-model-updates)及历史 [v1.5 公告](https://www.udio.com/blog/introducing-v1-5)，仍未取得当前内部 model_type 合同。观察记录为本批证据目录的 connector 文本摘要，不替代上方原始 HTTP 回执；历史公告不恢复已停用的下载能力。

后续 [Rust worker 子批](progress/2026-11-06-provider-channel-expansion-udio-worker.md)已接共享受控进程、stdio/超时/取消及调用 owner 禁止自动重放。最新 [结果准入与公共音乐元数据子批](progress/2026-11-06-provider-channel-expansion-udio-admission.md)已接 local/remote 原ID/目标/等待策略校验、128KiB远端解码预算与正常public music/精确账号/未知usage/Local次数结算、失败取消退款，并隔离caller Cookie覆盖；这些均是旧wire上的离线metadata合同，不是有效音频、当前平台/会话、远端取消/lease、跨请求exactly-once或完整媒体证明。精确测试/费用/UI/Keep、适用播放及image下载安全仍继续；无法从公开材料证明的当前业务合同留到集中录制，不伪造模型、Key/OAuth或下载能力，也不因此停掉其他原定渠道。

## 6. 既有图片输出的本地安全边界（2026-11-11）

[图片资产子批](progress/2026-11-11-provider-channel-expansion-udio-image-assets.md)为既有图片URL/base64路径补充目标、读取及解码准入，不是新增或确认当前平台图片权益。URL模式只做词法/IP literal准入，不新增下载或DNS验证，仍不代表图片有效；base64模式经匿名公网DNS固定、禁重定向、有界16MiB/张读取和PNG/JPEG完整解码。最多2张，共用原请求deadline，错误不重新生成、不泄漏地址或响应正文。音乐/视频元数据路径保持，不新增音频下载、播放流转存或限制绕行。
