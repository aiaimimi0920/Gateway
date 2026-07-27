# Neuro Gateway

[![CI](https://github.com/aiaimimi0920/Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/ci.yml)
[![Build Windows](https://github.com/aiaimimi0920/Gateway/actions/workflows/build-windows.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/build-windows.yml)
[![Docker](https://github.com/aiaimimi0920/Gateway/actions/workflows/docker.yml/badge.svg)](https://github.com/aiaimimi0920/Gateway/actions/workflows/docker.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

[English](README.md) | **简体中文**

Neuro Gateway 是一个可独立运行的 Rust AI 网关与模型提供方中继服务。它提供
OpenAI 兼容接口和提供方专用接口，负责线路清单、路由、运行时凭据、浏览器
Worker，并同时交付无界面的核心服务与 Windows 桌面启动器。

独立仓库：<https://github.com/aiaimimi0920/Gateway>

Gateway 可以作为个人电脑上的本地产品独立运行，也可以由 Neuro Platform 或
Loom 通过 API 管理。Platform 负责账号、权限、配额、计费与公开网站策略；
Loom 负责编排和本地 AI 大脑工作流；Hook 负责前台采集与集成入口。

## 项目职责

- Rust 网关运行时和 HTTP/WebSocket API：`gateway`。
- 提供方路由、运行时读取的凭据与请求中继。
- `manifests/` 下的线路清单和路由配置。
- `scripts/` 下基于浏览器的提供方 Worker。
- Gateway 管理 API 和本地桌面外壳：`gateway-ui`。
- Windows 发布包、完整性清单、校验和与运行证据工具。

Platform、Loom 和 Hook 的实现代码不会复制进本仓库。模块职责和集成边界见
[`INTEGRATION_CONTRACT.md`](INTEGRATION_CONTRACT.md)。

## 目录结构

```text
./
├── apps/desktop/       # TypeScript 前端与 Tauri 桌面启动器
├── deploy/             # 官方 Docker Compose 服务端部署栈
├── manifests/          # 提供方线路清单与 JSON Schema
├── scripts/            # 浏览器 Worker 与 Node 测试
├── src/                # Rust 库与 gateway 二进制
├── tests/              # Rust/Python 合同测试
├── tools/              # 校验、构建、打包与冒烟工具
├── routes.yaml         # 当前开发线路配置
└── routes.example.yaml # 可移植配置示例
```

## 环境要求

- Windows 10/11 或当前受支持的 Linux 发行版。
- Rust `1.91.1`，或 `rust-toolchain.toml` 固定的版本。
- Node.js `22` 与 npm。
- Python `3.11+`。
- Windows 打包需要 PowerShell 5.1+。
- 构建容器镜像需要 Docker Engine 或 Docker Desktop。

## 独立克隆与验证

以下命令都在 Gateway 独立仓库根目录执行：

```powershell
git clone https://github.com/aiaimimi0920/Gateway.git
Set-Location Gateway

cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked
```

运行线路清单和 Python 合同校验：

```powershell
python -m pip install --disable-pip-version-check -r tests/python/requirements.txt
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
```

安装并测试浏览器 Worker：

```powershell
npm ci --prefix scripts
node --test scripts/tests/*.test.mjs
```

验证桌面前端和 Tauri 外壳：

```powershell
npm ci --prefix apps/desktop
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## 运行时配置

服务从环境变量读取运行配置。`Config::from_env` 要求
`GATEWAY_REDIS_URL`；`GATEWAY_DATABASE_URL` 或 `DATABASE_URL` 可选。
本地开发可以从 `.env.example` 创建 `.env`，而 `.env` 和其他本地环境
文件不会进入 Git。

当前开发快照会继续保留已版本化的 `routes.yaml` 和其他开发状态。验证提供方
线路矩阵时，不要把 `routes.yaml` 替换成空配置。

启动无界面核心服务：

```powershell
cargo run --locked --bin gateway
```

启动桌面开发环境：

```powershell
Push-Location apps/desktop
npm run tauri dev
Pop-Location
```

## Windows 可移植发布包

默认输出目录是当前独立仓内的 `release/Gateway/<versionId>`。每个版本目录
不可覆盖，包含两个 Windows 可执行文件、线路配置、`.env.example`、精确构建
来源、清单、浏览器 Worker、文档、官方 `deploy/` 目录、工具、`manifest.json` 和
`checksums.sha256`。

在仓库根目录构建和打包：

```powershell
$id = "gateway-product-" + (Get-Date -Format "yyyyMMdd-HHmmss")
.\tools\build-gateway-release.ps1
.\tools\package-gateway-release.ps1 -VersionId $id -SkipBuild
```

使用 `-SkipBuild` 时，打包器会要求
`target/release/gateway-build-provenance.json` 与当前源码树、两个可执行文件
完全一致。省略 `-SkipBuild` 可以让打包器自行执行构建。

当前 Neuro 大工程要求本对话的正式产物写到固定目录，可以显式传入该路径：

```powershell
$neuroReleaseRoot = "C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway"
.\tools\package-gateway-release.ps1 `
  -VersionId $id -SkipBuild `
  -ReleaseRoot $neuroReleaseRoot -AllowCustomReleaseRoot
```

生成确定性 ZIP 与 SHA-256 文件：

```powershell
.\tools\compress-gateway-release.ps1 `
  -ReleaseDir ".\release\Gateway\$id" `
  -OutputDir ".\release\Gateway\packages" `
  -VersionId $id
```

输出文件为 `Gateway-<versionId>-windows-x64.zip` 和
`Gateway-<versionId>-windows-x64.zip.sha256`。

验证发布包完整性：

```powershell
.\tools\smoke-gateway-packaged-runtime.ps1 `
  -ReleaseDir ".\release\Gateway\$id" -IntegrityOnly
.\tools\smoke-gateway-ui-release.ps1 `
  -ReleaseDir ".\release\Gateway\$id"
```

发布包不会携带 `scripts/node_modules`，解压后按需安装：

```powershell
npm ci --prefix ".\release\Gateway\$id\scripts"
node --test ".\release\Gateway\$id\scripts\tests\*.test.mjs"
```

完整提供方证据矩阵仅适用于源码仓库，因为它依赖 Cargo 源码与仓库验证工具。
可移植包会携带 `scripts/invoke-gateway-live-provider-canary.ps1`；默认可运行
安全的 dry-run 合同，只有显式传入 `-AllowLiveProviderCalls` 和所需输入后才
会调用真实提供方。详细说明见
[`docs/provider-evidence.md`](docs/provider-evidence.md)。

## Docker

服务端正式部署请优先使用 [`deploy/`](deploy/README.md) 目录下的官方
Compose 栈。推荐的本地目录版本会把路由文件与 Redis 数据直接落到宿主机：

```bash
cd deploy
chmod +x docker-deploy.sh
./docker-deploy.sh
docker compose -f docker-compose.local.yml up -d
```

这套一键准备流程是 Gateway 对齐 Sub2API 推荐 Docker Compose 部署方案的做法：
先生成 `.env`、自动补齐关键密钥、创建本地持久化目录，再启动
`docker-compose.local.yml`。

```bash
cd deploy
cp .env.example .env
mkdir -p gateway_data redis_data
docker compose -f docker-compose.local.yml up -d
docker compose -f docker-compose.local.yml logs -f gateway
```

镜像默认以 `standalone` 模式启动，并会在首次启动时自动把
`routes.example.yaml` 初始化到持久化的路由文件位置。

也提供命名卷版本：

```bash
cd deploy
cp .env.example .env
docker compose up -d
docker compose logs -f gateway
```

根目录下的 `docker build` / `docker run` 仍然保留，适合开发阶段验证镜像：

```powershell
docker build -t gateway:local .
docker run --rm -p 4200:4200 --env-file .env gateway:local
```

镜像包含核心二进制、线路配置、提供方清单、浏览器 Worker，以及一个会在持久化
路由文件缺失时自动初始化 `/data/routes.yaml` 的容器入口脚本。运行时凭据通过
环境变量或 env 文件提供，Docker 构建过程不会读取本地 `.env`。

如果希望直接在仓库根目录下一键管理 Docker 部署，可使用：

```powershell
.\tools\deploy-gateway-docker.ps1 -Action up -Mode local
.\tools\deploy-gateway-docker.ps1 -Action logs -Mode local -Follow
.\tools\deploy-gateway-docker.ps1 -Action down -Mode local
```

这个 PowerShell helper 针对“本地新用户直接启动”的场景做了默认优化。首次执行
`-Action up` 时，它会默认写入 `GATEWAY_BIND_HOST=127.0.0.1`，并在 loopback
绑定场景下自动补齐以下控制台登录配置，且不会覆盖你已经显式设置过的值：

- `GATEWAY_MANAGEMENT_TOKEN=123456`
- `GATEWAY_CONSOLE_REMOTE_ACCESS=true`

启动完成后，直接访问：

- `http://127.0.0.1:4200/ui/`

然后使用管理密钥 `123456` 登录即可。如果你要部署成对外提供服务的服务器形态，
请显式传入 `-BindHost 0.0.0.0`，并在暴露端口前改成你自己的管理密钥。

如果要对本地源码做一次完整的 Docker 端到端验证，可运行：

```powershell
.\tools\verify-gateway-docker-stack.ps1 -BuildImage
```

## GitHub Actions

- `ci.yml`：Windows/Linux、Python、Node、Rust、桌面前端和 Tauri 验证。
- `build-windows.yml`：构建并上传 `release/Gateway` 下的 Windows 候选包。
- `docker.yml`：PR 只构建；只有 `main` 或 `Vx.y.z` tag push 才发布
  `ghcr.io/aiaimimi0920/gateway`，并会先用官方 Compose 栈做本地部署验证。
- `release-tag.yml`：只接受 `Vx.y.z`，校验后生成 ZIP、SHA-256，并发布
  GitHub Release；同时会附带 `Gateway-Vx.y.z-docker-deploy.zip` 与对应校验文件。

## 许可证

Gateway 使用 [MIT License](LICENSE)。
