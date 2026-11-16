# Udio 既有图片资产路径安全收尾

日期：2026-11-11（Asia/Shanghai）。状态：既有图片路径修复及定向验证完成，极简feature配置仍有范围外编译限制。本批只处理既有 image 输出路径，不新增音乐/视频下载，不证明当前 Udio cover 产品能力或权益。

## 缺口与最小修复

检查原 owner 发现：worker 结果只要求图片 URL 非空；URL 模式直接发布该值，base64 模式通过共享 HTTP client 无字节上限读取成功/失败正文，随后不解码即包装为图片，非图片 Content-Type 还默认 JPEG。既有总 timeout 不能替代字节预算。

新增 `src/upstream/udio/assets.rs`，复用现有 `public_assets` owner，不新建下载框架或修改共享传输实现：

- URL-only 保留“不下载”语义，仅对全部选中地址做现有 HTTPS、userinfo、fragment、非默认端口及 IP literal 安全准入，再返回原 URL；不查询 DNS、不声称图片内容有效或域名解析已验证。
- `b64_json` 使用无凭据、无代理、禁重定向的专用下载，实际 DNS 结果必须全部为可接受公网地址并固定到连接，保留 TLS 原 hostname 校验。
- 单张 decoded HTTP body 最大 16 MiB，Content-Length 和流式累计均受限；既有输出数最多2张，保留的原始图片 bytes 最多32 MiB。这不是包含 Base64/JSON/解码临时对象的进程总内存承诺。
- 完整 PNG/JPEG 解码，校验 MIME 与实际格式；复用已有8192×8192、64 MiB解码分配及2个decoder owner限制。缺失/错误MIME、HTML、空或截断内容不再默认JPEG成功；不声明GIF/WebP等格式已支持。
- 生成、资产读取及解码等待共享原请求绝对deadline。所有失败继续 `retryable=false / FallbackHint::Abort`，不重提、不换号；资产HTTP错误投影502，不把资产403当生成凭据401/403，不回显签名URL、正文或连接细节。
- 音乐和视频仍走原元数据分支，不进入 downloader。没有新增逐次收费下载、音频转存或绕过平台限制的能力。

`src/upstream/mod.rs` 仅补 Udio feature 的共享资产模块准入。`udio.rs` 有效行201→161；新资产模块63行；共享mod203→204，测试入口164→167，新增测试240行。无超限例外或baseline变更。

## 实际测试与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-image-assets-20261111`。

- `red.log`：旧实现真实 **0 passed / 1 failed**，把 `http://127.0.0.1:随机端口/image` 当成功图片URL返回。该红测只证明URL发布缺陷；旧base64无界读取和伪图片接受由直接源码调用链确认，不冒称本轮另跑过旧base64红测。
- `red-build.json` 编译成功但期间两个范围外 scheduler 测试文件变化；显式允许该已记录差异用于红测，不当最终统一来源。
- `green-build.json`：修复后编译成功，记录输入前后无变化；`green-frozen.json` 固定Cargo实际测试程序，SHA256 `37ce46b2f8fe3ec80279710edb7fcb6fccffaad287d886d3fcf349e1af03743b`。
- `focused.log`：**7 passed / 0 failed**，覆盖URL/base64私网拒绝且零资产读取、URL模式零下载、PNG/JPEG/octet-stream真实解码与匿名头、HTML/空/截断/MIME不符、302/403/500不重提、不安全scheme/userinfo/port/fragment，以及无Content-Length的chunked累计超限。
- `adjacent.log`：**116 passed / 0 failed / 1 ignored**，包括原模型绑定、音乐公共流程、精确probe/健康保护、结果准入、worker/no-replay合同。与本批新增按完整名去重共 **123 passed / 0 failed / 1 ignored**；ignored为上一批已显式验证的网页宿主，本轮未重复运行，不算本轮通过。
- `production.json`、`production-udio-off.json`、`production-udio-sole-asset.json`：默认runtime-defaults、仅关闭Udio、关闭Suno/ChatAIBot/Luma Web/Luma Agents但保留Udio的三项生产lib检查通过，输入前后无变化。第三项证明共享资产模块不依赖另外四个消费者feature，不称为仅Udio配置。
- `production-udio-only.json`：仅启用Udio的极简配置失败，报错位于范围外Gemini Web的`speech/WebUpload/prepare_media`等导入/函数。保留原失败，不接管Gemini/S06、不修改其feature或绕过门禁；不能报完整feature矩阵全绿。
- 本批5个Rust文件官方formatter、checker31项、官方ratchet与范围UTF-8无BOM检查通过；继承报告原bytes恢复。`final-receipt.json`绑定固定harness、123个实际通过测试名、生产检查、来源hash和明确失败边界；`final-diff-check.json`为文档收尾后的差异检查。

## 证据边界

资产合成测试通过原 sender 和真实loopback HTTP；共享 `public_assets::within` 仅在测试编译下把固定假域名映射到本地服务器，不转发真实公网。生产DNS固定/TLS路径复用既有owner并通过非测试编译，不将合成测试称为真实DNS重绑定攻击实测。

本地Node子孙进程超时/取消已有历史验证，本批没有重复添加另一套生命周期机制。没有真实Udio登录/生成/图片或音频下载，未读取真实凭据、创建容器、修改在线实例、出包、提交或推送。

只关闭旧图片下载的目标/字节/解码缺口。当前平台wire/session/模型/额度/图片权益、有效音乐播放、远端lease/取消、Server/独立进程/包及其他渠道/P50/P60/P70继续；不新增整卡`offline_ready`。
