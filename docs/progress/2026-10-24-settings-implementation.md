# 设置面板产品实现（2026-10-24）

## 范围与结果

本轮承接用户已认可的 HTML 方案，落实到真实 Web 控制台、可信本地设置窗口和
Tauri 原生命令，不再仅维护静态预览。保留工作树中原有的 API key、运维和路由等
其他修改；未提交、推送或重置仓库。

- 顶栏改为「界面偏好、后端连接、桌面行为、管理安全、诊断与日志」五个分类，
  替代中间栏重复的「设置」标题。分类支持方向键、Home、End 和可见焦点。
- 主题、语言改为单行 label＋下拉框，继续使用原 Provider 和持久化键，不再显示配图。
- 后端连接区将当前地址与待应用草稿分开；地址、测试连接、应用并连接同行。
  复制仅切换图标为勾，保持「复制」文案与宽度；反馈区域保留固定高度。
- 浏览器测试不带管理凭证、不跟随重定向、有超时及 256 KiB 读取上限。应用通过
  应用内确认前往目标 `/ui/` 独立登录，取消保留原页面并恢复焦点。由于 CORS
  不等同于能否导航，浏览器应用不以跨域测试成功为前提。
- 原生测试不保存配置、不启停服务。应用远端连接先检查目标，再保存并打开控制台，
  最后停止本客户端拥有的旧 sidecar；目标检查失败保留当前连接和本机进程。
- 编辑连接草稿清除旧测试反馈；持久化只保存规范化远端根地址，本机模式不保存
  未使用的远端草稿，拒绝把凭证或 URL 参数写入远端连接配置。
- 桌面设置接入真实的关闭行为、托盘菜单与当前用户 Windows 登录启动项。
  默认保持旧版关闭即退出；Ctrl+Q 始终退出；托盘不可用时不接受隐藏到托盘。
- 管理安全沿用真实管理密钥 API、鉴权、最后一把保护和删除确认。明文按需获取，
  15 秒、失焦、页面隐藏、会话变化或离开分类后移除，不保存到 localStorage。
- 诊断与日志接入保留时间、历史容量、打开目录和白名单 JSON 导出。

具体行为及权限边界见 `docs/settings.md`。

## 原生边界与兼容性

服务器提供的控制台仍无原生 IPC 权限。`lib.rs` 的非 `main` 拒绝规则及 capability
范围不变；后端连接、桌面行为、诊断分类只通过固定无参数导航动作打开本地可信
窗口，不向系统桥传输路径、配置或管理会话。本地管理安全分类返回已连接控制台，
不复制该控制台的登录凭证。

新增可选 `desktop-settings.json` 位于既有共享本地数据 owner，默认字段兼容旧安装，
有界读取并原子保存；不提升数据库版本，不创建便携 `.ng`。启动项只操作专用 HKCU
Run 值，不使用 shell、不修改其他值，未知未标记条目拒绝覆盖。

日志默认不自动清理且不限制历史容量。用户显式开启后，仅在下次本机后端启动时
清理新格式 `gateway-v2-*.log`；不递归、不跟随直接重定向项、扫描有界，保留旧版
日志和非本应用文件。活跃日志通过 Windows sharing／Unix 文件锁保护，不截断且
不计入历史容量。历史容量不是运行中硬限额，本轮未引入在线轮转。

诊断采用白名单，只含客户端类别、生成时间、偏好和有限运行状态，不含原始日志、
环境变量、用户路径、profile、路由文档或凭证；浏览器不冒充本机／服务器日志来源。

## 实际验证

- `npm run typecheck`：通过。
- 设置、连接 hook、窗口控件及样式定向测试：9 文件，55/55 通过。
- 真实 `BrowserConsoleApp.shell.test.tsx -t 'puts a settings button'`：1/1，
  其余 6 项未选中。定向前端共 56 项，不代表整文件或全仓回归全绿。
- Tauri `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --jobs 4`：
  最后整组 31/31 通过；`cargo fmt` 已执行并检查。
- 一轮并行前端验证期间，原有
  `state::tests::drop_requests_authorized_drain_before_waiting_for_child_exit` 出现一次
  `Drop must send an authorized drain request before process termination` 失败。
  随后该用例单独 1/1、整组 31/31 通过。未改该用例或退出实现，原因未证实，
  不以复跑成功抹除这次偶发记录。
- 有效代码行 checker：31/31；ratchet 通过，未修改 baseline 或 exception。
  本轮 39 个源码文件 UTF-8 无 BOM，最大 488 有效行。
- `git diff --check`：通过。
- 使用真实 `rsbuild.config.ts` 和 `main.tsx` 完成 Web、Tauri 两个前端目标编译。
  仅改输出目录并禁用 live-dist 发布／清理插件，不覆盖当前运行资产，不构成发布凭据。
- Playwright 对真实 AppShell、设置组件和 Provider 的隔离夹具验证通过：
  1600、1280、900、740、540、390px 无整页横向溢出；后端输入与按钮同行，
  错误／成功／原生 busy 状态几何稳定；取消切换保留页面并恢复焦点。
  中文和英文界面已查看，页面 JS errors 为空。管理 API 与原生 IPC 明确模拟，
  没有对真实系统执行设置写入。
- 浏览器有 `Password field is not contained in a form` 的 VERBOSE 提示，来自只读
  密钥展示 input，不作为 JS error，也不表述为零浏览器告警。

相邻 shell 文件仍有之前已记录的 default group 断言不匹配：
`renders non-codex providers in the same compact tree ledger layout` 期待
`data-group-id=""`，实际为 `"default"`。本轮未修改该业务或断言。

## 证据和未执行项

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-settings-implementation-20261024/`。

其中有隔离构建脚本及产物、布局验证脚本、`source-checks.json`、七张产品组件截图，
以及 `verification-summary.json`。截图中的后端地址、管理密钥名称和原生偏好为明确
的测试夹具，不是真实运行配置。

未生成新的产品 EXE 或 devtest 包，未进行正式发布、部署或现有实例重启，未修改真实
Windows 启动项、凭证、数据库或 `.ng`，未清理真实日志。未做 Windows 原生 GUI
端到端手测、真实登录启动验证、付费上游测试或全仓完整回归。

本轮创建的 `settings-product` 浏览器会话及 1439 端口的隔离预览服务已关闭。
