# Flow：真实 HTTP 登录态与图片提交实验

日期：2026-11-01，时间使用 Asia/Shanghai。此记录补充此前仅做资料研究、尚未实际提交的边界。

## 当前结论

本轮已实际完成：普通浏览器导入 → 关闭本次浏览器 → 独立 HTTP 登录态验证 → HTTP 创建项目 → 一次图片生成提交 → HTTP 额度复核。
图片提交被 Google 返回的 `PUBLIC_ERROR_UNUSUAL_ACTIVITY` 拒绝，没有产出图片。
视频没有提交：在收到反滥用拒绝后，停止进一步生成，不能把它记为视频实测失败。

这是独立 HTTP canary 的真实上游结果，不是 Gateway 公共生成 API 的成功验收；生产执行器仍保持 fail-closed。
不能据此断言所有 HTTP-only 方案在技术上不可能，也不能仅凭这个错误码把内部原因唯一归结为 reCAPTCHA。

## 实际发现并修复的问题

原 Flow 导入链错误地要求每条 Cookie 都具有 Secure 标记，因而丢弃普通登录 Cookie。
只允许官方 HTTPS 目的地址并不要求源 Cookie 全部标为 Secure；两者是不同约束。

新增回归测试先复现过滤错误（期望两条、实际只剩一条），再修复：

- `scripts/flow-auth-worker.mjs`：保留普通 Cookie 的原始 Secure 标记；仍拒绝不安全的 `__Secure-`／`__Host-` 前缀材料，保留域、分区、过期及大小限制。
- `src/console/google_web_json/cookies.rs`：移除 Flow 特有的全部 Secure 要求，继续使用原有前缀校验、作用域和大小限制。
- `src/upstream/flow_web/session.rs`：导入、快照和 Set-Cookie 轮换保留普通 Cookie 的 Secure 语义，目的地址仍严格限定官方 HTTPS origin。
- 同时补充 worker、JSON 与 Rust Cookie jar 回归；HTTP、外部域名、前缀违规仍被拒绝。

实验客户端还修正了普通协议与环境问题：Node 22 不会自动采用系统代理；本轮显式使用已有锁定依赖的代理客户端，
确认环境代理与 Windows 代理地址一致；正常 XSRF 更新与生成重试分开处理，不在不确定的生成提交后自动重发。
这些临时代码放在 linshi，不作为新增生产运行依赖。

## 真实证据

| 时间 | 操作 | 结果 |
| --- | --- | --- |
| 17:12 | 第一轮接收登录材料 | 实验进程遇 stdin EOF 退出；没有发出 HTTP 生成，材料仅在内存且已丢弃 |
| 17:22 | 原过滤器下 HTTP 额度预检 | HTTP 401 / RPC code 16；不能当作生成不支持 |
| 17:29 | 对齐代理、等待导入页面加载后的额度预检 | 仍为 HTTP 401；当时只保留 15 条 Cookie，尚无 XSRF at 参数 |
| 17:33:29 | 修正 Cookie 过滤后的额度预检 | HTTP 200，额度 1050，正常 XSRF 参数存在；导入共 21 条 Cookie |
| 17:33:44 | HTTP 创建隔离测试项目 | HTTP 200，项目创建成功 |
| 17:34:31 | 一次图片 HTTP 提交 | 外层 HTTP 200；RPC 内部返回 `PUBLIC_ERROR_UNUSUAL_ACTIVITY`，没有生成 payload／成品 |
| 17:35:27 | 图片拒绝后再次 HTTP 查询额度 | HTTP 200，仍为 1050；未观察到额度减少 |
| 17:37:05 | 结束实验进程 | 正常退出，内存凭据清除；本次专用浏览器进程数为 0 |

正常 Cookie 与 XSRF 随请求发送；没有提供或伪造 reCAPTCHA proof，没有调用 reCAPTCHA token 获取端点。
图片只提交一次，没有切换模型、代理出口、浏览器或其他生成入口重试；未继续提交视频。

创建的远端项目名称为 `Gateway HTTP-only canary 2026-11-01`，保留未删除；没有自动清理用户远端项目。
没有导出 Cookie 值、完整浏览器状态或原始敏感响应；安全 receipt 仅包含阶段、状态、公开错误码及非敏感统计。

证据目录：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-flow-web-20261101/live-canary-20261101-05/http-canary.safe.json`

实验脚本 `canary-runner.mjs`、`canary-capture.mjs`、`canary-http.mjs`、`canary-wire.mjs` 与本地测试位于该目录的父目录。
它们保持凭据在单个进程内，按精确官方 origin 发送，限制响应大小、请求时间和生成次数；不是可直接交付的生产适配器。

## 回归与交付状态

- worker 回归 3/3 通过，包括先红后绿的普通 Cookie 保留用例。
- 实验请求／错误分类／作用域／反滥用停止合同 6/6 通过；不把有效 JSON 或 HTTP 200 当作生成成功。
- Rust Flow 定向单元与公共入口本地合同：11 项通过、1 项真实登录测试按设计忽略；没有把忽略项当作真实生成验收。
- 复用本轮新编译的原生测试程序补跑共享 Google Web JSON 导入回归：8/8 通过，覆盖其他 Google Web 产品及 Flow 的 Cookie 兼容性。
- effective-line checker 自测 31/31、ratchet、文档/源码 UTF-8 无 BOM 与 Git whitespace 检查通过。
- 本轮改动的生产源码均不超过 250 有效行；没有增加既有大文件债务。
- 没有编译新手测 EXE，没有更新当前运行实例、凭据或数据库，没有提交、推送。

图片生成仍受阻，视频未验收；模板不标蓝、不添加未验证的模型和路由。
