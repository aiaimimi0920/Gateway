# Neuro Gateway Desktop

`Gateway/apps/desktop` 是 `Gateway` 子项目的本地桌面壳层，使用 Rust + Tauri + React 构建。

## Product boundary

- `neuro-gateway.exe` 是无 UI 的核心运行时。
- `neuro-gateway-ui.exe` 是给普通用户使用的本地桌面启动器壳层。
- 桌面壳层只负责：
  - 本地 profile 管理
  - sidecar 生命周期
  - 状态轮询
  - `/healthz`、`/readyz`、`/v1/models`、最小 API 测试
  - 日志查看
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

桌面 UI 通过 Tauri 调用本地 Rust 命令，Rust 命令再负责：

1. 读取和保存 profile；
2. 启动/停止 `neuro-gateway.exe` sidecar；
3. 读取 sidecar 日志；
4. 暴露运行时信息给前端。

桌面 UI 通过 Gateway 自己的 HTTP API 读取：

- `/healthz`
- `/readyz`
- `/v1/models`
- `/v1/chat/completions`（最小 API 测试）

## Release artifacts

`Gateway` release 构建会同时输出：

- `neuro-gateway.exe`
- `neuro-gateway-ui.exe`

目标发布目录：

```text
<Gateway-repository>\release\Gateway\<versionId>
```

在 Neuro monorepo 中，也可以按工作区发布规范显式传入：

```text
C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\<versionId>
```
