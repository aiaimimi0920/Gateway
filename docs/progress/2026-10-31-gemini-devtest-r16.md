# Gemini 请求内 HTTP Cookie 轮换修复与新包验收（r16）

## 本轮关闭的缺口

默认 Gemini Web HTTP 链在 GET `/app` 后丢弃响应 `Set-Cookie`，随后的文字、流式、媒体理解、
转写及默认 Web 原文朗读请求继续使用导入时的旧 Cookie；服务端删除 Cookie 后，原 header
也可能使其重新出现。本轮增加请求局部 `SessionCookies`，让后续 POST 使用成功引导返回的
新 Cookie，而非从共享 HTTP client、其他账号或其他调用继承认证。

此修复只覆盖一次调用内部的引导到执行，不是跨请求或跨重启的纯 HTTP 凭据持久化。
Web 图片生成的另一 legacy HTTP session owner 没有在本轮改写。没有改账号格式、数据库
版本、真实凭据或浏览器 storageState，也没有新增依赖。

## 改动与保护

- 新增 `src/upstream/gemini/web_reverse/session_cookies.rs` 与独立单元测试；在 bootstrap、
  modular exchange 和默认 speech 接线，保留原 `bootstrap_app` public signature。
- 导入的 Cookie header 没有原 scope 信息，绑定所选 origin 的 host-only root；接收响应时
  检查精确 origin、Domain、Path、过期、Secure、`__Secure-` 和 `__Host-` 规则。
  Google root 轮换替换合成 seed scope，避免同名新旧 Cookie 同时发送；删除后不复活。
- 单 Cookie 上限 8 KiB，最多 64 个，store 上限 32 KiB；错误及 Debug 不输出 Cookie 值。
  只在引导成功、完成页面状态分类和解析后接纳 Cookie。不向上传域携带认证。
- 引导正文限制 8 MiB，同时检查声明长度和实际流累计；显式禁止 redirect，拒绝跨源
  target 和含 userinfo 的 URL。状态不落盘，不修改共享 client 的 Cookie jar。

有效行记录在证据目录 `effective-before.json`、`effective-after.json`：bootstrap 66→102，
exchange 78→80，speech 138→150，mod 39→40，既有 execution tests 205→205；新增 Cookie
模块 189、Cookie 单测 125、新入口合同 192、fixture 115。未改 policy、baseline 或 exception。

## 红绿测试与相邻检查

旧实现红测为 4 failed / 1 passed，覆盖文字轮换、朗读轮换、删除后不复活、引导正文上限及
redirect 负例。修复后首次 5/5 转绿，再加入流式、共享 client 两次调用隔离与 chunked 正文
限流。最终定向 Rust **54/54**：unit/adjacent 24、五组入口合同 30，新增 13 项包含其中，
不把首次 green 重复计数。全部采用合成 Cookie 和 loopback HTTP，没有 skip 或 Google 调用。

同时通过 checker **31/31**、effective-line ratchet（3706 文件、14 个分类 immutable
runtime assets、受治理源码超过 700 行为 0）、`cargo fmt --all -- --check`、
`git diff --check`。没有重复全量回归、付费上游测试或正式发布审计。

## 新 desktop devtest 包

完整目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-dual-mode-r16/`。

官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4` exit 0；本次构建和打包
合计 796.828 秒，复用现有 devtest 缓存，不是冷构建基准。没有复制旧 EXE、升级依赖、覆盖
旧包或修改正式发布流程。原有编译告警未隐去。

- `buildKind=devtest`、`edition=desktop`、revision
  `fd74db9c374d8caa8309686592a0908c70376773`，包含构建时 dirty worktree。
- 构建前后与打包来源指纹：
  `2817720481d7259d5122246d95f07a329994fd8a6e90f8934c5e40a207e8485b`。
- provenance `builtAt=2026-10-30T18:25:40.3352544+00:00`；profile devtest、jobs 4、
  opt-level 1、incremental 开启、LTO 关闭。
- 清单有 5903 个待逐项校验文件；连同 manifest/checksums，目录实测 5905 文件、
  132394949 字节。官方 artifact smoke 已通过；总回执另逐项重算 bytes 和 SHA-256，
  检查不含 `.ng` / `scripts/tests`，并核对七个关键包内 worker 与源码一致。

| 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `gateway.exe` | 82253824 | `da6b9bcb5620d4ddae32e949483020c7094d0d36197b7501d8ffa9bb84d1839e` |
| `gateway-ui.exe` | 12862464 | `13419496e347e7f347b5e040f5a08771ac61efd277516d1f3920360bbeef7109` |

本进度文档在打包完成后新增，不在 immutable 包内；不能据此宣称当前全工作树 fingerprint
仍等于构建快照。测试状态只写 linshi，没有在包内补丁或写数据。

## 最终包运行证据

1. 包内 worker 导入成功，main suppress，依赖精确锁定且解析在 payload 内，不启动浏览器。
2. 包内认证 checkpoint 合成 local SQLite roundtrip 通过：新进程恢复、旧版本 CAS 拒绝、
   密文无合成敏感值明文、失效 tombstone 生效及 seed 保留。Node SQLite experimental
   warning 保留；此项不是生产 S3 或真实 Google 认证旋转验收。
3. 4359 隔离原生启动通过：全新 backend/UI 数据、foreign cwd，health/ready/UI HTML
   均 200，worker path 在包内；原生 UI 存活、响应、有 window handle，7 个 WebView2
   进程。仅清理本次精确进程，WebView2 全退出、端口释放，保护端口 listener 快照不变。
   未做人眼视觉审查或完整桌面业务回归。
4. 最终 EXE 的真实 TCP 公共 API 合成合同 **12/12**。六项未鉴权分支均 401；Web 图片
   数量和 Required 策略拒绝为预期 400；Canvas 音乐/视频明确 503，均零上游。
   默认 `/v1/audio/speech` 返回 200、`audio/wav` 和精确 48 字节合成 WAV。
   **此 fixture 的 `/app` 强制轮换 Cookie，后续 POST 不带新值即拒绝**；回执明确
   `bootstrapCookieRotationRequiredByFixture=true`，证明修复进入新 EXE，而非仅源码测试。
   合成 WAV 不代表真实 Google 语音生成。测试后 4360 已释放，未启动浏览器。

## 未关闭的内容与现场保护

- **未实现**：纯 HTTP 跨请求、跨重启认证写回。本次 request-local Cookie 不是此能力。
- **合同阻塞**：Canvas 音乐/视频尚无可信可用宿主传输。本轮经 Exa 联网取得 Google 官方
  Canvas 与 Lyria API 正文；官方 API 音乐文档不证明 Canvas preview 宿主声明了该模型，
  也不授权改用 API Key 回退。公开核对记录为 `public-reference-check.md`。
- **真实业务未验收**：r16 上 Google Web 图片恢复、媒体输入/输出、原文朗读、Web/Canvas
  专用转写和 MP3 输入，以及真实认证旋转/恢复、生产 S3 条件写回。
- **人工未验收**：桌面交互和用户实际场景回归。

没有重试此前被工具拒绝的真实 storageState 导出/隔离 worker 组合，没有换入口绕过，
没有重放已消费的媒体生成 guard。保护现有 r10 的 4200 实例，不修改在线账号、凭据、
配置、数据库，不提交、推送或部署新包。默认端口仍为 4200、数据目录 `.ng-devtest-4300`；
旧实例占用时明确拒绝抢占，不自动换端口或停止旧服务。

证据根：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r16/`。
最终聚合以 `validation.json` 为准，包含包完整性、回执、编码/语法检查和收尾 4200
health/readiness。新包可交手测，但不能宣称 Gemini 全部多模态已经完成。
