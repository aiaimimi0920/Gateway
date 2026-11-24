# 2026-11-21：Luma 四个剩余工作包续做

本批只处理 L1／L2／L4／L6。沿用暂不运行真实收费生成的边界，不修改现有 4200 实例、账号或凭据，不重做已交付的 L3／L5。

- **L1：同一创作 run 的会话后续审阅。** 按已归档的 `luma_session_await.waiting` 合同接入有明确 `user.gate_verdict` 和 `answer_format` 的单项审阅。绑定原 run、invocation 和内容指纹；分页未读完、同一 run 有冲突候选或会话结束时不允许发送。沿用每批四项素材展示、显式费用确认及 SQLite 单次提交回执，不开放其他 run、client tool、consent 或通用 connector action。分层布局和 Server 持久 MCP 身份不在本切片补造。
- **L2：Web 音视频结果链接。** 本地／远端 worker 结果共用 HTTPS URL 准入，拒绝内网字面地址、携带用户信息等不合格链接；保留合法签名查询字符串原样，错误不允许自动重提。此校验不宣称已验证 DNS、媒体内容、发行方或当前登录 Web 协议。系统原生浏览器通道返回 `Computer Use native pipe is unavailable`，未读取现有登录态，也未尝试重新登录。
- **L4／L6：可重复的 Server 进程验证入口。** [check-postgres.ps1](../../tools/luma/check-postgres.ps1) 的 `-ServerRestart` 启动隔离 PostgreSQL／Redis 和产物不对外暴露的合成 Luma peer。已通过普通 TCP 公共图像入口写入任务 checkpoint，在首次查询挂起时强杀 Gateway 测试进程；第二进程只查同一原任务。回读同一 PostgreSQL 审计行：原请求仍为 `running`、任务事实仍为 `queued` 且媒体未验，合成费用由 0 补至 12.5 credits；上游仅 `POST → GET → GET`。独立验证使用可注入 worker 时钟，不修改任务数据库记录、不等待真实 15 分钟、不使用新打包 EXE。使用路由文档绑定的合成账号与普通共享 Key；真实付费、DB 托管供应商账号的绑定来源及其他渠道不因这条用例关闭。

持续命令见 [Luma 开发工具](../../tools/luma/README.md)。

## 本批验证

- Rust 定向 **84 passed / 4 ignored**，包括 session-only review 和 Web 音视频链接回归。默认忽略的 PostgreSQL CAS／finalizer 用例与 Server restart 用例已按独立入口各运行 **1 passed**；仅子进程入口与正常 Web 浏览器 TCP 宿主不作为独立默认测试运行，后者本批未运行。
- 前端 **20 passed**；TypeScript `--noEmit`、关闭功能编译、Rust 格式、PowerShell 语法、有效行检查器 **31 passed** 及 ratchet 通过。Web 链接有目标红测；MCP 首次红测遇到无效夹具参数，不作为行为红测证据。
- [验证回执与范围源码 SHA-256](../evidence/luma/gateway-luma-four-20261121/verification.json) 不含真实账号、授权或数据库。Server 与 PostgreSQL 测试使用的容器已退出。

## 仍未完成

- **L1：** 其他交互类型未开放。目录描述提到分层 `index/label/bounds`，但当前 `luma_run_await` 输出 schema 没有定义这些字段的编码；不借用 Agents REST 格式。现有 Server 管理授权不是 PostgreSQL 请求审计要求的持久 Key／账号主体，不能填伪造 ID 复用 Local 日志；另需落实持久主体映射并验证。远端真实取消未验。
- **L2：** 下一项需要当前已登录 Web 中的只读会话／board／任务契约；可从恢复的现有浏览器通道读取，或使用已去除 `Cookie`、`Authorization`、签名查询参数和账号信息的记录。不要求重新登录。未开始任何真实生成。
- **L4／L6：** 路由文档账号的 Agents 强杀补账路径已验；不能据此把媒体 credits 当作 USD/cash、承诺 DB 托管账号自动补账，或宣称真实 worker、供应商媒体或费用验收完成。保持付费生成暂停；不重出 L5、不改 4200 实例、不提交或推送。
