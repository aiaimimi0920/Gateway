# Gemini Web 纯 HTTP 认证 checkpoint

本功能只保存默认 Web HTTP 路径已经持有并由同源响应更新的 Cookie，不读取、导出或修改
浏览器 storageState、localStorage、IndexedDB、persistent profile 或原始导入凭据。
它不是浏览器 checkpoint 的另一入口，不用于重试此前被拒绝的真实会话导出操作。

## 适用范围与默认行为

默认关闭。未配置时保留请求局部 Cookie 行为：每次调用从选中的 seed 开始，成功引导的
`Set-Cookie` 用于同一次调用的后续 POST，不打开 checkpoint 数据库。

显式启用后，默认/modular Web 的文字、流式、媒体理解/转写和原文朗读在网络请求前读取
已提交的 Cookie 状态；成功生成响应完成解析后，保存引导与生成响应中接纳的 Cookie。
下一次请求或新的 Gateway 进程可以恢复该状态。没有新增浏览器依赖或 Node 子进程。
Web 图片生成的另一 legacy HTTP session owner 不在此范围内；Canvas browser checkpoint
仍使用其独立配置与存储格式。没有自动启用现有账号。

## 显式配置

只在 Gateway 进程环境中安全提供以下设置，不将 key 写进路由、日志、发布包或聊天记录：

| 环境变量 | 含义 |
| --- | --- |
| `GEMINI_WEB_HTTP_AUTH_CHECKPOINT_DRIVER` | `disabled`（默认）或 `local`；目前不支持 `s3` |
| `GEMINI_WEB_HTTP_AUTH_CHECKPOINT_DIR` | 受信任本机磁盘上的绝对目录，不支持 UNC、SMB/NFS 或映射网络盘 |
| `GEMINI_WEB_HTTP_AUTH_CHECKPOINT_KEY` | 32 随机字节的标准 Base64，跨重启保留，由安全配置提供 |

启用后 key、目录、driver 非法或缺少稳定 `credential_id`，均在上游请求前拒绝，不静默回退。
停用只需恢复 `disabled`，不会迁移、删除或改写已有数据库。旧 EXE 忽略此独立存储；
回退旧版后不再具备新 checkpoint 的恢复能力，不代表旧 seed 仍被上游接受。

## 身份、加密与生命周期

记录绑定所选 adapter、`credential_id`、runtime-state locator、精确 origin、解析后的完整
Cookie seed，以及存储目录和 key 标识。不同账号、Cookie seed、origin、key 或命名空间
不混用状态。重新使用完全相同的身份和 seed 会重新选中其旧 checkpoint；这不是永久撤销
清单。不会因调用方仅改模型名而复制另一账号的认证。

每次调用重新读取，不把认证 jar 装到共享 HTTP client，也不维护跨请求的内存认证缓存。
AES-256-GCM 使用随机 nonce 和 binding AAD；状态与失效标记均加密，revision 是不含秘密的
随机标识。表字段不明文记录 Cookie、token、账号 ID 或原始源路径。日志只输出固定错误码。

快照保留 Cookie 的有效路径、Domain/host-only 及绝对过期时间，恢复不会重新计时 Max-Age。
session Cookie 的跨进程保留是此显式 opt-in 的语义。已删除或过期 Cookie 不用原 seed 补回。
精确同源、Secure、`__Secure-`、`__Host-`、64 Cookie、单个 8 KiB、store 32 KiB 的限制仍生效。

只有响应通过 HTTP 状态、challenge/auth 分类及业务内容解析后才提交。普通失败不保存
bootstrap 或失败响应的 Cookie；认证失败尝试按所读 revision 写加密 tombstone，后续同一
seed 在网络前被拒绝。challenge 不当作确定的凭据失效。

保存失败或 CAS 冲突只记录固定码，保持已经接受的业务结果成功，不重发 prompt、不切账号。
失效标记写入失败也不修改源 seed。重新准入以磁盘权威状态为准，不做无条件覆盖。
提交前崩溃仍可能丢失最近 Cookie；这不是业务 exactly-once 或跨进程生成互斥。

## 本地存储边界

在显式目录中使用新的 `http-auth-checkpoints-v1.sqlite3`，不修改 Gateway runtime 数据库、
浏览器 checkpoint 数据库、原始凭据或 `.ng` storage version。没有新增依赖。

SQLite `BEGIN IMMEDIATE` 与 revision 条件更新执行 CAS，`synchronous=FULL`、回滚日志，
busy timeout 250 ms，不自动重试写入。单条加密记录最多 128 KiB，最多 1024 个 seed 绑定，
数据库最多 65536 页，打开前还检查数据库主文件不超过 256 MiB。达到限制明确失败，不淘汰
其他账号状态。记录容量与 SQLite 页数不是整个文件系统的磁盘配额，也不包含自动清理。

目录和文件拒绝符号链接/reparse point。POSIX 新目录/文件分别为 0700/0600，Windows 继承
受信任目录 ACL；父目录、ACL、磁盘和 key 管理仍由部署方保护。SQLite 和文件系统操作
没有整个调用的硬时限保证；不得放在网络文件系统上。备份使用 SQLite backup API，或停止
所有使用该目录的实例后复制，不能在写入中只复制主文件。

## 验收限制

本地合成合同与新进程恢复不能证明真实 Google Cookie 旋转、并发账号会话可用性、生产
磁盘故障恢复或所有多模态均成功。该能力不提供 HTTP S3 checkpoint，也不补齐 Canvas
音乐/视频宿主传输。真实账号和人工桌面业务验收仍单独记录，不自动导出或启用现有账号。
