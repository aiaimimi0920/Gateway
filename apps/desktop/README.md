# Neuro Gateway Desktop

`Gateway/apps/desktop` 是 `Gateway` 子项目的本地桌面壳层，使用 Rust + Tauri + React 构建。

## Product boundary

- `gateway.exe` 是独立核心运行时，通过 HTTP 的 `/ui/` 提供完整管理后台。
- `gateway-ui.exe` 自动运行本地核心，或连接指定的已有 Gateway。
- 管理窗口直接加载所选服务器的 `/ui/`，与浏览器使用完全相同的页面。
- 服务器页面没有 Tauri 原生权限；本地连接设置窗口单独持有原生权限。
- 桌面窗口沿用 Loom 的无系统标题栏布局；窗口按钮与连接入口合并到应用顶栏。
  窗口操作使用仅含固定动作的本地导航桥，不能读取文件、修改连接配置或调用进程命令。
  旧服务器页面未确认支持应用内窗口按钮时，保留系统标题栏与菜单。
- 桌面壳层只负责：
  - 保存本机或已有服务器的连接选择
  - 本机 sidecar 生命周期与启动检查
  - 打开服务器管理页面和本地连接设置窗口
- 桌面壳层不复制 Gateway 的 provider routing、credential pipeline、request pipeline、management core。

## Build

在仓库根目录运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File Gateway\tools\build-gateway-release.ps1
```

单独调试前端：

```powershell
cd Gateway/apps/desktop
npm install
npm run typecheck
npm run build
```

单独调试 Tauri：

```powershell
cd Gateway/apps/desktop
npm run tauri -- dev
```

## Runtime model

默认使用本机模式：自动准备用户目录中的空路由配置与状态，启动随包
`gateway.exe`，然后打开管理窗口。新安装默认管理密钥为 `11011101`。
无需安装 Redis 或 PostgreSQL 即可登录、编辑和保存本地路由配置；供应商账号需在后台自行配置，
需要数据库或 Redis 的高级服务能力仍取决于服务端配置。
默认端口 4200 已占用时使用空闲端口，不接管或停止已有服务。

管理窗口右上角的“连接设置”（Ctrl+Shift+G）可切换到“连接已有 Gateway”，
输入 HTTP(S) 服务器地址。连接模式会保存，重启后保持；连接失败不会启动
本地后端。服务器自己的认证与远程访问策略继续生效。
关闭管理窗口退出客户端，并清理本客户端持有的本地后端；外部服务不受影响。

本地数据默认位于当前用户的 `~/.ng`。如果 EXE 同级已有 `.ng` 目录，
则优先使用该目录作为便携数据目录；不会自动创建 EXE 同级目录。
路由和状态位于 `local` 子目录，连接选择位于 `connection.json`，运行时对象
位于 `objects`，WebView 数据位于 `webview`。两个 EXE 共用同一目录规则。
`GATEWAY_DATA_DIR` 可显式指定绝对数据目录；保留 `GATEWAY_UI_DATA_DIR` 测试隔离覆盖。
普通更新优先兼容旧格式；必要的迁移随 EXE 编译发布并在启动时自动执行。
详见 [本地数据与迁移规则](../../docs/local-data-storage.md)。
本机模式忽略继承的 Gateway 环境配置和包目录及其父目录的 `.env`，
使用本地版本档案、事务日志和原子激活记录，在重启时恢复已保存配置。
独立运行的 headless Gateway 可显式设置 `GATEWAY_CONSOLE_STORAGE=local`
并使用 `standalone` 角色启用同一存储方式；原有 Redis 部署保持原有配置权威来源。
不要在同一个状态目录中直接切换 Redis 和本地配置存储。

以下底层 profile/进程命令仍由桌面原生层提供，供诊断和自动化使用：

桌面 UI 通过 Tauri 调用本地 Rust 命令，Rust 命令再负责：

1. 读取和保存 profile；
2. 启动/停止 `gateway.exe` sidecar；
3. 读取 sidecar 日志；
4. 暴露运行时信息给前端。

桌面 UI 通过 Gateway 自己的 HTTP API 读取：

- `/healthz`
- `/readyz`
- `/v1/models`
- `/v1/chat/completions`（最小 API 测试）

## Release artifacts

`Gateway` release 构建会同时输出：

- `gateway.exe`
- `gateway-ui.exe`

目标发布目录：

```text
<Gateway-repository>\release\Gateway\<versionId>
```

在 Neuro monorepo 中，也可以按工作区发布规范显式传入：

```text
C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\<versionId>
```
