# Web 专用账号导入与模板蓝色登记修复

## 本轮范围

- `aistudio-web`、`gemini-web`、`gemini-canvas` 接入全局“添加账号”的专用网页登录
  控件，提供可选账号名称，不再要求用户把这些 Web 账号作为通用 API Key 录入。
- 保留 `chatgpt-web` 已有网页登录／网页会话 JSON 组件；没有新增 Google Cookie
  JSON 粘贴或文件导出入口。识别基于持久化 preset，支持改名的服务商及 Canvas 旧 preset。
- AI Studio 账号库“手动添加”接入同一管理认证会话；成功结果沿现有草稿、模型池、
  模型路由及 revision/CAS 保存链合并，不丢弃池中已有账号，不启用账号自动测试。
- 为 `gemini-canvas` 和 `aistudio-web` 补齐 `TEMPLATE_ADAPTATION_RECEIPTS` 登记。
  `gemini-web` 保持已验收，`chatgpt-web` 仍未标蓝，不拿 bootstrap 403 或 mock 代替真实生成。

## 实现与安全边界

管理 API 复用现有 `gemini-auth-sessions` URL，以加法式 `aistudio-web` family 维持旧
合同。创建前要求管理认证、目标服务商存在、family 与 preset 匹配，账号名称最多
128 字符且不含控制字符。AI Studio 异步结果合并前再次校验目标与 draft 归属。

AI Studio 登录采用已成功使用过的两阶段流程：普通 Chrome 优先、Edge 备选；登录
期间无自动化／调试连接，用户关闭本次专用窗口后才短暂重开并读取必要 scoped Cookie。
新增管理链本身仍需真实重新登录验收；本轮没有启动真实登录、生成或付费上游请求。

Node 结果仅经私有 pipe 交给 Rust，再由正常 Gateway 对象存储 owner 保存；管理响应
只返回对象引用、模型和账号标签。只读 bootstrap／ListModels 不调用 GenerateContent、
Drive、上传或 WAA。目录只选当前实现支持且实际存在的四个模型，不制造 verified_models。

新导入仅限 Windows 本地桌面，同一进程并行 1 个，外层 25 分钟超时，沿现有进程树
监管回收；终态会话有保留数量限制。Cookie bundle 限 32 KiB／64 条，Rust 解析前拒绝
超过 64 KiB 的私有结果，不向错误或普通日志暴露 Cookie。子进程收集仍使用已有有界
生产 I/O owner，并非新增完整浏览器状态导出。

## 定向验证

- 前端 TypeScript：通过。
- 新导入控件、实际添加账号弹窗接线、账号合并、旧凭据保留、蓝色登记、既有 Canvas
  导入、认证 hook 与 API schema：9 个测试文件、69 项通过。
- AI Studio 管理 worker／scoped Cookie／普通登录浏览器合同：9 项通过；只使用合成材料。
- 行数 checker 自测：31 项通过；仓库 effective-line ratchet 通过。
- `cargo fmt --all -- --check`、`git diff --check`：通过。
- Rust 认证会话定向编译与测试：23 项通过。命令为
  `cargo test --locked --lib console::gemini_auth_sessions --jobs 2 --no-default-features --features runtime-defaults`。
  本次 debug 测试目标增量编译约 13 分 48 秒；既有 unused／dead-code 与 linker 信息
  warnings 保留，未因此扩大修改范围。
- 额外 Gemini Web／Canvas／AI Studio 模板及 Business 兼容回归：4 个文件、14 项通过。
- 使用当前 Rsbuild 配置执行隔离桌面前端 bundle 构建：通过，输出仅在临时证据目录，
  没有覆盖当前 `dist` 或运行实例，也没有生成 EXE。

修复过程中保留失败事实：曾出现新增测试引用不存在的 API spy、旧 Canvas 测试使用
过时显示名称、新 Rust 模块误引用私有模块；均按真实接口／当前显示名称修正后重验。
不修改产品名称来迎合过时测试，不通过制造假 API 或扩大蓝色范围让测试变绿。

本轮关键有效行数：`gemini_auth_sessions.rs` 500 → 446；提取的 capture owner 123，
新 AI Studio auth owner 208；`CredentialDialog.tsx` 400 → 404；其余本轮文件均未超过
500，不新增行数例外。统计详见临时证据目录的 `effective-lines.json`。

## 蓝色登记依据与未验边界

- Canvas：`2026-10-31-gemini-canvas-http-only-r32.md` 的真实公共 HTTP 调用，不包含音乐
  或视频生成。
- AI Studio：`2026-11-01-aistudio-http-public-api-r36.md` 的真实公共 API 非流式／buffered
  SSE 调用。r37 多模态保留首批失败及后续复验，不解释为稳定性问题全部解决。
- 新导入控件的合成回归不等于 Google 重新登录整链已通过，也不表示 Cookie 永久有效。

没有编译或替换 EXE，没有重启／修改既有运行实例、路由修订、账号或凭据，没有提交推送。
旧 r38 手测 EXE 不会自动获得这次源码修改。

临时验证产物位于：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-import-20261101/`。
