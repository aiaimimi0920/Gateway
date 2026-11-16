# Luma MCP 后台授权会话、回调和保存增量

## 已实现的流程

在上一批独立只读MCP账号保存链路上，新增运行实例持有的Luma授权会话：显式已登记public client与精确loopback redirect → 先绑定端口再返回授权URL → 用户官方登录/MFA → 一次性code交换 → Ready → 管理用户显式import → 正常SecretPatch/配置编译/revision CAS保存。

管理路径为`/v1/internal/gateway/console/luma-auth-sessions`（创建）、`/:id`（查询）以及`/:id/:action`（cancel/complete/import）。创建和所有action要求管理认证与secret consent；查询检查同一owner。所有响应含no-store，参数采用deny_unknown_fields。callback本身没有保存权限，不能触发自动落库或工具调用。

## 权限、期限和资源所有权

- client由调用者显式提供，不自动登记、不硬编码调查client、不借用第一方身份；固定issuer/resource/token endpoint。重授权须保持原client/redirect，保留credential ID及操作员元数据，不断言同一个Luma用户/团队身份。
- 同一实例最多16个会话、32个已接受操作，创建另有16个并发准入名额；授权窗口最多20分钟。原token绝对expiry进一步限制Ready/保存预算，不在保存时延长。
- 错误state/无关回调不消费等待状态；正确拒绝为终态。交换被取消或future被丢弃后不可重放code，状态明确Failed。过期/取消/失败短期可查询，不能复活材料。
- 绑定严格的`127.0.0.1:<registered-port>/callback`，占用时失败，不静默换端口。沿用有界HTTP头解析；读头5秒、回复1秒，监听同时受会话期限与取消约束。
- 显式import一旦接受即由运行实例持有，不依赖原HTTP请求继续存在。保存最多20秒、publication lock后复核期限、单次CAS；重复import不重复提交。超时/失败/未poll即丢弃记录SaveUnconfirmed及目标ID，不伪报成功或自动重试。
- 生命周期接入正常spawn/shutdown/join；关停关闭新准入、取消等待/交换、等待创建退场并join已接受保存。没有新独立代理服务，也不持久化PKCE verifier、code或等待会话。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-login-20261113`。

- `red.log`：新增管理权限/revision两项测试在旧实现实际失败，端点尚未接入。
- 已独立只读核验状态、锁、权限、Keep和期限；未发现确定阻断问题。
- 离线测试使用实际管理Router、真实loopback socket及synthetic token peer。token endpoint替换仅在cfg(test) task-local中编译，生产路径不接受替换URL。
- 测试包含新建与重授权、错误state后正确callback、显式import权限、秘密不回显、保留ID/元数据、重复import、授权后修改revision、交换future abort、保存future未poll即drop、锁等待过期、owner隔离、容量及端口释放。
- 首轮`green.log`保留112通过、2失败、2忽略的真实结果：保存任务返回前guard被提前析构，导致成功保存和锁等待过期两项错误地观察到`luma_login_save_unconfirmed`。`guard-repro.rs`/`guard-repro.log`最小复现确认此行为；改为`guard.disarm()`让future持有整个guard，并增加构造future后必须仍为Importing的断言，不放宽原保存/期限契约。
- `green-retry.log`：修复后119项通过、0失败（lib 114、目录管理集成3、授权管理集成2）；2项要求独立浏览器会话的专项忽略。7项新增后台会话lib测试全部通过。测试构建8分14秒，执行分别20.21/0.26/0.13秒，不将编译耗时当测试耗时。
- 格式、checker测试、修复后有效行ratchet和本批23项源码空白检查通过；`scope-hashes.json`记录源码SHA-256。修复后save为129有效行、lifecycle测试183有效行，无需例外或baseline调整。`production-check.log`：锁定依赖、离线、非测试runtime-defaults lib编译检查通过，存在工作区既有warning，不称零警告。
- `final-verification.log`：测试及编译完成后23项源码SHA-256全部匹配，范围内源码/文档空白与UTF-8无BOM检查通过。源码未在验证后继续修改。

## 范围边界

本批不打开浏览器、不创建真实授权会话、不读取/修改现有凭据或数据库，不实际调用Luma、不生成/下载/消费额度、不出包或提交Git。临时测试存储和产物均位于linshi。

此处loopback接收端属于Gateway进程所在机器；跨机器浏览器/Server联合流程未验收，不能当远程授权解决方案。桌面一键授权/保存UI、动态client注册、refresh、真实账号联合验收、model_list及生成/取消/费用、远端MCP session DELETE仍未完成。当前不增加业务模板、不将后台工作流当原生桌面交付。
