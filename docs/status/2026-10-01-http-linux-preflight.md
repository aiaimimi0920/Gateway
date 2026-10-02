# 2026-10-01 HTTPS 后续边界与 Linux 构建预检

## 本轮结论

本轮没有新增“Linux 已通过”或“打包 EXE 正向 HTTPS/H2 已通过”的结论。
已完成的是可信 HTTPS 上游条件核查，以及当前源码的隔离 Docker 构建预检。
上轮实际通过的 12 项 TLS/H2 与 1 项 EXE 拒绝合同仍见
[TLS/H2 检查点](2026-10-01-http-tls-h2-closure.md)。

## 为什么没有强行完成 EXE 正向 HTTPS/H2

当前根 manifest 为 wreq 启用 `webpki-roots`。固定版本 `wreq 0.16.1` 的
`src/tls/conn/ext.rs:45-65` 在未指定 `CertStore` 时直接加载内置 Mozilla CA；
只有未启用该 feature 的分支才调用默认路径加载器。

Gateway 自身没有自定义 `CertStore` 或 CA 文件配置接线。因此不能仅设置
`SSL_CERT_FILE`，就假设已交付 EXE 会信任本地合成 CA。上轮 probe 通过公开 API
设置自己的证书根，不是当前 EXE 的配置能力。

后续正向业务闭环需要已授权、具有可信证书的可控 HTTPS/H2 上游。已询问用户
是否提供 URL 和凭据文件位置；本检查点未收到该输入。没有为测试关闭验证、
改 Windows 信任库、修改 hosts、增加产品配置功能，或用普通公网连通代替业务验收。

## Linux/Docker 实际盘点

- Docker Engine `29.5.3` 可用，VM 报告总内存 `8329347072` 字节、12 CPU。
- `.wslconfig` 仍为 memory 8 GB、swap 2 GB，没有调整或重启。
- `gatewaydev0727-gateway-1` 内已有工具链为 Rust `1.91.1`，新源码要求 `1.98.0`。
  只运行了 `rustup toolchain list`，没有在活动挂载工作区执行 Cargo 或安装工具链。
- 开发镜像 `neuro-gateway-dev:local` 创建于 2026-09-01；不能把它当作本次迁移的新镜像。
- 采样显示现有容器工作集已占用 VM 内存的较大部分，约 5 GB。该数值不是主机空闲内存，
  也不能证明一次完整 release/LTO 编译一定失败；本轮只是没有贸然启动它挤占现有服务。
- 当前 Gateway、PostgreSQL、Redis 三个容器都处于运行状态；4200 的 healthz/readyz
  在检查后均返回 200。它们不是本次新 Linux 候选的验收对象。

## 安全的构建上下文

从上一 Windows 交付的普通源码快照中选取官方 Dockerfile 所需文件，复制到：

```text
C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-linux-candidate-20261001\context
```

共 2,418 个文件、16,200,478 字节。每个输入在复制时与当前工作源码逐 hash 比较，
并在 Dockerfile check 后再次核对上下文未变。

没有复制真实 `routes.yaml`、`deploy/.env`、`deploy/gateway_data`、`.ng`、
node_modules、target 或 dist。正式 Dockerfile 本身使用 `routes.example.yaml`
作为镜像模板，没有用空配置替换真实工作配置。

旧 `tools/verify-gateway-docker-stack.ps1` 会临时覆写其所在仓库的 `deploy/.env`，
并在 finally 中执行该 Compose project 的 `down -v`。当前 checkout 已被活动开发
容器挂载，因此没有直接执行此脚本，也没有在活动 checkout 上开展 build/run 验收。
以后若复用该脚本，也必须先解决数据与配置的隔离范围，不能仅凭 finally 恢复来规避
运行中配置被临时改变的风险。

## 已执行的 Docker 门禁

对上述冻结上下文运行：

```powershell
docker buildx build --check --progress=plain -f <context>\Dockerfile <context>
```

耗时 7.2 秒，退出 0，实际输出：

```text
Check complete, no warnings found.
```

这是 Dockerfile 检查，**没有执行 RUN，没有完成 Rust/Linux 编译，也没有产生新镜像**。
检查期间成功解析 Rust 与 Node 基础镜像元数据。

另一次本轮成功的联网 `docker manifest inspect rust:1.98.0-bookworm` 确认
linux/amd64 manifest 为：

```text
sha256:4e4a7e7939c17991ab35f2b8c2e67593980f771d28f6b1254b1850f860fd0c7f
```

随后一次冗余 manifest 查询超过 30 秒，保留为失败，不冒充成功或重跑已通过的
Dockerfile check。最终收尾复用本轮先前成功查询及 check 回执。

Docker 官方 container-driver 文档也已联网读取，确认有 memory、memory-swap、
cpu-quota 等独立 builder 资源限制选项；没有创建 builder、更改默认 builder、
拉起额外构建容器或擅自提高 WSL 资源。

## 保全与后续条件

- 82 个既有容器的 ID、名称和镜像保持一致；Gateway 三容器的状态、启动时间、
  restart count 均未变化。`deploy/.env` 和 `.wslconfig` 保持原 hash。
- 全局“所有动态状态完全不变”检查最初失败：`easy-email-localproof003` 在本轮开始
  就处于 restarting，restart count 2476；检查期间其既有循环继续增加。
  原始失败保留。最终区分容器身份、Gateway 状态和其他项目动态状态，未对该容器执行操作。
- 两个新临时脚本由仓库官方 lexer 测得 59、71 有效行；Python 语法与 UTF-8 无 BOM
  检查通过。本轮没有改生产代码或依赖，不重复上轮完整矩阵。
- 完整 Linux release 编译和运行仍未开始。下一步需要选择不会干扰现有服务的构建资源
  方案或时间窗口；这不自动授权停止其他容器、修改 WSL 或重启 Docker。

证据目录：`linshi/gateway-linux-candidate-20261001`。关键文件包括
`context-manifest.json`、`dockerfile-check.log`、`dockerfile-check-receipt.json`、
`rust-image-amd64-observation.json`、`preservation-before.json`、`preservation-after.json`、
`preflight-result.json`、`source-preservation.json`、`effective-lines.json`。

未提交、推送、部署、覆盖 Windows 候选、安装信任根、停止既有服务或删除 `linshi` 外内容。
