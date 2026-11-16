# 同步与备份实现边界

## 已确认的产品范围（2026-11-15）

- 导出、导入使用同一种密码加密备份文件。
- 包含服务商、账号凭据与其引用的登录材料、模型映射、权益组、通用设置。
- 包含管理密钥、AI 调用密钥和余额；恢复时替换目标对应数据。
- 不包含运行日志和普通调用历史。未决扣费、FreeBuff 原 claim 等防止重复消费的事实不能按“历史”简单剔除。
- WebDAV、S3、GitHub 均为手动上传和下载恢复，不自动合并或后台覆盖。
- 实际导入必须先验证、展示覆盖范围并确认，保留恢复前副本。

## 当前代码状态

`src/console/backup` 已接入受管理身份及敏感操作授权保护的
`POST /v1/internal/gateway/console/backup`，设置页不再使用不可用占位。
支持保存同步目的地、导出、校验预览、确认暂存恢复、取消待恢复及三种手动远端传输。

- 固定版本 NGBK 封装，Argon2id（64 MiB、3 次、1 lane）派生密钥，AES-256-GCM 认证加密。
- 每次随机 salt 和 nonce；头部受认证；错误密码、损坏和未知版本拒绝解密。
- 明文最大 32 MiB；远端下载有总时限和有界读取；密码与远端凭据类型不实现 Debug。
- WebDAV 使用 HTTPS、独立用户名密码和 ETag 条件写入，不跟随重定向。
- S3 使用显式目的地和凭据、条件写入，不使用推理对象存储的环境凭据，不自动重试上传。
- GitHub 使用 Contents API 条件更新；下载取得 blob SHA 后读取固定 blob，防止分支并发变化。
- 三种传输层仅接收版本化加密封装。下载后的认证、解密和业务内容校验仍由备份 owner 完成，封装头检查不代替认证。

同步目的地凭据复用本地 seal 加密保存，不写入浏览器 localStorage。
管理密钥导出生成新 revision；恢复后的管理密钥优先于目标旧环境密钥。
导出包含 SQLite 调用密钥及余额数据和对应 seal；普通已结束请求历史及凭据测试结果从副本移除并 VACUUM。
活动 runtime SQLite 和其他 SQLite 登录材料均使用数据库在线快照，不复制其活动 WAL/journal。

## 使用与恢复契约

1. 导出时输入并确认至少 8 字符的备份密码，下载 `.ngbk`。配置和快照操作持有现有 publication / route writer 锁；非数据库文件若在读取时变化则拒绝。建议停止账号授权、浏览器会话更新及其他配置编辑后导出；不承诺独立文件 writer 之间的全局事务时刻。
2. 导入选择文件和密码，先校验格式、路径、seal、SQLite、路由恢复状态及管理密钥，再显示备份时间和数量。点击“确认覆盖并准备恢复”才会创建待恢复任务；当前运行数据不变。
3. 完全退出使用该数据根的桌面端和后端再启动。共享 `gateway-local-data::prepare` 在发放 usage lease 前取得独占锁并恢复；仅关闭网页或仅重启后端不一定足够。可以在退出前取消待恢复任务。
4. 恢复替换 `local`、`objects`、`credentials`、`gateway.json`；原数据保留在数据根的 `backups/restore-*`。中断事务先回滚原数据，并明确要求再次启动。不改变 storage version，不要求用户执行 SQL。
5. WebDAV/S3/GitHub 初次上传使用 create-only；覆盖已有备份须先下载取得远端 revision。覆盖时再次检查 ETag/blob SHA，版本变化则拒绝。下载不会自动恢复，可先保存加密文件。

## 限制与运行边界

- 当前支持受管理的本地 SQLite 数据布局；server/PostgreSQL、外置对象目录或 S3 对象存储明确拒绝，不能用缺少账号材料的备份冒充成功。同步目的地为 S3 与运行对象存储为 S3 是两回事。
- 备份明文封装上限 32 MiB，文件最多 4096 个；内部文件经 base64 封装，实际可容纳的原始文件总量小于 32 MiB。超限明确失败，不截断。
- 不包含进程环境、外部服务、操作系统开机启动注册、其他数据根或普通运行日志。
- 恢复保留的旧副本和待恢复目录含明文敏感材料。Unix 工作目录使用 `0700`；Windows 沿用数据根继承 ACL，应只允许当前用户访问。不要发布或上传整个数据目录。
- 已覆盖进程中断状态的恢复；未进行断电、磁盘控制器缓存丢失级验收，不宣称断电持久性。
- 新前端需要配套新后端；旧后端没有该接口时显示不可用提示。此次开发不替换或重启运行中的 4200/4400，不修改其真实数据库。
- 真实 WebDAV/S3/GitHub 账号联通及云端权限尚需用户提供独立测试目的地后验收；合成测试不等于真实云端验收。

## 定向验证

```powershell
cargo test --locked -p gateway-local-data restore::tests -- --test-threads=1
cargo test --locked --config 'profile.test.package.gateway.debug=0' --test console_backup_contract -j 2 -- --test-threads=1
npm run typecheck --prefix apps/desktop
npm run test --prefix apps/desktop -- --run src/features/settings/GatewayBackupSettings.test.tsx src/features/settings/SettingsImplementation.test.tsx src/features/settings/SettingsParity.test.tsx
npm run test:effective-lines --prefix scripts
npm run check:effective-lines --prefix scripts
```

测试前将 `TEMP`/`TMP` 指向 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-backup-tests`。
测试使用合成密码、合成凭据和 loopback 模拟服务，不读取 4200/4400 的账号数据，不上传到真实云存储。
仅对 Gateway 测试包关闭调试符号以避免 Windows 链接超大 rlib；不改变发布优化或依赖。

2026-11-15 实测：完整往返集成测试通过（包括受保护 HTTP 状态/导出、错误密码拒绝、
非默认配额余额、调用密钥、管理员密钥优先级、同步凭据 seal、WAL 登录材料、
历史剔除、独占恢复及旧副本）；恢复事务 5 项、设置页定向测试 7 项及类型检查通过。
代码行数检查器测试 31 项和 ratchet、`git diff --check` 通过。

另启空白隔离预览：前端 `http://127.0.0.1:1441/ui/`，后端 `127.0.0.1:4441`，
数据位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-backup-preview-20261115/data`。
浏览器已确认新页面可加载、导入/导出/同步设置可操作、导出密码窗口可打开。
此实例没有复制真实账号，不是正式发布包；不应把空白预览当作 4400 数据迁移完成。

GitHub blob raw media type 已对照官方 REST 文档核验：
https://docs.github.com/en/rest/git/blobs?apiVersion=2022-11-28#get-a-blob
