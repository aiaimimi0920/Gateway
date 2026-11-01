# AI Studio Web HTTP 生产接入与真实验收（r36）

## 验收范围

目标是 `aistudio-web` 独立模板与 AI Studio Playground，不是 Gemini Canvas、旧 Build app、
CodeAssistant 或官方 Gemini API Key。调用阶段不启动浏览器，允许本地原生 JS proof 引擎。
本次隔离实例不替换用户正在运行的 EXE，不改其账号、数据库或端口。

## 当前结果

- **passed**：生产 Node HTTP worker 与仓库内构建的原生 helper 完成真实调用，
  一次 `GenerateContent` 返回 200、`STOP`、19 字节随机答案精确匹配；fresh proof 实际生成。
  所有 bootstrap／catalog／preferences／logging／challenge／interpreter 阶段均为 200。
- **passed**：Gateway 公共 `/v1/chat/completions` 非流式与 buffered SSE 各一次真实调用，
  均为 HTTP 200、19 字节随机答案精确匹配、`finish_reason=stop`，路由证明指向
  `aistudio-web-reverse`；不是 fixture 或官方 API 回退。两次均正常 drain。
- **passed**：Rust AI Studio 定向测试 39/39；新增防重放回归先红后绿。
- **passed**：Node 定向测试 19/19，模板测试 1/1，前端 typecheck，manifest 校验 44 项，
  checker tests 31/31、ratchet（governed source >700 = 0）、Rust formatter 与 `git diff --check`。
- **passed**：后端 `devtest` 构建及本地对象目录选择政策单测 2/2；这不是桌面发行整包。

## 公共 API 真实验收

2026-11-01 01:40—01:41（Asia/Shanghai）使用正常重新登录后导入的 Cookie 对象。
每个独立实例各创建一个临时正常 access key，通过公共 API 发送一个随机校验提示。
路由、SQLite 和状态目录独立；对象根显式指向现有正常凭据 owner，只读取精确对象键，
不复制 Cookie。因此不能称为对象存储完全隔离。

| 项目 | r3 非流式 | r4 buffered SSE |
| --- | --- | --- |
| 公共端点 | `/v1/chat/completions` | `/v1/chat/completions` |
| 模型 | `gemini-3.5-flash-lite` | `gemini-3.5-flash-lite` |
| HTTP／Content-Type | 200／`application/json` | 200／`text/event-stream` |
| 随机文本精确匹配 | 19 字节 | 19 字节 |
| 完成信号 | `stop` | `stop`、`[DONE]`、2 个 SSE JSON 帧 |
| 公共请求耗时 | 6,641 ms | 7,656 ms |
| 路由证明／请求 ID | `v1`、provider line 正确、ID 匹配 | `v1`、provider line 正确、ID 匹配 |
| 进程树采样 | 6 次，未观察到浏览器 | 6 次，观察到 Node／原生 proof helper，未观察到浏览器 |
| 采样到的进程树工作集峰值 | 112,963,584 bytes | 218,275,840 bytes |
| 正常 drain | 通过，exit 0 | 通过，exit 0 |

工作集是离散采样值，不是严格内存峰值或并发容量承诺；r3 没有采样到短生命周期 helper。
生产调用入口本身不导入浏览器模块、不设浏览器回退，与 r4 进程证据相互印证。
两次公共 API 成功闭环后停止真实生成，没有重放已消耗的 run guard。

此前失败保留原始记录：r1 路由写入了错误的本地目录，HTTP 400，审计为 0 次上游尝试；
r2 已进入新 HTTP worker，但旧 Cookie 的 bootstrap 返回 302 到 Google 登录页，未到生成阶段。
随后以无自动化连接的普通 Chrome 专用窗口重新登录、关闭，再导入必要 Cookie；
没有绕过登录／challenge，也没有把认证失败归结为平台不支持 HTTP。

## 生产改动

1. 维持 `aistudio-web`、`gemini-web`、`gemini-canvas` 三个独立模板，AI Studio base URL
   修正为 `https://aistudio.google.com`，Playground 页面默认值为 `/prompts/new_chat`。
   默认模型改为本账号目录实际存在且已真实成功的 `gemini-3.5-flash-lite`。
2. 生产文本入口改走 `scripts/aistudio-http-worker.mjs`；移除该入口的旧 Build 执行、
   deterministic synthetic success 和浏览器回退。显式 fixture 分支继续保留。
3. 请求局部 Cookie/SAPISID 认证、固定 HTTPS 目标、解码后大小限制、解释器摘要校验、
   fresh WAA proof、完整终止帧和文本增量追加分别由独立小模块负责。
4. 原生 helper 经私有 stdin/stdout 管道调用，SpiderMonkey 在 helper 进程内，不是浏览器，
   也不是直接嵌入 Gateway Rust 地址空间。Gateway 复用现有通用进程树监管的取消／回收能力。
5. 错误设置 `retryable=false` 与 `FallbackHint::Abort`，防止响应丢失后由外层重试／改道再次生成。
   旧 endpoint browser override 不得改变 AI Studio 的 DirectHttp 执行模式。
6. 未实现的图像、语音和 embeddings 生产路径明确拒绝，不退回官方 API 来冒充支持；
   line manifest 现声明 `direct_http`、`conversation/models` 与 session auth。
7. 本地启动仅在绝对 `GATEWAY_DATA_DIR`、`local` driver 与绝对对象目录均显式设置时，
   保留可信宿主选择的对象目录；其余 routes/state 默认值仍受选定 data root 隔离。
   两项独立政策单测先红后绿，主程序已重编验证。
8. bootstrap 指向 Google 登录的重定向固定分类为 `aistudio_http_login_required`，
   不跟随、不记录 Location 或响应正文；Rust 当前对外包装状态仍是 503。

## 安全证据

证据根：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-aistudio-http-20261101-r36`

- `production-worker-r1.safe.json`：生产 JS 模块与新 helper 的真实成功证据。
- `public-api-r3.safe.json`、`public-api-r4.safe.json`：公共 API 成功、源码与二进制摘要、
  安全进程树元数据及 drain 结果；`public-api-r1.safe.json`、`public-api-r2.safe.json` 保留失败历史。
- 此前对照在 r34／r35：无 proof 的生成 403；fresh SpiderMonkey proof 的生成 200。
  只证明当前账号对照结果，不推断所有 Google 账号永久强制相同机制。
- 验收仅使用正常导入的精确 Cookie 对象键；登录失效后经用户正常登录重新导入必要 Cookie。
  没有导出完整浏览器状态、复制 Cookie 到证据目录，或把认证头、proof、网页正文写入报告。

## 构建与测试说明

第一轮默认 `devtest` 定向测试 38/38，耗时 31m53s。直接 Cargo 受仓库
`build.incremental=false` 影响；一次重复的非增量单用例编译被精确停止，不算通过。
后续定向测试显式使用 `CARGO_INCREMENTAL=1` 和仅当前 crate 的测试优化覆盖：

```powershell
cargo test --locked --offline --profile devtest --jobs 2 --config 'profile.devtest.package.gateway.opt-level=0' --lib aistudio -- --test-threads=1
```

防重放用例首先在 `assert!(!error.retryable)` 处实际失败；修改错误 owner 后，39 项全绿。
最终后端验收构建不带该测试优化覆盖，使用原 `devtest` 优化级别 1、锁定离线依赖和 2 jobs。
完整后端构建通过，耗时 12m43s；本地启动修复后的热增量构建通过，耗时 24.21s。
这不是同一规模改动的性能对照，不据此承诺提速倍数。

验收后端：`target/devtest/gateway.exe`，SHA256：

```text
20878033695d9ea350b5ea82346529aff1c0cc77d9321e21d1a4a9fef91b0739
```

原生 helper SHA256：

```text
fceefb8eff10e86d0f7102bfc7d0a1b686110404d2ea383b4a98965b0be171fe
```

这不是已完成的桌面手测包，更不是正式 Release；没有提交或推送。

## 未作为完成项交付的边界

- SSE 目前是完整响应后的 buffered SSE，不是真实逐 token 流式透传。
- 当前 HTTP codec 支持文本；附件、原生函数调用及其他未实现选项拒绝，不能声称全多模态完成。
- AI Studio 专用管理 UI 导入交互、helper 自动定位／发行打包和长期会话刷新未完成整体验收。
- 尚未交付新的桌面 EXE 整包，也没有更新用户当前运行实例。

实现与运行依赖见 [AI Studio Web HTTP](../aistudio-web-http.md)。
