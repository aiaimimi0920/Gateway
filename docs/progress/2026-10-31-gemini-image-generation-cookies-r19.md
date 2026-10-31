# Gemini 图片生成响应 Cookie 传递修复与 r19 手测包

## 本轮完成范围

补齐 r18 留下的一项真实代码缺口：Web 图片生成响应的 `Set-Cookie` 原来没有传给同一请求的
只读取回。现在模板和无模板发送都创建请求局部 `SessionCookies`，在有界、成功且非 challenge /
认证失败的响应后更新，再按所有权传给 bootstrap/history，保留域、路径和删除语义。

没有把 jar 展平成原始 Cookie header 后重新初始化，没有安装共享 HTTP jar、持久写凭据或浏览器
状态。既有一次生成、禁止 redirect、固定 turn/candidate、最多六次只读轮询和 30 秒共享 deadline
不变；失败和取消随请求释放状态，无 detached poller。文字与 legacy Canvas 仍返回无 jar 的结果。

**这不是全部 Gemini 多模态完成。** Canvas 音乐和视频没有在本轮取得成功合同，也没有被替换成
普通 Web 或官方 API Key 调用。图片 downloader 的 Cookie 连续性、legacy preflight 与图片跨请求 /
跨进程 HTTP checkpoint 不在本轮覆盖范围。

## 实际红绿验证

全部数据来自 loopback 合成模板和会话，零 Google 请求、零真实凭据，无浏览器启动或真实 guard 重放。

- **旧 r18 EXE 红测**：一次 StreamGenerate 被接受并轮换 Cookie，随后 `/app` 仍发送旧 Cookie，
  上游 fixture 返回 401，公共 `/v1/images/generations` 返回 503 /
  `gemini_web_image_read_http_failed`。只有一次生成，没有重新提交。
- **新 r19 EXE 绿测**：同一公共入口 HTTP 200，返回预期合成图片 URL。严格计数为一次生成 POST、
  一次 bootstrap GET、两次 history POST；生成、bootstrap 和 pending history 三处轮换均生效。
  generation 的 history-only Cookie 未发给 `/app`，foreign-domain Cookie 被拒绝，源状态 hash 不变。
  这是 URL 取回合同，不证明真实 Google 生成或图片解码成功。
- **Rust 57/57**：图片单次提交/取回 16、共享 Web owner 34、图片绑定/解析 7。新增三项回归分别
  验证生成响应的根 Cookie 轮换、删除及路径/域隔离，每项覆盖模板和无模板发送，并在同一 client
  上执行两次，证明下一请求仍从原 seed 开始。测试编译 14m56s，16 项执行 22.90s。
- **其他检查**：formatter、diff check、checker 31/31、ratchet 通过；3719 文件，14 个 immutable
  runtime assets，受治理源码超过 700 行为 0。十二个相关 Rust 文件均为 UTF-8 无 BOM。

主要有效行 before→after：stream owner 160→181、template sender 85→92、heavy sender
467→481、image caller 54→60、recovery 55→59；新测试 167。bootstrap 254→254，其余测试均不超过
250。heavy sender 仍只负责一次发送与响应处理，没有修改 policy、baseline 或 exceptions。

## 新 desktop devtest 包

目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-dual-mode-r19/`。

官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4` exit 0，构建打包实测
681.394 秒。buildKind 为 devtest，不是正式发布；来源 revision
`fd74db9c374d8caa8309686592a0908c70376773`，包含构建时工作树，来源指纹
`794b76a62a6a22bb65095fd7ad29da37d766f004c032e488e966656906be271d`。
provenance builtAt 为 `2026-10-30T21:40:23.5219919+00:00`。

清单 5904 项逐项重算 bytes/SHA-256 通过；完整目录 5906 文件、132856205 字节，无 `.ng`、
无 `scripts/tests`。锁定 worker 依赖导入、官方 artifact smoke 和上述新 EXE 公共 API 检查通过。

| EXE | 字节数 | SHA-256 |
| --- | ---: | --- |
| gateway.exe | 82708992 | `0969ace1457d7a24125531ff9bb0882befb7858b92314e515e2f9274b1d1b3c6` |
| gateway-ui.exe | 12862464 | `8ea620a043d935149c2abe7a23897ece208a3122cdcc90fab3bdfa82000cfaf3` |

本进度文档在打包后新增，不在 immutable 包内，不声称此刻整个工作树仍等于打包来源指纹。
未重复原生 UI 启动/人工业务验收，未跑全量回归、付费上游、正式发布安全审计。

## Canvas 剩余项与用户配合

- 音乐：尚未取得当前账号允许的音乐模型/宿主传输合同，生产代码仍明确拒绝，不猜模型字段或
  放宽 native fetch。
- 视频：此前宿主操作失败没有产生 operation 或视频。成功响应、轮询/取回合同和生产 host 视频
  执行器仍缺，不能把历史空正文 401 单独当作未登录。本轮没有重新生成视频。
- 当前 UI 工具 inventory 返回 native pipe 和 browser access host 不可用。因此没有要求盲目重登，
  没有改用另一技术操作 UI 或导出登录状态。已请求用户只查看已登录 Canvas 应用的音乐/视频按钮、
  错误或截图，先不要点击生成。
- 真实 Google 多模态/认证轮换、HTTP S3 实现及生产 S3 条件写回、人工桌面业务仍未验收。

在线仍保留 r10 UI PID 36304、后端 PID 46468；收尾 healthz/readyz 均 200，保护 listener 与开工一致。
仅停止本轮合成测试后端 PID 10296/43888，4365/4367 均释放；无本轮 Cargo/rustc 残留。
没有提交、推送、替换在线版本、修改原账号或数据库。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r19/`。
以 `validation.json`、`binary-old-r18.json`、`binary-r19.json`、Rust 日志和 `package.json` 为准。
