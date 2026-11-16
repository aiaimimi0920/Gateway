# Suno 渠道与公开合同快照（2026-11-08 更新）

本文只冻结已联网取得的事实和未决边界，不表示真实账号验收，也不将 Suno 整卡标为 `offline_ready`。
R15 的 Web 调查与实现继续保留；官方 OAuth/MCP 是另一个需要接力的分支，不能用其中一种认证的成功替代另一种。

## 1. 官方 OAuth/MCP 已证实存在

官方 [Suno MCP FAQ](https://help.suno.com/en/articles/14486145)（页面编辑日期 2026-10-28）明确：
所有套餐（包括 Free）可用；生成按套餐消耗 credits；须登录并授权；生成歌曲进入同一个 Suno library。
[能力页](https://help.suno.com/en/articles/14551361)列出歌曲、Covers/Extends/Mashups、Speech、上传及查询套餐/credits。
这些是产品说明，不是可据以虚构 `tools/list` 或 `tools/call` schema 的依据。

匿名 GET `https://api.suno.com/mcp` 返回 401；已保存认证挑战，没有发起 initialize、注册客户端或登录。
受保护资源 metadata：

- URL：`https://api.suno.com/.well-known/oauth-protected-resource/mcp`
- `resource`：`https://api.suno.com/mcp`
- `authorization_servers`：`https://studio-api-prod.suno.com/api/v2/external/oauth`
- `bearer_methods_supported`：`header`
- `scopes_supported`：`api:v0 / credits:spend / download / library:read / library:search / library:write / offline_access / profile:read`
- 文件 SHA256：`d772f9e526c51a37a11bac72075bb62f0c622b24a860353e79577258463e6961`

授权服务器 metadata 的两个实际返回 200 的地址：

1. `https://studio-api-prod.suno.com/.well-known/oauth-authorization-server/api/v2/external/oauth`
2. `https://studio-api-prod.suno.com/api/v2/external/oauth/.well-known/oauth-authorization-server`

两者正文相同，SHA256 为 `ac7e8b30e43312a6a0d00e55793a2a6e8b7094f4bcd2416c15e3cba4f1125b6f`：

| 字段 | 实际值 |
|---|---|
| issuer | `https://studio-api-prod.suno.com/api/v2/external/oauth` |
| authorization_endpoint | issuer + `/authorize/` |
| token_endpoint | issuer + `/token/` |
| revocation_endpoint | issuer + `/revoke_token/` |
| registration_endpoint | issuer + `/register/` |
| response_types_supported | `code` |
| grant_types_supported | `authorization_code / refresh_token` |
| code_challenge_methods_supported | `S256` |
| token_endpoint_auth_methods_supported | `none / client_secret_basic / client_secret_post` |
| client_id_metadata_document_supported | `true` |

AS scopes 比资源 scopes 更广，不自动将它们全部申请，也不能把 scope 名当作已支持业务能力。
后续接入需要独立 audience、PKCE/state、注册/客户端身份、最小 scope、授权提交及公共持久刷新 owner。
2026-11-12 经用户明确同意，已登记 Gateway 独立 public client 与本机 redirect，并进入官方登录页；本次账号在 linking 阶段被拒绝，原文为 `Connecting AI apps to Suno isn't available for your account yet.`。未交换授权码或取得工具目录。仅客户端登记有真实证据，不代表 callback 完成或该账号/其他账号可用；完整回执见 [客户端登记与账号资格](progress/2026-11-12-provider-channel-expansion-suno-mcp-registration.md)。没有重试或绕过资格限制。
根路径 discovery 的 404、GitHub API 的 403 均保留为失败资料，不作为有效合同来源。

## 2. 额度与认证拆分

`suno-web` 保留 Clerk session/Cookie 身份；OAuth/MCP 候选名 `suno-oauth`，待真实工具及客户端合同冻结后创建正常模板。
目前未找到并验证适用于本项目的官方普通 API Key 注册/消费合同；不等于断言 Suno 不存在其他渠道。
第三方 Suno 代理服务的 Key 不是 Suno 官方 Key，不纳入官方 API 渠道。

同账号套餐 credits 与同库有官方依据，但**尚不能证明 Web/OAuth 的全部权益、速率、生成和下载额度完全等价**。
在额度验证前，按认证产品隔离管理，不能仅凭邮箱或相同模型名合池；也不能反向声称它们已经证实独立扣额。

官方 [How do downloads work?](https://help.suno.com/en/articles/13876865)（编辑日期 2026-09-03）另列 download credits：
Pro 20/月、Premier 60/月，另有 Free 的条件额度及 Studio 例外。
因此生成、播放、导出、付费转换/下载是不同操作：本批不增加 billing download、WAV 转换或其他隐式收费调用。
拿到 `audio_url` 不等于有效音频已读取，不等于已获得另一次付费下载的用户许可。

## 3. Web 实现参考与未决 wire

已只读固定 [OpenCLI](https://github.com/jackwener/OpenCLI/tree/36bbbd7f60bd361a96e62e417a28ea09d6c2266b/clis/suno)
commit `36bbbd7f60bd361a96e62e417a28ea09d6c2266b` 的 Suno 文档、auth/generate/utils/download 及 Apache-2.0 许可文件。
仅参考端点/状态/身份事实，不复制该项目实现或引入它的运行依赖。

- 参考实现使用 POST `/api/c/check`、POST `/api/generate/v2-web/`、POST `/api/feed/v3`。
- 同一提交返回两候选 clips；按原始 IDs 轮询，`complete / error` 为其处理的终态。
- 参考文档要求持久 `suno_device_id` 与每次请求更新 `browser-token`；Clerk 短期 bearer 另有刷新需要。
- 参考实现的 feed body 使用 `clip_ids`，本仓原有 `filters.ids.clipIds`；本批不凭单一第三方实现替换既有 wire。
- 参考文档使用 `chirp-fenix` 等新模型，本仓仍有旧模型兼容映射；不能把本批旧映射的合成测试当作当前模型实测。

匿名 `https://suno.com/create` 实际重定向至登录页；公开 chunk 获取遇到 `HTTP Error 403: Forbidden` 后停止。
没有绕过 challenge，没有用第三方源码代替官方当前 capture，也没有把登录 HTML 当作生成协议。
当前 Web 的模型、精确 feed body、Clerk 刷新、challenge、生成/下载权益仍需补可靠来源及集中录制。

## 4. 浏览器元数据子链的当前保证

[浏览器子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-suno-browser.md)已接原提交ID的同快照完整终态；页面全局feed可含合法其他歌曲，但只选原ID、恢复顺序，坏/重复/失败项、部分集合和默认等待超时不能假成功。不主动增加feed请求，不因此宣布现有wire/model合同已是当前平台事实。

点击前复用既有native response capture且未改Gemini/S06；精确origin/path/POST、128KiB单响应decoded-body准入、16项业务队列及1MiB JSON累计准入，native错误优先、正文省略、停止后迟到响应不准入。**1MiB是JSON累计准入，不是实际累计读取、wire或heap严格上限**：单响应已预准入后仍可能先读入至多128KiB再因累计超限被拒。redirect响应拒绝不等于阻止浏览器网络跟随；capture.stop不保证撤回页面请求或远端任务。

最终Suno53项与共享native37项Node去重90、合成包合同1项通过；真实隔离CDP验证七场景和同context其他页相同URL不串入、借用浏览器/原页继续可用。假audio_url只证明元数据路径，不证明有效音频或当前平台可用性；不会隐式执行下载/付费转换。

### 4.1 完成音乐的实际 MP3 准入

本轮在三种执行方式共同的 Music finalize 接入原 `audio_url` 匿名读取与完整 MP3 解码，合同及精确限制见[音频准入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-suno-audio.md)，运行证据见[音频子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-suno-audio.md)。原 JSON/clip/URL 不改；完成集合任一首失败即整次失败，显式 pending 不读取未完成音频或声称它有效。只读原播放 URL，不调用另收费的 download/WAV 转换；失败不重新生成、换号或 fallback。

资产传输不携带生成或 caller 身份，复用公网 HTTPS/DNS 绑定/无 proxy、redirect 的有界 reader。精确帧边界、ID3 边界与固定 Symphonia `0.5.5` PCM 解码外，额外拒绝该库会容错补静音的 reservoir 历史缺失。32MiB/首、64MiB/请求、2个资产 owner、20分钟/首 samples 上限及原 deadline 共同约束；取消逐帧生效，不能抢占已开始的单帧。

本合同不验证 MP3 CRC、听感、内容或版权；没有 MP3 统一 EOF，完整尾帧缺失可能仍是较短合法文件。证明本次读取字节不等于保证原 URL 将来不变；正常 router/合成上游/Local 结算也不替代真实平台、Server、独立进程或整包验收。浏览器精确 probe 与限定正常 UI→管理 TCP 联合现由§4.3承接；当前 wire、Clerk/device/challenge、OAuth/MCP 业务与完整渠道仍需接力。

### 4.2 基本浏览器任务的 native model 绑定

[模型绑定子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-suno-model-binding.md)已把路由解析出的 native model 传入本地/远端 worker，并在原生 Simple/TEXT POST 前核对基本操作和提示词、只改变 `mv`，保留页面 auth/challenge。两种已冻结 simple prompt 形态可发送，Custom/纯伴奏/续写/未知操作、未 arm 与第二次提交拒绝，不猜测 UI 模型选择器。

专用 `/v1/internal/browser-executor/suno-bound-v1/execute` 防止旧 service 忽略字段后仍生成；没有 legacy 路径重试，Suno 远端连接/HTTP/解码/业务失败已纳入共享 no-replay。新 worker 的 `wireModel` 必须被父进程核对后才进入原 finalize；此字段只证明发出请求的绑定，不独立证明平台最终执行型号。仍只声明既有两个 native ID 的合成合同，不宣布它们是当前可用目录。

受 Service Worker 控制的页面在点击前拒绝，但不承诺未来的 Service Worker、页面所有后续网络或跨请求 exactly-once。gate/capture/浏览器/lease 正常清理后才输出；强杀远端 lease 和平台已接受任务不因此被撤销。完整边界及运行证据见子批回执。

### 4.3 基本 BrowserBacked 精确音乐测试

[精确音乐测试子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-suno-browser-probe.md)已放开 provider/endpoint/credential 配置的 BrowserBacked，沿正常费用确认、精确账号、配置模型/alias、普通 bound sender 与全部 MP3 链；未知模式仍拒绝，不强制 DirectHttp。模型回执/远端失败/pending、任一音频坏和三阶段 Drop 均不能假成功或重放；原 Grant/Origin/费用/账号门禁保持。

正常 main→真实管理 TCP/Grant→合成 bound remote 联合已验：取消零 probe/上游，重新确认后一次 probe、两首 MP3、封闭未知费用结果；普通 Key余额/完整配置/revision保持。联合暴露并最小修复了共用目标判断对可选 `base_url:null` 的误拒绝，最终固定受众校验不放宽。Rust最终去重359 passed/5旧opt-in ignored、前端192及完整范围/来源/资源证据见回执；不是当前官网、真实生成、Server或原生包验收。以下“probe限制仍保留”仅记录历史停点。

## 5. 来源与接力资料

默认直连基本音乐的显式账号probe、逐次费用确认及正常账号表单由[单点测试子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-suno-probe.md)接续：仅空参数/精确配置模型与账号、完整MP3准入后成功；无文本评分、音频/URL返回或普通Key扣费，unknown保持未知。其收尾时因浏览器worker未绑定native model而保留unsupported；此历史缺口已由§4.2绑定与§4.3精确probe/正常网页联合承接，不能继续据旧停点断言当前入口仍封闭。当前wire/会话/权益、完整公共/Server/进程/包及OAuth/MCP仍未完成。

本地 browser worker 后续资源改造见 [worker 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-suno-worker.md)：复用受控进程、metadata 管道预算和取消回收；正常结果先关闭/断开及释放 lease 再输出。实际隔离 Chromium/CDP 测试证明正常成功/业务错误不关闭借用浏览器及原页，不是实际 Suno 生成。
该批当时未接的浏览器完整同ID终态/native响应准入由上方新子批承接，不能外推为所有响应/远端执行器的内存保证；完成音乐的实际 MP3 准入和限定 Local 公共结算由音频子批接续，其他格式不外推。默认直连及基本浏览器精确probe/费用/UI与限定正常网页联合均有上述离线证据；强杀远端lease对账、当前wire/Clerk会话、完整公共链、Server/独立进程/原生包仍缺；OAuth/MCP保持discovery-only状态。

固定公开文件、URL、获取时间、状态码与 SHA256 见：

- `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-20261106-contract/entry.json`
- `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-20261106-contract/discovery-sources.json`
- `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-20261106-contract/pinned-sources.json`
- `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-20261106-contract/pinned-code.json`

后续代码与测试证据单独登记，不把来源下载、编译、元数据响应或模拟服务器成功计为真实音乐生成成功。
