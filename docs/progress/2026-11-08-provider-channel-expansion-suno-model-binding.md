# Suno 基本浏览器任务的模型绑定与远端版本准入

日期：2026-11-08。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`。
本子批承接[单账号音乐测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-suno-probe.md)明确保留的浏览器模型缺口；不是 Suno 整渠道或总计划完成回执。

## 1. 实际缺陷与方案

隔离 Chromium 红测证实，旧 worker 虽然收到 `requestBody.model`，实际 POST 的 `mv` 仍是页面上次选择；错误提示词、Custom、纯伴奏及续写也能被旧实现当成基本生成成功。原 Rust 父进程仅用路由模型标记最终 JSON，不能证明真正发出了哪个模型。

本批继续使用原浏览器、原 Create 点击、原 native response capture、原 clips/轮询与 MP3 finalize，不添加另一套生成器：

- Rust 把配置/alias 解析为已有 native ID，生成仅含基本任务的 worker 输入；原请求不变，未实现参数或显式冲突在调度前拒绝。
- 页面提交前安装单次路由，在精确 origin、`/api/generate/v2-web` 的第一个 POST 上核对基本 Simple/TEXT、非纯伴奏、提示词及无续写/上传等其他操作后，**只改变 `mv`**。保留页面原 auth、challenge/token、交易及会话字段，不自行重造身份。
- 已知的 `prompt` 与 `gpt_description_prompt` 两种简单生成形态都可准入；未知/高级模式不是猜测后继续发送。支持范围是已有两个 native ID 的合成合同，不是当前官网模型目录。
- 响应必须对应已准入提交；worker 返回 `wireModel`，本地、Rust service、远端父进程核对后才进入原 finalize。旧/错模回执不能成功，完整音乐仍需要全部原 MP3 验证。
- 远端使用 `/v1/internal/browser-executor/suno-bound-v1/execute`。旧服务不认识该入口，因此不能靠忽略新 JSON 字段而先生成；不探测性发送到旧入口，也不收到 404 后回退旧路径。Node 与 Rust 两种 service 都沿用已有鉴权，并将新路径限定为 Suno。
- Suno 已加入共享远端媒体 no-replay owner：连接/HTTP/解码/业务错误与不确定结果均禁止转入本地生成或自动重试，redirect 不跟随；未配置远端时原显式本地策略保持。

版本准入保证以同一正常构建/打包的 service 与 worker 为边界；不替操作者任意替换旧脚本或不遵循协议的远端实现担保。`wireModel` 是已发送原生请求的绑定回执，**不是平台最终执行型号的独立证明**。

## 2. 生命周期与安全边界

gate 先于响应 capture 和唯一 Create 点击就绪；未 arm、第二次 POST、继续请求失败均不能报告成功。只移除本调用的 handler，等待活跃 handler 结束；正常终态先清理 gate/capture、关闭 owned browser 或断开本次 CDP、释放原 lease，最后输出。清理失败不伪装成功。

受 Service Worker 控制的页面在点击前拒绝。该检查不证明页面未来永远不会注册 Service Worker，页面路由也不是对站点所有未来网络行为的控制。停止 capture/handler 不撤销平台已接受的生成，不承诺跨请求 exactly-once 或强杀后远端 lease 回收。

native POST 字符串在 JSON 解析前限制为 128 KiB；这不等于 Playwright 获取该字符串之前的分配上限。既有 native 单响应/累计准入、worker stdin/stdio/进程限制保持，不将它们外推为整个 Node service 或浏览器的严格 heap 总量保证。所有新错误均省略原生正文和认证材料。

## 3. 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-model-binding-20261108`。

- `native-red.log`：真实隔离 Chromium 的错误模型与错误任务红测，exit 1；不是 import 失败冒充行为红测。
- `node-final-r2.json/.log`：15 文件、**101 passed / 0 failed / 0 skipped**，包含 19 项真实隔离 Chromium/CDP 测试；另含 Suno CLI/响应/轮询/清理、实际 Node service+worker 子进程及相邻 Udio/Luma 回归。此前中间结果不累加。
- 正常原生 POST 验证改为指定型号，原 challenge/token 和 Authorization 保持；Custom、纯伴奏、续写、未知操作、错误提示词与受控 Service Worker 均零生成 POST；重复提交只有首个请求出站且最终失败。借用浏览器/原页仍可用。
- `rust-red-r2.json/.log`：实际 Rust no-replay 红测 **0 passed / 1 failed**，失败原文 `an uncertain dispatch must not start a local generation: None`。不是编译错误冒充行为红测。
- `rust-green.json`：重新编译后立即复制冻结同次 lib test EXE，SHA256 `107f68f800a825e0699a6ea0034fd9019f9118bae257c0096f54e977ae014871`；该 EXE 是测试程序，不是可交付 Gateway 应用。
- `rust-binding-tests.json/.log`：**7/7** 新增定向通过，包含 alias→native、原请求不变、未知参数/模型零调度、旧入口/redirect 零 legacy dispatch、缺/错绑定回执拒绝，以及连接/HTTP/JSON/缺结果/业务失败不重放。
- `rust-contract.json/.log`：同一冻结 EXE 完整名去重 **354 passed / 0 failed / 5 ignored**，已包含上述 7 项，不与中间结果或历史 252/224 相加。保留普通直连 probe 的 BrowserBacked unsupported/零出站测试，以及绑定后完成音乐的全部 MP3 准入回归。5 项既有跳过分别是四项隔离 Redis lease-release 测试和一项 Luma Agents 显式 browser host；本批不新建 Redis 或消费旧授权补测。
- `package-final.json/.log`：新模块复制、支持文件记录及实际隔离导入的合成包合同 **1/1**；`python-suno.log` 的五项 source contract 通过。修正旧 source contract 中已经过期的 `closeBrowser`、`page.waitForResponse` 等断言，实际生命周期与 native 行为仍由运行测试保护。
- 十个 Rust 文件的定向官方格式、checker **31/31**、最终 `ratchet-final-r3` **5121 文件 / 14 immutable artifacts / governed source >700=0** 通过；checker 输入未变，未重复其单测。文档终态 `diff-final` 及本批范围审计通过。
- `production-on.json/.log`：`cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4`，exit 0。`production-off.json/.log`：从当前 `runtime-defaults` 仅移除 `line-suno-web-reverse-api` 的代表性关闭配置，locked/offline/jobs4 生产 `lib` 编译 exit 0；展开命令见 `production-off-command.json`。关闭编译曾等待独立 Dim 的 Cargo 构建锁，未中止它或另开重复构建。两种配置不等于完整 feature 矩阵。
- 首次 Rust 尝试被并行 Dim OAuth 的 `E0382` 阻挡，未执行行为测试；本批没有修改该独立 owner。后续真实红测与绿测另列，失败未被抹掉。

`node-final-r2` 前后只有独立 Dim 输入变化；本批 Node/脚本输入未变。红测构建期间也有并行 Dim 和本批 Node 输入变化，不能反推为全仓单一快照；最终 `rust-green` 编译、冻结 EXE 的定向/相关回归前后选定输入保持不变。`production-on` 期间独立 Dim 的 catalog/tests、internal-console 及管理路由六项输入发生变化；`production-off` 期间只有 `src/http/routes/internal_console/dim_oauth/tests/catalog.rs` 变化，本批消费源码未变。最终 ratchet 前后选定输入稳定，之后的独立 Dim 开发不倒推为本批已联合编译/回归。仍不将定向通过外推为最新共享全仓单一快照或全功能联合回归。

`scope.json`、`scope-final.diff` 与 `audit.json` 按本批直接 preimage 核对 29 份文本（25 份源码/测试、4 份文档）；UTF-8 无 BOM、尾空白与绝对文件链接通过。25 份源码均不超过 500 有效行，最大为既有 Node service 的 495 行；Rust 与最终冻结测试构建、本批 MJS 与最终 Node 运行输入逐项同 hash，policy/baseline/exceptions 原 bytes 未改。范围中没有 Dim/S06、真实凭据或运行实例修改。

## 4. 未完成与接力终点

本批不放开账号音乐 probe 的 BrowserBacked 前后端限制。下一步需把经过绑定的浏览器执行链接入该正常入口，覆盖原费用确认、精确账号、完整 MP3 与正常 UI→管理 TCP 联合，而不是单凭 native 元数据回执宣布测试通过。

当前官网 wire/model、Clerk/device/challenge/刷新、真实额度和下载权益、完整参数/格式、远端取消与强杀后 lease、Server/独立进程/原生整包、OAuth/MCP 客户端和工具业务仍未验。未读取真实凭据、调用真实 Suno、真实生成或触发收费下载/转换，未使用 Redis/Docker、修改现有实例或出包/提交/推送。

模板仍为 17，无新增整卡 `offline_ready`；P50/P60/P70 与总目标继续。P34 的已消费授权不复用。
