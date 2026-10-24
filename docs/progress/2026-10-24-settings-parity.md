# 网页与桌面设置归属统一（2026-10-24）

## 本轮范围

遵循用户确认的“一套管理控制台，桌面仅增加本机能力”设计，不改变三发行版结构。

| 分类 | 桌面 EXE | 普通浏览器 | 设置归属 |
| --- | --- | --- | --- |
| 界面偏好 | 显示 | 显示 | 当前 origin 的 localStorage |
| 后端连接 | 显示 | 隐藏 | gateway-ui 本机连接选择 |
| 桌面行为 | 显示 | 隐藏 | gateway-ui 本机关闭、托盘及启动项 |
| 管理安全 | 显示 | 显示 | 当前连接的 Gateway |
| 诊断与日志 | 显示 | 显示 | 当前连接的 Gateway 后端机器 |

浏览器切换 Gateway 时直接访问另一个地址，不提供无实际切换作用的后端表单。
分类键盘导航按实际可见项循环，不再假定总是五项。
启动恢复窗口只保留后端连接和桌面行为，不维护另一套本机日志设置。

## 实现

- `SettingsNavigation` 统一分类可见性；`ConsoleSettingsWorkspace` 复用管理安全及
  `GatewayDiagnosticsSettings`，后端连接、桌面行为继续使用同窗口可信本地子视图。
- 远端控制台依旧没有 native IPC。原生展示桥只允许 backend、desktop 两类，
  移除 diagnostics 分类及本机日志导出权限，防止远端连接时误导出客户端日志。
- 主题、语言继续使用 `gateway-ui-theme`、`gateway-ui-locale`，初始化恢复、刷新保留，
  不写到后端，不更换存储版本。
- 新增后端诊断与日志 API：`GET/PUT .../console/diagnostics/settings`、
  `POST .../console/diagnostics/export`。使用既有管理员鉴权、remote-access 和 no-store 边界，
  不接受用户传入文件路径，纯 API 构建也可使用这些管理接口。
- 后端 tracing 负责实际文件写入，独立运行时同时保留 stdout；桌面 launcher 显式传递
  捕获日志文件，后端避免重复输出。日志策略保存在后端 state 中。
- 日志导出有文件数量、扫描数量及每文件读取上限，丢弃首条不完整记录，并对重复出现的
  token、授权头、密码、密钥及 URL 凭证等脱敏。诊断报告只导出白名单，不序列化 Config。
- 文件操作拒绝静态链接、reparse point、硬链接和非普通文件。Windows 共享模式及
  Unix 文件锁保护活跃日志。目录必须由可信操作系统账户管理，完整边界见 `docs/settings.md`。
- 前端只下载当前后端响应，切换后端、会话变化、离开页面会取消并丢弃旧请求；
  请求有 30 秒超时，避免断开的远端使分类永久锁定。复制仅切换图标，文字仍为“复制”。

日志保留与历史容量是**下次该 Gateway 启动时**执行的历史清理策略，默认不清理；
不截断活跃日志，不等同运行中的总容量硬限制。本轮没有引入在线日志轮转。
旧 desktop 日志字段继续兼容读取、原样保存，但不再驱动清理或由诊断面板修改。

## 验证结果

- 前端设置、连接、窗口控件、控制台 shell、路由加载、默认权益组和样式：15 文件 78 项通过；
  TypeScript 检查通过。扩展回归发现 shell 中“未分配组为空”的旧断言，核对既有
  `defaultAccountGroup` 及其保护测试后改为 `default`；没有改动分组产品逻辑。
- Rust 后端诊断单元测试：7 项通过。覆盖策略持久化、损坏文件保护、尾部读取边界、
  脱敏、硬链接拒绝及当前/活跃/无关日志保护。
- `console_diagnostics_contract`：3 项通过，覆盖管理鉴权、远端访问边界、后端持久化、
  两种导出、no-store、非法策略及任意路径拒绝。
- Tauri 库定向测试：panel_request 3 项、chrome 1 项、desktop_settings 3 项通过。
- 内嵌设置和启动窗口 Python 合约：11 项通过。
- 有效行检查器 31 项及 ratchet 通过；root/native `cargo fmt --check`、`git diff --check` 通过。
  本轮审查的源码均为 UTF-8 无 BOM，包含未跟踪文件的空白检查，最大 316 有效行，
  无新增超限文件、无 baseline 或 exception 修改。

隔离浏览器使用真实组件与 mock API/桌面桥，确认 3/5 分类、主题及语言刷新恢复，
诊断页面不请求原生子视图，策略通过同一后端 API 保存，JSON 和日志实际下载到测试目录。
1280px 与 540px 内容区无页面横向溢出，无 popup、无 console error/warning。
这不是实际 Windows child WebView 或跨机器部署验收。

另使用 Cargo 测试生成的开发态 backend 在全新隔离目录与独立端口启动，验证实际文件
tracing、日志导出、诊断 PID 与该进程一致、匿名访问拒绝、策略保存和重读；启动日志标记
仅写入一次，无 ANSI。结束后已停止本轮进程与浏览器/HTTP 夹具。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-settings-parity-20261024/`。
其中含浏览器截图、下载文件、源码哈希与行数、`runtime-smoke-result.json`。

## 交付边界

本轮只修改源码并做开发验证，没有生成新的交付 EXE/手测包，没有正式发布、部署或替换
现有程序。没有修改真实 `.ng`、凭证、数据库、启动项或日志，没有重启现有服务。
保留工作树前序修改，未提交、推送。未运行全量上游、付费服务或正式 release 审计。
