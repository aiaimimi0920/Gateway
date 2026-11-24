# Neuro Gateway

[文档导航](docs/README.md)

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
Loom 负责编排和本地 AI 大脑工作流；Otter 负责前台采集与集成入口。

## 项目职责

- Rust 网关运行时和 HTTP/WebSocket API：`gateway`。
- 提供方路由、运行时读取的凭据与请求中继。
- `manifests/` 下的线路清单和路由配置。
- `scripts/` 下基于浏览器的提供方 Worker。
- Gateway 管理 API 和本地桌面外壳：`gateway-ui`。
- Windows 发布包、完整性清单、校验和与运行证据工具。

Platform、Loom 和 Otter 的实现代码不会复制进本仓库。模块职责和集成边界见
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
- Rust 版本以 [`rust-toolchain.toml`](rust-toolchain.toml) 为准。
- Node.js `>=22.22.0` 与 npm。
- Python `3.11+`。
- Windows 打包需要 PowerShell 5.1+。
- 构建容器镜像需要 Docker Engine 或 Docker Desktop。

## 独立克隆与验证

以下命令都在 Gateway 独立仓库根目录执行：

```powershell
git clone https://github.com/aiaimimi0920/Gateway.git
Set-Location Gateway

npm run test:effective-lines --prefix scripts
npm run check:effective-lines --prefix scripts
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked -- --test-threads=1
```

有效代码行门禁采用 Neuro 的 150/500/700/1500 阈值和已审查的采用时基线。
CI 会阻止新增超大文件和既有 700 行以上文件继续增长，但不会要求无关任务先
清偿全部历史旧债。扫描器、严格审计、例外 schema、支持语言和显式排除规则见
[`docs/effective-code-lines.md`](docs/effective-code-lines.md)。

仓库 Cargo 配置是直接构建时的保守兜底。日常 Windows 出包使用[增量手测流程](docs/devtest-build.md)，不走正式发布流程。

运行线路清单和 Python 合同校验：

```powershell
python -m pip install --disable-pip-version-check -r tests/python/requirements.txt
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
```

安装并测试浏览器 Worker：

```powershell
npm ci --prefix scripts
npm run audit:prod --prefix scripts
node --test scripts/tests/*.test.mjs
```

验证桌面前端和 Tauri 外壳：

```powershell
npm ci --prefix apps/desktop
npm run audit:prod --prefix apps/desktop
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml
```

## 运行时配置

`GATEWAY_STORAGE_MODE=local` 使用 SQLite，要求 `standalone`，不需要外部 Redis 或 PostgreSQL；桌面受管后端自动选用本地模式。`GATEWAY_STORAGE_MODE=server` 要求 `GATEWAY_REDIS_URL`，配置 `GATEWAY_DATABASE_URL` 或 `DATABASE_URL` 后启用依赖 PostgreSQL 的能力。官方 Compose 同时部署 Redis 和 PostgreSQL。开发环境可从 `.env.example` 创建不入库的 `.env`。数据目录见[本地数据存储](docs/local-data-storage.md)。
共享启动配置 `.ng/gateway.json` 可设置 `enable_management_ui`。该值与编译版本
是否包含管理 UI 能力共同决定最终行为；也可用绝对路径环境变量
`GATEWAY_CONFIG_FILE` 指定配置文件。详情见
[`docs/release-editions.md`](docs/release-editions.md)。

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

默认输出目录是当前独立仓内的 `release/Gateway/<versionId>`。Gateway 现在发布
三种版本形态，能力矩阵和配置开关见
[`docs/release-editions.md`](docs/release-editions.md)。每个版本目录不可覆盖。

在仓库根目录构建和打包：

每次发布前先执行[模型显示静态表更新守则](docs/model-display-catalog-release.md)：
查询官方模型目录，审核公司与模型的固定知名度排序，记录来源、日期及未归类模型，
在构建冻结源码之前更新静态表。运行时不联网调整排名，也不据此增加凭据权限。

```powershell
$id = "gateway-api-" + (Get-Date -Format "yyyyMMdd-HHmmss")
.\tools\build-gateway-release.ps1 -Edition api
.\tools\package-gateway-release.ps1 -Edition api -VersionId $id -SkipBuild -UseExampleRoutes
```

使用 `-Edition web` 可构建带浏览器管理后台但不带桌面启动器的版本；使用
`-Edition desktop` 可同时得到浏览器管理后台和 `gateway-ui.exe`。使用
`-Edition all` 会按 `api`、`web`、`desktop` 顺序串行构建三版。

`-UseExampleRoutes` 将 `routes.example.yaml` 作为包内两份路由文件的来源。
自动候选构建和标签发布始终使用此模式；不传该开关的本地打包仍保留开发者的
`routes.yaml`，对外分发前应使用示例模式。两种模式都不会改写源路由文件。

`build-gateway-release.ps1` 要求 Node.js `>=22.22.0`。API 版只审计浏览器
Worker 依赖，并使用 `--no-default-features --features runtime-defaults` 编译；
Web 和 Desktop 版还会安装、审计桌面依赖，执行 typecheck 和
`npm run build:web`，随后进行 Gateway 的 Cargo release 构建，并在该 Cargo 步骤里导出
`GATEWAY_PREBUILT_WEB_UI=1`，让根级 `build.rs` 直接复用已经生成好的 Web UI
产物，而不是再隐式触发一轮浏览器控制台构建。Desktop 版还会构建 Tauri
桌面壳。所有 release 构建都会强制单 job，并关闭 incremental compilation。

使用 `-SkipBuild` 时，打包器会要求对应版本的 provenance、能力标记和当前
源码树完全一致。省略 `-SkipBuild` 可以让打包器自行执行构建。正式包会排除
测试目录和生成的测试产物，但不会删除仓库中的测试源码。

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
npm run audit:prod --prefix ".\release\Gateway\$id\scripts"
node --test ".\release\Gateway\$id\scripts\tests\*.test.mjs"
```

完整提供方证据矩阵仅适用于源码仓库，因为它依赖 Cargo 源码与仓库验证工具。
可移植包会携带 `scripts/invoke-gateway-live-provider-canary.ps1`；默认可运行
安全的 dry-run 合同，只有显式传入 `-AllowLiveProviderCalls` 和所需输入后才
会调用真实提供方。详细说明见
[`docs/provider-evidence.md`](docs/provider-evidence.md)。

## Docker

配置 Compose、密钥、持久化和验证见[部署手册](deploy/README.md#quick-start)。已有数据库按[迁移说明](deploy/README.md#postgresql-and-the-gateway-schema)升级，不要删卷。

启动后的管理操作见[运维手册](docs/operations-manual.md)和[凭据测试策略](docs/credential-test-policies.md)。

## GitHub Actions

- `ci.yml`：Windows/Linux、Python、Node、Rust、桌面前端和 Tauri 验证。
- `build-windows.yml`：构建并上传 `release/Gateway` 下的 Windows 候选包。
- `docker.yml`：PR 只构建；只有 `main` 或 `Vx.y.z` tag push 才发布
  `ghcr.io/aiaimimi0920/gateway`，并会先用官方 Compose 栈做本地部署验证。
- `release-tag.yml`：只接受 `Vx.y.z`，校验后生成 ZIP、SHA-256，并发布
  GitHub Release；同时会附带 `Gateway-Vx.y.z-docker-deploy.zip` 与对应校验文件。

## 许可证

Gateway 使用 [MIT License](LICENSE)。
