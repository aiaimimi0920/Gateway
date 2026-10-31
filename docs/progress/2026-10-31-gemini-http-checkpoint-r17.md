# Gemini 纯 HTTP 本地认证持久化与新包验收（r17）

## 本轮关闭的剩余项

r16 只在一次 HTTP 调用内保留 Cookie 轮换。r17 增加独立、默认关闭的 local HTTP auth
checkpoint：显式启用后，默认/modular Web 的文字、流式、媒体理解/转写和原文朗读能够在
后续请求和新的 Gateway 进程中恢复已提交 Cookie。既接纳成功 bootstrap 的更新，也接纳
成功生成响应的更新；业务内容解析成功后才提交。新增路径不启动浏览器或 Node 子进程。

该 owner 不读取浏览器 storageState、profile、IndexedDB 或原始导入文件，不修改源凭据、
runtime 数据库或 `.ng` storage version。没有自动启用现有账号，没有新增依赖。浏览器
checkpoint 保持独立；Web 图片生成的另一 legacy HTTP session owner 不在本轮接线范围内。

配置与边界见 `docs/gemini-http-auth-checkpoints.md`。必须显式提供 driver、受信任本地目录
及安全管理的 32 字节 key；目前 driver 只有 `disabled` 和 `local`，不支持 HTTP S3。

## 改动与安全审查

- 新增 `http_checkpoint/{mod,config,codec,local}.rs`：绑定 adapter、credential ID、runtime
  locator、origin、完整 Cookie seed 与目录/key 命名空间；AES-256-GCM、随机 nonce 和 AAD。
- 新增 `http_session.rs`：一次调用独占所读 revision 和 jar，不安装共享认证缓存；成功后
  CAS 提交，认证失败尝试写加密 tombstone。普通失败不保存，保存失败不改变已接受的业务
  结果、不重发 prompt、不切账号、不无条件覆盖新 revision。
- 新增 Cookie snapshot：保留实际路径、Domain/host-only 与绝对过期，不在恢复时重置
  Max-Age，不用旧 seed 补回已删除/过期 Cookie；恢复仍检查 Secure/Host 前缀和原有上限。
- 独立 `http-auth-checkpoints-v1.sqlite3` 使用 `BEGIN IMMEDIATE`、FULL synchronous 与
  回滚日志，忙锁 250 ms；128 KiB/记录、最多 1024 绑定、65536 页及主文件 256 MiB 检查。
  Windows 拒绝映射网络 drive，目录/文件拒绝 reparse point；父目录与 ACL 仍须受信任。
- blocking 文件准备工作不占 async worker；正常路径显式关闭 SQLite connection，异常由
  connection/transaction drop 释放和回滚。没有整个磁盘操作的硬时限或业务 exactly-once 保证。
- 错误、Debug 和保存日志只保留固定码，不输出 Cookie、key、路径或底层 SQL 内容。

审查和失败回执解释另存证据目录 `design-review.md`。不将上述存储保护当作真实 Google
并发认证旋转、断电故障恢复或跨 worker 生成互斥已经验收。

## 源码验证与代码规模

最终 Rust 定向检查 **72/72**：Web reverse unit/adjacent 34，六组入口合同 38。其中新增
7 项业务合同覆盖文字/流式/朗读跨进程恢复、失效拦截、新 seed、普通失败、删除不复活、
保存失败不重放、配置拒绝、disabled 兼容和无效响应不提交；另有一个子进程入口计入测试
计数。新增 10 项 unit 覆盖加密绑定、损坏、CAS、命名空间、事务回滚、容量、路径和过期。

旧实现经过实际红测，第二进程返回 `gemini_web_session_invalid`；最初夹具模块路径编译
错误不算业务红测。实现初测还有四个 loopback 恢复失败：合成主 Cookie 沿用了 `__Secure-`
名称但 origin 是 HTTP。夹具改为显式 synthetic primary 后转绿，未放松产品前缀校验。
所有失败日志保留，不把中间失败计为成功。

checker **31/31**、effective-line ratchet、`cargo fmt --all -- --check`、`git diff --check`
均通过。ratchet 3717 文件、14 个分类 immutable runtime assets、受治理源码超过 700 行为 0。
18 个新增/修改 Rust 文件全部不超过 250 有效行，最大为 219；没有改 policy/baseline/exception。

既有文件 before→after：bootstrap 102→101，execution 79→82，exchange 80→84，execution
tests 205→205，speech 150→166，mod 40→42，session_cookies 189→191。记录见
`effective-before.json`、`effective-after.json`。初测与独立 Otter 构建共享机器，按内存使用
jobs 2；后续恢复 jobs 4，不把这些时间当性能基准。没有跑全量回归、付费上游或正式发布审计。

## 新 desktop devtest 包

完整目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-dual-mode-r17/`。

官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4` exit 0，构建和打包实测
733.125 秒。复用现有 devtest 缓存，不复制旧 EXE、不升级依赖、不覆盖既有包，不冒充正式包。

- `buildKind=devtest`、`edition=desktop`、revision
  `fd74db9c374d8caa8309686592a0908c70376773`，包含构建时 dirty worktree。
- 构建前后和打包来源指纹：
  `3e22b228b36468f96a7001d9442e0a4054f800c7b543df7d3adf26cceb76e58a`。
- provenance `builtAt=2026-10-30T19:22:39.6652285+00:00`；devtest profile、jobs 4、
  opt-level 1、incremental 开启、LTO 关闭。默认 4200、`.ng-devtest-4300`。
- 清单 5904 项；完整目录 5906 文件、132794253 字节。官方 artifact smoke 已通过；最终
  聚合另逐项重算 bytes/SHA-256，检查无 `.ng` / `scripts/tests`，并核对七个 worker 源码 hash。

| 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `gateway.exe` | 82647040 | `6d9a13288bc1644dba83b3bf036486940c7478b78bb8801e21f02ae8a77fd0b5` |
| `gateway-ui.exe` | 12862464 | `724056d507f20b9825d180b407de35bef675404286da25a9510e85e19ec28b9a` |

本进度文档在打包完成后新增，不在 immutable 包内；当前工作树整体指纹不因此继续等于
构建快照。未在包内补丁或写测试状态。

## 最终 EXE 与运行验收

1. **公共 API 跨后端进程**：端口 4361，只有合成 provider/credential、全新数据与 key。
   第一个最终包 `gateway.exe` 的 `/v1/chat/completions` 返回 200，结束后启动第二个独立
   后端，同一合成 seed 再返回 200。fixture 强制第二次 bootstrap 携带上次 POST 设置的
   Cookie；两次各只有一次 GET 和一次 POST。SQLite 无合成敏感值明文，测试不落 key。
2. **旧包对照**：同一合同运行于 r16，第一次 API 200，第二进程 bootstrap 因未恢复 Cookie
   返回 401；`http-checkpoint-binary-old-r16.json` 为预期失败，不计入通过。
3. **原生桌面**：4363、foreign cwd、全新 backend/UI 数据，health/ready/UI HTML 均 200；
   worker 路径在包内，UI 存活、响应、有 window handle 和 7 个 WebView2。仅清理本次创建
   的精确进程，WebView2 全退出、端口释放，保护端口 listener 快照不变。未做人眼视觉验收。
4. **包内依赖/原浏览器 owner**：worker main suppress 导入、精确锁定依赖解析通过；包内
   浏览器 checkpoint 的合成新进程恢复、CAS、密文和 tombstone 仍通过。此项不是新的 HTTP
   实现，也不代表生产 S3 或 Google 会话验收。Node SQLite experimental warning 保留。

上述验收全部零 Google 调用、零真实账号使用。没有执行此前被拒绝的真实会话导出/隔离
worker 组合，没有替代入口或已消费媒体 guard 重放。

## 保留现场与未完成边界

在线仍保留 r10 的 UI PID 36304、Gateway PID 46468；没有重启、替换、配置或启用其真实
账号，也没有改在线数据库。新包默认端口同为 4200，占用时拒绝抢占。没有提交、推送或部署。

仍未关闭：Canvas 音乐/视频可信宿主传输；真实 Google 多模态与认证旋转/恢复；生产 S3
条件写回与人工桌面业务验收。新的 HTTP local 能力尚不包含 HTTP S3 或 legacy Web 图片
session owner，也不自动开启真实账号。不能宣称 Gemini 全部多模态已经完成。

证据根：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r17/`。
`validation.json` 汇总实际 hash、测试、构建、原生/API 回执、UTF-8/语法及收尾 4200
health/readiness，最终状态以该回执为准。
