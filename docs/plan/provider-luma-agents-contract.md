# Luma Agents API 与 Web 渠道合同

接手开发见[当前剩余工作](../providers/luma-development-status.md)与[仓库工具](../../tools/luma/README.md)。现存 11 月 17–19 日证据已归档到[仓库证据目录](../evidence/luma/README.md)；第 5 节的 11 月 5 日临时资料本次已不存在，保留其历史出处，不作为当前可复核文件。

更新：2026-11-20。对应 [总计划 W10/R14](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。
初始官方公开资料于 2026-11-05 联网核实；2026-11-17 重新联网核对 [Usage 指南](https://docs.agents.lumalabs.ai/guides/usage/) 的晚到扣费、[Video reframing](https://docs.agents.lumalabs.ai/guides/videos/reframing/) 的视频扩画及 [Video editing](https://docs.agents.lumalabs.ai/guides/videos/editing/) 的视频重绘合同。本次没有读取真实凭据、调用真实生成 API 或安装 SDK。

## 1. 产品、认证和额度

- 新官方产品为 `https://agents.lumalabs.ai/v1`，`Authorization: Bearer <API Key>`，文档示例为 `luma-api-*`。不是旧 Dream Machine endpoint。
- 现有 `lumalabs` preset、`lumalabs_compatible` adapter 和 `lumalabs-web` 模板仍属于 Web board；`wos-session`、realm、signature/ws_token 与浏览器 worker 不成为官方 API 的凭据或 transport。
- 官方 FAQ 明确同一 client 的多个 Key 共享 quota；与当前 Web 的权益关系仍为 `unknown`。不合池、不共用秘密、不自动回退。
- 新方向为 `lumalabs-api`，后端 preset 与执行 owner 使用独立 adapter/profile；正常前端模板、材料保存、Keep/清空、精确账号测试、公共 pipeline 与 UI 验收须分别给出证据，不能从底层 wire 测试外推。

## 2. 已核实的官方生成合同

`POST /v1/generations` 返回 201；`GET /v1/generations/{generation_id}` 返回 200。
ID 为 UUID，状态为 `queued / processing / completed / failed`。只轮询原 ID；HTTP 出错、失败或不明消费不能自动重提 POST。
官方文档专门提示数据库错误可能已经启动生成；SDK 默认 5xx retry 会制造重复，不能照搬 SDK 默认重试。

图像模型为 `uni-1 / uni-1-max`，图像生成为 `type=image`，编辑为 `type=image_edit`。prompt 为 1–6000 字符。
生成支持九种 aspect ratio、auto/manga、png/jpeg、boolean web_search 以及最多九张 image_ref；manga 生成只允许竖向比例。
编辑须有单个 source，可用公开 URL、inline base64+media_type、file_id 或同 client 先前 generation_id，额外 image_ref 最多八张；编辑输出比例由 source 决定，不能把请求比例当成功控制。

视频当前模型为 `ray-3.2`。`type=video` 接文生视频、图生视频、多 keyframe 和单 generation_id 延展。
分辨率为 360p/540p/720p/1080p，时长为 5s/10s；HDR、loop、anchor、EXR 的组合有专门限制，不能把旧 Web ray3.14/action 参数照搬。
`video_reframe` 已接现有公共 `/v1/videos/generations` 的显式 type 及账号媒体测试，详见[扩画子批](../progress/2026-11-17-provider-channel-expansion-luma-video-reframe.md)。仅 `ray-3.2`，单个 source，目标比例必填；支持 5s/10s、四档分辨率及 API 的归一化 `source_position`。拒绝竖向比例＋1080p、HDR/EXR、loop、edit 和 image anchors/keyframes。URL/inline source 必须带 video MIME；源内容、≤10s 时长和同 client 归属由上游验证，本地不获取输入 URL。UI 已于 2026-11-19 提供归一化位置选择，仍要求逐次费用确认。参数映射、本地合成链与真实视频生成/播放分别验收。

`video_edit` 已接同一公共视频入口与正常账号媒体测试，详见[重绘子批](../progress/2026-11-17-provider-channel-expansion-luma-video-edit.md)。仅 `ray-3.2`，单个视频 source；API 支持自动控制、九档强度、五类 conditioning、单 guide 或至多 64 个配对 keyframes，以及 720p/1080p HDR。拒绝被忽略的比例、EXR、loop、end_frame 和不在本地 profile 内的 duration。UI 提供源视频、分辨率、自动／九档强度，并于 2026-11-19 补齐细粒度 conditioning、guides 和 HDR 表单。实际源内容、≤18s 和源帧位置由上游验证；本地合成链通过不等于真实生成／播放。

2026-11-19 补齐[图片分层验证](../progress/2026-11-18-provider-channel-expansion-luma-layering.md)与[视频高级参数界面](../progress/2026-11-19-provider-channel-expansion-luma-video-controls.md)：`layering` 已沿图片入口接单源／uni-1、1k/2k、有序 RGBA 图层与文本／字体 metadata，字体数字精确保留问题已修复；扩画 UI 可选位置，重绘 UI 已提供 HDR、五类 conditioning、单 guide 与最多 64 组关键帧，变更请求会撤销旧费用确认。上述更新限于已验本地范围，不代表真实生成／播放或完整费用验收。

LLM inference 已拆成[独立产品合同](../providers/lumalabs-inference-contract.md)与模板，不使用 Agents Key／credits。Files 小文件管理已在下方单独记录；重绘 EXR／duration、真实源与费用、取消后的追账及跨进程恢复等仍保留，不把独立 LLM 接入算作 Agents 媒体操作完成。

2026-11-20 [Files 小文件管理](../progress/2026-11-20-provider-channel-expansion-luma-files.md)接入正常已保存 Agents 账号。管理接口 `POST /v1/internal/gateway/console/luma-agents-files` 使用 management token、secret grant、精确 provider/credential 与配置 revision；操作为 list/get/upload/delete。上传只用固定官方 origin 的 multipart，Gateway 本地上限 8 MiB；文件名禁止路径和 header 注入。列表固定每页 25 项、原样编码 cursor；读取状态与 file_id，ready 后可复制供已有媒体输入复用。删除必须在 UI 确认，204 表示文件不能再被引用，不表示物理字节已清除。请求不自动重试、跟随跳转或跨账号回退；取消/超时可能留下已受理远端操作，须先刷新核对。授权前不缓冲上传 JSON；并发复用现有两个账号操作槽，输入读取限 30 秒、单次上游全流程限 60 秒、解码后 metadata 限 256 KiB。没有开放公共终端用户 Files API、presigned/PUT/complete、TTL/user_id、持久化幂等或跨进程恢复；真实账号与主网页→管理 TCP 验收另记。

2026-11-20 后续 [Files 签名上传](../progress/2026-11-20-provider-channel-expansion-luma-presigned-files.md)扩展同一管理入口，随后[六项收尾](../progress/2026-11-20-luma-six-closeout.md)将上限扩到 5 GiB。≤64 MiB 保留缓冲路径，超过后采用有背压的 multipart → storage PUT；API 凭据不发送给存储，无 proxy／公网 DNS 固定，原 file_id complete。小文件上游预算 180 秒，大文件 3600 秒；metadata 和小文件接收预算 60 秒，大文件每次输入闲置上限 60 秒。支持未来 expires_at 和不透明 user_id，后者仅归因、不充当 ACL。只在输入长度／结束边界与 PUT 均成功后 complete，不重试不确定上传；官方没有 range／multipart 续传合同，持久化幂等、跨进程续传和真实账号隔离仍不宣称支持。

## 3. 资产与费用语义

- `completed` 的 output 是一个或多个带 type/url 的资产；不是拿到 URL 就已经验证资产可读取。
- 官方说明 URL 是有效期约一小时的 presigned S3 URL，但示例只给 `storage.example.com`，没有可据此冻结的真实 CDN host 白名单。不得猜厂商 bucket。
- 资产下载不得转发 API Bearer、caller Cookie 或其他产品材料；不自动跟随跳转。动态 URL 应限制 HTTPS、公网 DNS/IP、端口、userinfo/fragment，并绑定实际连接地址，避免 DNS 检查后重新解析。
- 资产需有累计字节预算，核对媒体类型及实际载荷；HTML/空正文/截断/无有效媒体结构都不能当生成成功。容器检查与真实视频解码/播放验收分开报告。
- `usage.credits_charged` 是供应商实际确认扣除的 credits；`null` 表示未知，`0` 才是确认无扣款。它可能晚于 completed 或 failed，甚至永久未知，不估算、不换算成 tokens，也不为了等费用重新生成。
- 已保存账号的“查询任务与费用”入口接受单个 UUID，每次点击只发一次 `GET /v1/generations/{id}`。管理身份、敏感授权、精确账号和 revision 前后绑定；30 秒截止、64 KiB 元数据上限，不下载资产、不轮询、不重放生成。页面展示原审计及独立的新查询结果；不回填审计或修改本地次数／cash，真实供应商完成与本地媒体验证分开。2026-11-20 已匿名联网复核官方 Usage 文档的同任务实际扣费合同。
- `usage: null` 表示没有用量报告，不继续查询；有 usage 对象而 credits 未结算时，成功任务在媒体下载验证后、失败任务在身份及原失败分类固定后补查同一个任务。最多 3 次 GET，等待间隔 1/2/4 秒，补查共用原累计元数据预算、最多 8 秒，并在原总 deadline 前预留 250ms。已有金额直接返回；查错、任务身份/原终态不符或截止后保留原结果及未知费用。补查不会采纳新资产、重提 POST 或派生后台任务。
- 正常 failed 结果通过内部 typed receipt 将已验证 UUID、model、type、实际 credits 写入公共请求和管理 probe 的原审计。公共错误不含该 receipt；请求仍失败、普通 Key 沿原流程退款，供应商 credits 不因此变成本地成功收费。
- 2026-11-20 [取消审计子批](../progress/2026-11-20-provider-channel-expansion-luma-cancellation.md)增加请求内快照，完整身份／状态／费用校验后立即记录，取消／超时／资产失败由现有公共和管理审计 owner 保存。只记已观察事实，未知仍为 null；后续无 usage 不擦除已有金额。`mediaVerified` 区分上游 completed 与本地资产完成校验；不据上游状态改成本地成功或处罚取消请求的模型健康。快照不含输入、秘密或资产 URL，不新增后台 I/O；强制进程终止和取消后未知账单仍不保证恢复。
- 本地取消停止原 future/HTTP/轮询，不等于撤销已经受理的远端生成或证明上游无扣费。不自动 DELETE 远端对象，不伪造 exactly-once 或跨进程恢复。

## 4. 本地边界与后续交付

复用正常 JSON passthrough dispatcher 与公共媒体入口，不另起转发服务、不改旧 Web wire。
本地生成、轮询、DNS 和下载共享单一总 deadline；本地限额与官方能力分开说明，不延长 route 的外层预算。
错误保留固定分类，合理 4xx/5xx 保留真实 HTTP status；意外 1xx/2xx/3xx 映射 502，不能让拒绝准入变成公共成功/跳转。不暴露自由文本、Key、输入图像、签名 URL 或私有连接细节。

至少需要覆盖：成功 POST→同 ID poll→资产读取、图片编辑、视频、身份/模型错配、未知/坏状态、HTTP/异步失败、超时/取消、大小预算、坏资产/跳转/私网和不重提生成。
后端普通 SecretPatch/受众 Keep、正常前端模板/手工模型/Key UI及真实管理TCP/本地重建已有独立子批证据。公共媒体pipeline与显式媒体probe子批已闭合Local普通次数扣费/失败取消退款、成功原生事实及正常管理媒体单点；完整供应商费用/cash、其他操作及Server/真实材料验收继续，不能从Local外推。

2026-11-17 的[晚到扣费补查记录](../progress/2026-11-17-provider-channel-expansion-luma-agents-settlement.md)补齐已验证成功媒体的有界 credits 查询，复用公共响应和审计白名单。同日[失败任务费用审计](../progress/2026-11-17-provider-channel-expansion-luma-failure-audit.md)接通正常失败终态的任务事实、晚到费用与原公共/probe 审计；Luma 52 项、共享错误与审计相邻 80 项通过。取消期间已验证事实由上方 11 月 20 日子批补齐；超过截止时间及跨进程追账、USD/cash、真实账号验收及包交付仍未覆盖；下方历史子批的未完成描述只在这些已验范围内更新。

2026-11-05 的 [独立执行链子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-owner.md) 已接 image/image_edit/video 的请求映射、一次提交/同 ID 轮询及资产读取，20 项新增合同与 17 项相邻 Rust 测试通过，生产及代表性关闭编译通过；不是正常模板、公共完整业务、真实 CDN 或整卡完成。multi-keyframes 的 `10s + hdr` 在同一官方 guide 内存在互相矛盾的支持/校验/价格说明，当前明确拒绝并保留待证，不假称组合合同完整。

后续 [后端配置与正常 Key 保存子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-config.md) 已注册独立 preset/profile/family/implementation line，修正 Agents origin 误归 Web，并接原声明/最终合并身份和静态材料保护、原 SecretPatch/完整 Keep 消费者与正常管理 router。新增 14 项测试及相邻回归通过；不开放正常 UI 模板、不新增整卡 offline_ready，也不冒称真实 TCP/SQLite 重建或公共媒体结算已验收。

最新 [公共媒体pipeline子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-luma-agents-pipeline.md) 已修source编辑/公共base64上传、成功UUID/native credits审计与generic文本planner边界；生产Local bootstrap/正常普通Key/public router oneshot/真实loopback上游/SQLite覆盖一次提交/同ID轮询/资产、次数扣费退款、失败/取消与精确身份，85个不同Rust测试（Luma32项，12新增）及生产/代表性关闭编译通过。null/0/0.25只作成功供应商事实，不变Token/USD；未知Token保留既有预占，完整费用/晚到扣额/cash未完成。不是前端TCP/EXE、Server、真实CDN或播放验收；显式操作/费用授权的精确媒体probe及其他操作/Web/P50/P60/P70继续，不增整卡offline_ready。

## 5. 官方来源与证据

最新[显式媒体probe子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-luma-agents-probe.md)已接正常账号卡的逐次费用确认与原管理接口，必须显式操作、型号和单个精确账号；image/image_edit/video 复用原 sender，不继承/保存费用授权，不让默认或自动/命名文本计划生成媒体。默认栈 Luma41项、相邻20项和显式真实main→管理TCP浏览器1项共62个不同Rust测试通过，前端46项/类型/隔离构建、生产及代表性关闭编译通过；3031项Rust和887项UI输入无漂移。成功native白名单审计保留credits null/0/小数，不变Token/USD，普通Key余额不扣；不是供应商不扣费、Server、原生EXE或真实平台证明。其余操作/完整费用/播放/Web/P50/P60/P70保留，模板累计仍16，不增整卡offline_ready。

后续 [真实本地管理/UI子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-local-ui.md) 已完成正常main→真实管理TCP、模板/Key/模型metadata Keep/明确清空及两次同目录同端口runtime重建，后端断言原Key保留及清空后的空Key。供应商Key沿Console revision文件持久化；普通接入Key/token/余额另由真实SQLite保留，无生成时余额4保持，不是媒体settlement。最终2个Rust测试、main隔离构建/来源/静态检查通过；该批未改生产源码、未做真实平台/EXE/后台scheduler验收，不增整卡offline_ready。其当时待补的Local公共pipeline由上方最新子批承接；精确媒体probe/其余操作和集中真实验收继续。

后续 [正常模板与Key UI子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-console.md) 已开放独立lumalabs-api空池模板及实际GlobalDialogs/编辑controller/最终SecretPatch接线；手工模型限定当前三个实现、别名source保留、暂存受众按稳定ID复核、metadata Keep不丢暂存Key。最终382项前端通过（68新增）、1项私有样本跳过，类型/隔离构建/合成浏览器通过，模板累计16。没有真实管理TCP/SQLite或平台调用，不新增整卡offline_ready。精确媒体probe须显式操作/费用授权合同，不能把普通文本默认测试自动转成付费生成；公共pipeline/结算及其余操作继续。

- [Quickstart/auth/polling](https://docs.agents.lumalabs.ai/)
- [FAQ/同 client quota](https://docs.agents.lumalabs.ai/guides/faq/)
- [Create](https://docs.agents.lumalabs.ai/api/resources/generations/methods/create/)、[Get/schema](https://docs.agents.lumalabs.ai/api/resources/generations/methods/get/)
- [Image generation](https://docs.agents.lumalabs.ai/guides/images/generation/)、[Image editing](https://docs.agents.lumalabs.ai/guides/images/editing/)
- [Video generation](https://docs.agents.lumalabs.ai/guides/videos/generation/)、[Video editing](https://docs.agents.lumalabs.ai/guides/videos/editing/)、[Reframing](https://docs.agents.lumalabs.ai/guides/videos/reframing/)
- [Error handling](https://docs.agents.lumalabs.ai/guides/error-handling/)、[Usage/actual credits](https://docs.agents.lumalabs.ai/guides/usage/)、[Files](https://docs.agents.lumalabs.ai/guides/files/)

完整公开资料、抓取时间/status/hash 和正文提取保存在 [本批证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-agents-20261105/public)。
原默认 Python User-Agent 请求返回 403，使用明确标识合同调查用途的 User-Agent 后公开 HTML 返回 200；未登录、未处理或绕过 challenge。
Markdown、llms/sitemap 猜测路径的失败记录保留，不以失败响应作成功 schema 证据。
