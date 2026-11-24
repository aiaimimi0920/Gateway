# Luma MCP 配置变更后的任务恢复

本批只推进 L1 已有 Local 创作任务的配置变更恢复，不重新开启已通过的模型、Files 或打包验收。真实账号只读授权继续有效；此前获准的一次显式刷新已经执行，本批没有再次刷新真实凭据，也没有执行供应商收费生成。

## 改了哪些内容

过去任务读取要求 `task.revision == expectedRevision`，因此修改无关账号配置或正常 OAuth 刷新后，旧任务连查询和结束会话也无法执行。现在保留任务创建时的 revision，使用原配置归档证明原账号，再核对当前配置：

- 原 access token、refresh token 与完整 OAuth context 相同，允许无关配置变更后的访问；没有离线刷新权限的账号也适用。
- 令牌变化时，只沿同 provider／credential、原材料完全对应且已处于 `Published` 的持久刷新记录向前核对。支持多次刷新，中间的无关配置变更不打断身份连续性。不会读取 JWT 猜身份，也不把相同 client ID 当作相同账号。
- 缺少原配置归档、缺少刷新记录、记录尚未发布、账号被替换或链无法验证，返回 `luma_mcp_task_identity_unverified`，不发上游请求。每次最多读取 32 代刷新记录，重复材料立即停止。
- 原任务读取、取消、结束、尚未发送的精确请求提交及素材复用使用同一证明。原任务 ID／revision／工作区／费用事实保留，不迁移数据库或把任务改挂到新账号。
- 管理请求仍必须携带当前 revision；执行中的配置变化仍会阻止后续生成，最终响应也要通过当前授权与 revision 校验。`opening`／`submitting` 的不确定结果仍不重放生成。
- 界面在配置／secret grant 变化后保留已输入的任务 ID，清除旧结果和费用确认，等用户显式查询；切换账号或管理身份仍清空任务 ID。响应外层绑定当前 revision，任务自身保留历史 revision。

实现位于 `src/http/routes/internal_console/luma_mcp/creation/identity.rs`，由既有 journal 和 workspace 入口调用；没有新增后台刷新器、平行凭据库或存储版本。

## 验证

定向执行记录归档到 `docs/evidence/luma/gateway-luma-mcp-recovery-20261120/verification.json`。Rust 首轮为 **25 passed / 1 failed**：多次刷新夹具在第一次刷新成功后调用 `Registry::join()`，该方法会关闭注册器，第二次准备刷新因而返回 400。删除夹具中这次过早关闭，保留正常 runtime shutdown 的收尾；产品代码没有因该测试失败改动。修正后仅复跑该项，**1 passed / 0 failed**，最终 26 个不同 Rust 用例都有通过证据，未重跑已通过的 25 项。两轮编译分别 21 分 16 秒、19 分钟；定向测试分别 28.05 秒和 1.56 秒，未运行全部 6454 项测试。

新增 Rust 用例覆盖无关配置修改、两次正常显式刷新及重启后原任务查询／素材复用、同 ID 换账号拒绝、缺原归档拒绝、未发布刷新结果拒绝、未确认提交不重放，以及 session open 期间换账号阻止生成。刷新测试通过正常管理 API 和一次性本机 OAuth peer 执行，真实账号和 4200 实例没有修改。

前端相关 14 个不同用例通过：原有 Panel／Reuse 的 10 项通过；新增 Recovery 的 4 项通过。新增测试最初遗漏循环变量的 `const` 声明，修正后只重跑该文件及类型检查，保留首次失败回执。TypeScript 检查、31 项 checker tests、规模 ratchet、改动 Rust formatter、UTF-8 无 BOM 和 `git diff --check` 通过；既有无关规模债务不在本批处理。

本批最大生产文件为 store 的 195 有效行，新增身份核对模块 95 行；新增恢复测试 181 行、测试夹具 120 行。长期测试和源码均在仓库，临时目录只放执行产物。

## 剩余边界

该增量关闭 L1 的 Local 配置变更恢复子项，不等于整个 L1 完成。分层布局、run 用户交互、Server 持久身份与供应商真实业务仍未关闭。重新登录或手工粘贴的新 token 如果没有可验证刷新链，即便用户认为属于同一账号，也不会自动继承旧任务。

L2 当前浏览器入口本批再次初始化仍返回 `Module not found: @oai/cua/tinyskyAlt`，没有取得 Web 当前 wire/session；不要求用户重复登录。L4 完整 Server／Redis 与强杀边界、L6 真实业务验收仍单独保留。按原六包计仍有 4 包未整体关闭。

未出新 EXE；现有 `gateway-devtest-20261120-luma-mcp-options` 不包含后续素材复用、全部模型和本批恢复增量。未运行全量 Rust／全部前端／正式发布检查，也没有提交或推送。
