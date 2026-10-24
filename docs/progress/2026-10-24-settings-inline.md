# 设置分类内嵌显示修复（2026-10-24）

## 原因与修复

此前 `DesktopSettingsEntry` 在分类挂载时发送 `connection`、`settings-desktop`、
`settings-diagnostics`，原生 `show_category` 会显示另一个 `main` 设置窗口。
这保留了本机权限隔离，却将普通分类切换错误地变成跨窗口配置。

现在三个分类使用 `gateway-console` 原窗口中的 `gateway-local-settings` 本地子视图，
只占据当前设置内容区，不再调用上述开窗动作。保留原侧栏、顶部分类和后端管理会话。
主题、语言及内容背景保持一致；尺寸和侧栏变化只调整区域，不重新挂载表单；
父区域滚动时裁剪子视图，避免覆盖分类栏。离开分类或发生完整文档导航时销毁视图。

打开后端设置只读取配置，不触发自动连接、sidecar 启动或配置保存。
测试或应用连接失败留在当前页显示错误；原 `main` 启动恢复入口仍保留。

## 权限与生命周期

- 后端页面仍没有原生 IPC。展示桥只传三个白名单分类、有限尺寸、主题和语言，
  不传路径、配置、凭证或启动项。原生侧严格解析、限制 URL 大小并裁剪区域。
- capability 只列精确本地 WebView 标签；子视图只加载打包资产，拒绝远端导航和弹窗。
  子视图仅获设置相关命令，不能调用旧版 profile、进程控制或原始日志读取命令。
- 使用固定版本 Tauri 的 `unstable` feature 启用 child WebView API，未升级依赖或改锁文件。
- 高频几何请求合并为一个 worker 和一个最新待处理项；不堆积线程或无界队列。
  多 WebView 窗口使用原生 Window 句柄处理托盘、窗口按钮和关闭，OAuth 仍由控制台
  WebView 接收事件，不因新增子视图失去原窗口引用。

## 验证与边界

- TypeScript 类型检查通过；设置、连接 hook、窗口控件、样式共 10 文件 62 项通过。
- `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --jobs 4`：34 项通过。
- 启动恢复与内嵌设置 Python 合约：10 项通过；`cargo fmt --check` 通过。
- 有效行检查器 31 项、ratchet、`git diff --check` 通过；未改 baseline 或 exception。
  新增模块最大 168 有效行；源码使用 UTF-8 无 BOM。
- 隔离浏览器夹具使用真实设置组件，验证三分类原位显示、失败不离页、busy 分类锁、
  侧栏收展不重载、滚动裁剪、离开时清理、中英文，以及 1600/1280/900/540/390px
  无子内容横向溢出。无 popup、无页面 JS error。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-settings-inline-20261024/`。
浏览器夹具明确以 iframe 模拟原生子视图，IPC 和系统操作均 mock，不能代替真实
Windows child WebView 手测。原生测试为库单元测试，不是已运行产品 GUI 的证明。

本轮未编译、打包或替换产品 EXE，未部署或重启现有实例；未修改真实启动项、
凭证、数据库、`.ng` 或日志。保留前序 dirty changes，未提交或推送。
