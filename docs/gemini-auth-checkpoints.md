# Gemini 浏览器认证 checkpoint

本功能保存的是浏览器认证状态，不是生成任务、媒体结果或业务请求的重放记录。
默认关闭，不会自动导出当前已有账号或改写导入 JSON。启用后，只适用于由浏览器池拥有的
`storage_state_file` context；外部 CDP 和 persistent profile 仍由原生浏览器拥有。

纯 HTTP Web 另有请求局部 Cookie owner，默认不跨请求保存，也不读取这里的浏览器状态。
可独立显式启用[纯 HTTP 认证 checkpoint](gemini-http-auth-checkpoints.md)，目前只支持新的
加密 local 存储，不改导入凭据、不与浏览器数据库混用、不自动启用现有账号。请求内轮换、
HTTP 跨进程恢复和浏览器 storageState 恢复是三个不同的验收范围。

## 状态与生命周期

导入 JSON 仍是 revisioned seed。checkpoint 采用单独存储，绑定 seed 的完整 identity、
内容 SHA-256、checkpoint 存储命名空间与加密 key 标识。先读取当前 seed，再读取匹配的
checkpoint；重新导入不同内容、删除源、切换账号或存储配置，都不会复活旧认证状态。

每次空闲准入重新检查 checkpoint 版本；外部 worker 提交了新版本时重建旧 context。
版本未变则保留 live Cookie，不用导入文件或 checkpoint 反复覆盖浏览器的旋转状态。
执行中的 context 不会被外部更新强拆。

只有成功操作且仍有认证 Cookie 时才保存 `storageState({ indexedDB: true })`，包括 Cookie、
localStorage 和 IndexedDB。保存期间保留 busy lease。普通失败不保存；401/403（包括代理
fetch 返回的认证错误）尝试按当前版本写入加密失效标记，阻止下次静默恢复同一失败状态。
新 seed 可以重新引导。失效写入也受 CAS 约束，旧 worker 不能撤销较新 worker 的状态。

保存错误、版本冲突或捕获超时只输出固定错误码并回收本次 context。已经成功的业务结果
保持成功，不重发 prompt，不切账号重试。远端 PUT 超时可能已提交；不自动重发，下一次
准入重新读取权威对象。进程在操作后、checkpoint 提交前崩溃，仍可能丢失最近旋转状态。

**这不是业务 exactly-once，也不是跨 worker 的生成互斥。** 多个 worker 仍可能同时调用
同一账号；CAS 仅防止旧认证状态静默覆盖新版本。不同浏览器并发旋转上游 Cookie 的行为
仍需真实账号验收，不能由本地 CAS 测试证明。

## 显式配置

| 环境变量 | 用途 |
| --- | --- |
| `GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_DRIVER` | `disabled`（默认）、`local` 或 `s3` |
| `GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_KEY` | 32 随机字节的标准 Base64；必须由安全配置提供并跨重启保留 |
| `GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_DIR` | local 模式的显式绝对目录，不能是直接 UNC 路径 |
| `GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_PREFIX` | s3 模式的独立对象前缀，限字母、数字、`_`、`-` 与非空 `/` 分段 |

配置只在 worker 启动环境中设置，不放入版本化配置、日志、发布包或聊天记录。
缺失/非法 key 或不完整配置直接拒绝准入，不回退到不持久化模式。改 key 会创建独立命名空间，
不会自动迁移或删除旧记录；如需保留旧 checkpoint，必须保管旧 key。

### local

使用独立 `auth-checkpoints-v1.sqlite3`，不修改 Gateway 原有 runtime 数据库，也不升级 `.ng`
存储版本。依赖已声明的 Node.js >=22.22.0 内置 `node:sqlite`，不增加 npm 依赖。
当前 Node 22 会输出 SQLite experimental warning；功能关闭时不加载该内置模块。

目录必须位于受信任的本机磁盘，由运行账号管理；不支持把 SQLite 放在 SMB/NFS 或映射的
网络文件系统上做跨主机锁。直接目录与目标文件拒绝符号链接；父路径和 Windows ACL 必须
由部署方保护。POSIX 新目录为 0700、数据库为 0600；Windows 继承目录 ACL。数据库和
事务日志只写加密 payload，不把 Cookie、token 或源路径作为明文表字段。

SQLite `BEGIN IMMEDIATE` 与 revision 条件更新构成 CAS，`synchronous=FULL`，使用回滚日志，
忙锁最多等 250 ms，不做无限重试。单记录上限 16 MiB，最多 1024 个 seed/namespace 绑定，
数据库配置 65536 页上限（新建库默认 4 KiB 页，约 256 MiB）。达到容量会明确失败，不淘汰
仍有效的认证状态；本轮不包含自动保留期清理。备份应使用 SQLite backup API，或停止所有
使用该目录的 worker 后再复制；不要在写入中仅复制数据库主文件。

### s3

复用现有 `AI_GATEWAY_OBJECT_STORAGE_*` / 兼容 `OBJECT_STORAGE_*` 的 endpoint、bucket、
region、path-style 和访问凭据。checkpoint driver 单独选择，不改变原 seed 的存储 driver。
要求 HTTPS；仅 loopback 测试端点允许 HTTP。拒绝 URL 内置用户名、密码、query 和 fragment。

对象位于 `<prefix>/<binding SHA-256>.checkpoint`，AES-256-GCM 加密并绑定 AAD，不写回原始
seed key，也不修改 legacy 本地镜像。首次创建必须 `If-None-Match: *`，更新必须 `If-Match`
先前 ETag；缺 ETag、409/412、远端错误或损坏记录都不退回旧镜像或无条件 PUT。
只适用于真正原子支持这些条件请求的 S3 实现；不以普通 PUT 成功冒充 CAS 已支持。

每次请求创建并销毁专用 S3 client，SDK `maxAttempts=1`，不跟随 region redirect。
远端操作和浏览器捕获各限 5 秒；实际流累计与声明长度均限制为 16 MiB。超时中断 SDK 请求，
捕获超时后的迟到结果不能再写入。SQLite 同步磁盘操作没有异步总时限保证。

16 MiB 是持久化记录和读取流的限制，不是浏览器自身的内存配额；Playwright 的
`storageState()` 仍会先在内存中组装快照，特别大的 IndexedDB 需要部署侧资源限制。

共享 checkpoint 的 worker 必须拥有相同 seed identity 与 checkpoint namespace。
identity 包含解析后的源定位；不同本地根/镜像定位不会自动合并为同一账号状态。
故意恢复某个旧 seed 的完全相同内容，会重新选择它自己的旧 checkpoint；本功能不是
永久凭据撤销清单，也不会让已被上游撤销的 Cookie 重新有效。

## 安全与验收边界

加密不能代替目录权限、S3 IAM 或 key 管理；拥有 key 与密文者仍可解密。没有新增匿名管理
接口、凭据导出 API 或日志正文。原始导入 JSON 的既有保护策略不由本功能替代。

本地合同覆盖合成 Cookie、独立子进程恢复、并发 CAS、事务中断、失效标记、损坏/超限、
超时与 loopback S3。没有自动启用真实账号，没有真实 Google 调用/认证恢复或生产 S3 验收。
不得将这些测试当作此前被工具拒绝的真实导出/隔离 worker 操作的替代入口。
