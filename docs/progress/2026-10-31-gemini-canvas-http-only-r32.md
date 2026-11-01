# 标准 Gemini Canvas 调用阶段 HTTP-only 改造（2026-10-31，r32）

## 结论

**标准 Canvas 的请求阶段 HTTP-only 已接入生产 owner，并通过新构建后端的真实公共 API 验收。**
不是仅有 Python 原型成功，也没有把普通 Web 或官方 API Key 调用当作 Canvas 成功。
11 项成功业务用例的最终路由审计均为 `direct_http`、精确同一凭据、单次 route attempt；
请求阶段浏览器策略禁用，pool canary 访问为零，没有浏览器/Node worker 后代。

## 范围与实现

标准 Canvas 模板继续使用历史 preset ID `gemini-canvas-browser-relay` 和
`gemini_canvas_web_reverse_compatible` adapter，不合并普通 Web 模板。
调用阶段由 Rust 直接执行 Google Canvas 宿主 `q4uTj` / `RouteLlmRequest`：
一次账号引导 GET、一次生成 POST，不使用普通 Web `StreamGenerate`、官方 API Key、
browser pool、CDP 或页面 `fetch`。首次登录、凭据过期后重新认证仍可使用浏览器。

共同 `canvas_host_generation` owner 已接到新 HTTP transport，保留现有文字、
buffered SSE、图片生成/编辑、TTS、专用转写与多模态输入的上层转换。
标准 Canvas 的路由在旧 endpoint execution override 之前强制 `DirectHttp`，
模板与 payload 标为 HTTP required。旧账号不需要人工编辑数据库或重建账号。

独立 legacy `gemini_canvas_program_web_reverse_compatible` 实验线不在本轮迁移范围。
音乐/视频生成保留明确的 `gemini_canvas_host_media_contract_unavailable`，不隐式回退到 Web。
不因 HTTP transport 改造就宣称这两种生成能力已支持。

## 安全、资源与故障边界

- 仅允许生产 `https://gemini.google.com` origin；loopback fixture 仅在测试构建开放。
- 每次调用使用请求局部 CookieStore，保留导入 Cookie 的域、路径、Secure 和绝对过期时间，
  不读取 profile/CDP extra key，不共享账号 cookie jar，不导出或复制真实登录状态。
- 主 `runtime_state_object_key` 优先；读取失败不退到旧 header。只有没有主 state key 时
  才兼容显式 Cookie header。导入材料读取阶段上限 2 MiB，底层对象存储策略不能被放宽。
- 内层 JSON 上限 24 MiB；嵌套 JSON 各层上限 32 MiB；最终 form 上限 64 MiB。
  限额在序列化/编码过程中执行，不在完整分配后才拒绝。
- 引导 HTML 上限 8 MiB，RPC 响应上限 32 MiB；同时检查声明长度和实际逐 chunk 字节数。
  总 deadline 覆盖编码、材料读取和网络 exchange，发送前检查剩余时间。
- 不跟随重定向、不做 XSRF retry、无跨账号或浏览器 fallback。所有 host 错误经过
  `without_host_replay`，保留分类但禁止自动重放，避免不确定执行后的重复生成。
- RPC parser 只接受正确 `q4uTj` frame 和双层生成正文；拒绝重复响应、内部错误和超量 frame。
  不把外层 HTTP 200 当作生成成功。错误文案不包含原始上游正文或认证材料。
- 浏览器 Cookie 仍有时效；本轮不承诺无限免登录，不新增后台登录续期或持久化 Google Cookie 轮换。

## 验证记录

定向离线检查已通过：

- `cargo test --locked --offline --lib canvas -- --test-threads=1`：766 通过，0 失败，
  3 个既有显式忽略。编译 7m35s，执行 49.40s。
- `cargo test --locked --offline --lib session_cookies -- --test-threads=1`：18/18，
  含 storage-state 域/路径/过期、删除/轮换及不合法输入合同。
- `cargo test --locked --offline --lib object_storage:: -- --test-threads=1`：29/29，
  含调用方收窄读取上限、旧默认读取、已有 S3/本地限额及路径隔离合同。
- 六项新的 loopback host 测试覆盖禁浏览器/旧配置、单次失败、引导/POST redirect、
  非零账号 scope、未知模型及过期 deadline 请求前拒绝、整个 exchange 总时限。
- `cargo fmt --all -- --check`、`git diff --check` 通过。
- effective-line checker tests 31/31，ratchet 通过，governed source 超过 700 为零。
  新 HTTP transport/session/wire/limits 分别为 194/100/129/79 有效行，loopback 测试 241；
  storage-state seed owner 112；旧 host owner 缩至 399。没有新增软上限例外或更改 baseline。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-canvas-http-20261031-r32/`。

开发中已记录的红测证明旧 host owner 依赖 browser runtime；初轮迁移回归发现
旧 preset 断言和过宽的 `private` 单词泄漏断言，已按新合同与独特 sentinel 修正。
资源限额增强的首次编译还发现模块 re-export 可见性及 HashMap 测试写法错误，已修正；
这些失败不计为通过，最终结果以上述回归及以下真实验收记录为准。

## 新构建与真实公共 API 验收

官方快速入口：`tools/build-gateway-devtest.ps1 -Edition api -Jobs 4`，通过。
Windows PowerShell 5.1 首次因继承的 PowerShell 7 模块路径无法找到 `Get-FileHash`，
没有开始 Cargo 构建；改用已安装 `pwsh.exe` 执行同一入口后通过，未修改用户配置。
构建类型为 `devtest`，不是正式 Release 或新的桌面整包。

- 编译产物 SHA-256：`b2490a33daaeb331e4d6781760d6403d316b9397ec1c1c06dd0110e6486efd70`。
- 构建前后源码指纹一致：`8288aba846164b9734326448960601c4775a3d0b81e71527510a93d314af7eba`。
  后续仅补充本进度文档；验收时没有修改生产 Rust 代码。
- 隔离入口 `127.0.0.1:4326`，独立本地路由/数据库/管理及 relay token。
- 运行目录只有新 `gateway.exe`，没有复制 browser worker payload；
  `GATEWAY_REQUEST_TIME_BROWSER_POLICY=disabled`，`43261` 为只记录接触的 pool canary。
- 标准本地模式会把对象存储固定到所选 data root。验收用临时目录引用指向原 objects owner，
  仅按已导入的主 credential key 读取，不复制、导出、覆盖 state；验收结束移除临时引用。
- 通过正常管理 API 和 secret grant 保存隔离配置，不绕过鉴权，不操作用户在线实例。

| 业务用例 | 真实结果 | 客户端耗时 |
| --- | --- | --- |
| OpenAI Chat 非流式 | HTTP 200，新随机验证串精确匹配 | 3,156 ms |
| OpenAI Chat SSE | HTTP 200，精确答案、`stop` 和 `[DONE]`；仍为 buffered SSE | 2,031 ms |
| 图片生成 | HTTP 200，JPEG 1024×1024，464,023 字节，解码及绿色主体检查通过 | 12,406 ms |
| 图片编辑 | HTTP 200，JPEG 1024×1024，462,141 字节；绿三角改红三角，实际视觉复核通过 | 9,594 ms |
| TTS WAV | HTTP 200，24 kHz / mono / 16-bit，99,143 frames，4.131 秒非空音频 | 5,047 ms |
| 图片理解 | 上传本轮编辑图片，精确返回 `red` | 2,860 ms |
| WAV 音频理解 | 本轮 TTS 音频转为准确文本 | 3,687 ms |
| 专用转写 | 真实 multipart `/v1/audio/transcriptions`，准确 JSON transcript | 4,109 ms |
| MP3 输入理解 | 本地转码本轮 TTS 后上传，准确文本 | 5,984 ms |
| PDF 文档理解 | 上传自行构造的 611 字节 PDF，准确读出新随机验证码 | 2,359 ms |
| 视频输入理解 | 上传自行合成的 2 秒 MP4，准确识别先红后蓝 | 2,985 ms |

音乐/视频生成另各一次公共 API 验证，均明确返回 HTTP 503 /
`gemini_canvas_host_media_contract_unavailable`，没有生成、重放或 browser/Web fallback。
此项通过的是“不支持能力的安全拒绝合同”，不是生成能力可用。

最终管理审计有 15 行：11 行业务成功、2 行有意能力拒绝、2 行已保留的初轮失败；
全部结束且 route attempt 为 1、同一精确凭据。成功行的 `selectedCandidate.executionMode`
均为 `direct_http`；失败行未填执行模式，不把空值伪报为已记录 DirectHttp。
12 条安全 HTTP host 完成日志包含一次“RPC 可解析但无完整图片”的失败结果。
独立图片协议诊断不经过公共 Gateway，不计入上述 11 项业务成功。

### 保留的失败与诊断

- 首个文字用例因隔离对象根被本地模式覆盖，返回 `gemini_canvas_http_state_unavailable`；
  未进入 Google exchange。修正隔离目录引用后，以新的排他 guard 验收成功。
- 首个图片请求得到可解析 RPC，但没有可提取的完整图片，按设计返回
  `gemini_canvas_host_media_missing`，未重试。未保存原始 Google 生成正文，无法追溯
  该次缺失的具体原因，不称已经定位上游瞬时原因。
  通过 HTTP 读取当前公开 Google `ponblb` 模块核对 enum，独立单次诊断确认 operation 4
  可返回 STOP/JPEG；随后使用独立 guard 和已核实输入的新公共 API 验收通过。
  没有放宽完整媒体准入，也没有因此增加自动 retry。
- 初次专用转写验收脚本误发 JSON，被公共 multipart 入口在 pipeline 前拒绝（无审计行）；
  修正为真实 multipart 后，用新的 guard 通过，不改生产入口来迁就测试脚本。
- 配置准备过程中，完整 test policy、secret grant 和空 `api_key` 的提交规则由正常 API
  拒绝并修正，仅发生在隔离配置准备阶段，没有上游生成。

所有旧 guard 和失败收据保留；没有为了复核重跑已经消费的 live probe。
最终安全汇总：`final-verification-safe.json`；逐项结果：`*-safe.json`。

## 资源和数据核对

调用结束的实测快照：backend Working Set **34.65 MiB**，private committed **10.79 MiB**，
本进程 Peak Working Set **49.50 MiB**。这是单账号串行短测快照，不是并发、泄漏或
节省比例基准；没有拿它与桌面/Chromium 全树做不等价百分比比较。

backend 仅有 Windows `conhost.exe` 后代，没有 Chromium、Edge、Node 或浏览器 worker；
pool canary 接触 **0**。原对象根 **473 个文件的路径、长度、最后写入时间变化为 0**。
用户 r30 UI/backend、r27 backend 保持原进程，未关闭 Edge。
收尾时仅停止本轮隔离 backend/canary，移除本轮临时 junction；原 state 仍存在，
473 个原对象的元数据变化仍为 0。见 `cleanup-safe.json`。

## 运行现场与交付边界

用户正在运行的 r30 桌面 EXE、r27 后端、Edge、账号及数据保持不动。
本轮真实验收使用新的隔离端口与本地数据库，仅引用程序此前正常导入的 credential object；
不重新导出会话、不覆盖旧包、不自动切换用户 EXE，不自动提交或推送本轮改造。
上轮已推送的 `fb44c708ad6842937990539fd58456b090824a5c` 不包含本轮 HTTP-only 改造。

尚未验证：多账号真实并发、长时间 Cookie 有效性/重新认证、内存压力/泄漏测试、
官方所有模型别名、所有文档格式和音频编码，以及正式发布门禁；不以本轮结果扩大承诺。
