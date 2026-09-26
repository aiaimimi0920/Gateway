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

Gateway 现在自带一个本地默认的 Cargo 限流配置：
[`./.cargo/config.toml`](.cargo/config.toml) 中的 `build.jobs = 1`。这样日常
Rust 构建默认不会再轻易把整台工作站的 CPU / 内存打满。Release 构建脚本仍会
强制使用 `CARGO_BUILD_JOBS=1`、`CARGO_INCREMENTAL=0`，保证支持的发布路径保持
可复现；而源码挂载的开发容器现在改为默认使用
`GATEWAY_DEV_CARGO_BUILD_JOBS=4`、`GATEWAY_DEV_CARGO_INCREMENTAL=1`，
优先缩短本地迭代重编译时间，同时允许你按机器情况显式覆盖。

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

`build-gateway-release.ps1` 要求 Node.js `>=22.22.0`，会先安装并审计两棵
Node 生产依赖树，再显式执行一次 `npm run build:web`，随后进行 headless
Gateway 的 Cargo release 构建，并在该 Cargo 步骤里导出
`GATEWAY_PREBUILT_WEB_UI=1`，让根级 `build.rs` 直接复用已经生成好的 Web UI
产物，而不是再隐式触发一轮浏览器控制台构建。headless Gateway 和桌面 UI 的
Cargo 构建都会强制单 job，并关闭 incremental compilation。

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
npm run audit:prod --prefix ".\release\Gateway\$id\scripts"
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

根目录下的 `docker build` / `docker run` 仍然保留，适合快速复用 Docker 层缓存的
开发验证。没有传入审计 nonce 的 raw build 不能证明本次重新执行了依赖审计，
因为 BuildKit 可能直接复用之前的审计层：

```powershell
docker build -t gateway:local .
docker run --rm -p 4200:4200 --env-file .env gateway:local
```

需要做本地安全验证时，应传入每次都不同的 nonce，强制两棵生产依赖的镜像内
审计层在本次构建中重新执行：

```powershell
$auditNonce = [guid]::NewGuid().ToString("N")
docker build --build-arg "GATEWAY_AUDIT_NONCE=$auditNonce" -t gateway:local .
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

当前浏览器控制台除了原有的 route-config / revision 能力外，还新增了两个直接面向
账号池管理的工作区：

- `账号台账`：把 `routes.yaml` / route-config 文档里的 `provider.credentials[]` 视为
  可复用账号单元，并按 Provider / 服务商分组浏览；
- `分组策略`：把多个账号组织成逻辑分组，记录 `billing_multiplier` 等后续可供
  Platform 消费的费率元数据。

这些分组元数据保存在同一份 route-config 文档的顶层 `account_groups` 字段中，
因此它在当前 standalone / Redis 管理模式下无需 PostgreSQL 也能直接工作。

运行时路由默认保持向后兼容：

- **不带**账号分组选择器的请求，仍按原有路由逻辑运行；
- 可信内部调用方可在同时携带 `x-internal-api-key` 时，附带
  `x-neuro-account-group`（或 `x-account-group-id`）请求头，把 YAML/static
  路由候选限制在指定账号组内。

为了让 Platform 更容易消费计费倍率与账号池关系，Gateway 现在还提供了一个仅管理面可读的汇总接口：

```bash
curl http://127.0.0.1:4200/v1/internal/gateway/account-groups \
  -H "x-internal-api-key: 123456"
```

返回结果包含：

- `accountGroups[]`：已经折算好的 `billingMultiplier`、成员数量、启用状态、成员账号 ID；
- `accounts[]`：账号到 provider / group 的反向映射；
- `providers[]`：provider 到账号清单的汇总视图。

账号台账中的 `测试` 按钮调用独立的凭据连通性探测接口：

```text
POST /v1/internal/gateway/console/credentials/{credential_id}/probe
```

该接口只接受已登录的管理会话，并要求请求精确携带敏感信息确认接口返回的有效
`Secret Grant`。客户端必须通过 `x-secret-grant` 请求头发送该值；请求头缺失、Grant
过期、不存在或上下文不匹配时，后端统一返回 HTTP `403` 和
`console_secret_access_required`。Grant 会绑定管理密钥指纹、请求 Origin 和客户端
IP。探测直接使用 active route-config 编译后的凭据，不依赖 PostgreSQL；停用凭据
不会发起网络请求。返回状态只有：

- `passed`：已完成受支持的 HTTP 凭据探测；
- `failed`：上游请求失败，消息已清洗；
- `unsupported`：固定模型、浏览器态、状态型或未知适配器没有安全的无副作用探测。

响应格式为 `{ "result": { "credentialId", "providerId", "status", "message", "checkedAt" } }`，
不会返回 API Key、Cookie、Token 或上游响应正文。

如果要对本地源码做一次完整的 Docker 端到端验证，可运行：

```powershell
.\tools\verify-gateway-docker-stack.ps1 -BuildImage
```

`-BuildImage` 会在内部生成并传入唯一的 `GATEWAY_AUDIT_NONCE`，因此官方 wrapper
可以强制重新执行镜像内审计，同时不要求宿主机预装 Node.js 或 `node_modules`。

## GitHub Actions

- `ci.yml`：Windows/Linux、Python、Node、Rust、桌面前端和 Tauri 验证。
- `build-windows.yml`：构建并上传 `release/Gateway` 下的 Windows 候选包。
- `docker.yml`：PR 只构建；只有 `main` 或 `Vx.y.z` tag push 才发布
  `ghcr.io/aiaimimi0920/gateway`，并会先用官方 Compose 栈做本地部署验证。
- `release-tag.yml`：只接受 `Vx.y.z`，校验后生成 ZIP、SHA-256，并发布
  GitHub Release；同时会附带 `Gateway-Vx.y.z-docker-deploy.zip` 与对应校验文件。

## 许可证

Gateway 使用 [MIT License](LICENSE)。
