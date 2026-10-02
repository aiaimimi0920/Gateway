# 2026-10-01 PC2 Linux 候选：源码已冻结，镜像层下载仍未闭环

接续会话 `01a0f724-0591-7dd0-aaba-df6738b8a597` 的最后任务：
构建官方 Linux/Docker release 候选，并执行隔离的 19 项公开 API 验收。
用户本轮允许考虑 PC2 Linux 环境；连接入口为 `aikey/部署/主机连接.md`。
本机上一次构建失败及恢复见
[Linux 构建恢复](2026-10-01-http-linux-build-recovery.md)。

后续已正常提交并推送源码；GitHub-hosted 构建又暴露真实的 npm/OSV 安全门禁。
浏览器 TLS 依赖的修复和剩余 GTK 链审批边界见
[2026-10-02 安全构建检查点](2026-10-02-browser-tls-security-closure.md)。
该检查点没有改变下面冻结快照或 PC2 失败回执，不代表候选已构建完成。

**首轮完成了向 PC2 传输并校验冻结源码，以及一次隔离构建的预检和失败收尾。
BuildKit 基础镜像拉取超时，Rust 编译没有开始；没有新候选镜像，19 项运行验收未执行。**

**最新停点（2026-10-02 01:09 UTC）：认证代理及 immutable manifest 已验证，
但镜像层下载仍未闭环。r4 已保留部分流快照并主动取消；进程和 loopback 桥均已关闭。
下面保留首轮原失败记录，最新续接证据见文末。**

## 输入及连接边界

- PC2 为 `mjc@192.168.15.104`，Docker `26.1.5+dfsg1`，4 CPU、约 7.6 GiB RAM。
- 使用已有 Ed25519 密钥及 `known_hosts` 身份校验。NAS 的密码仅在本机读取，
  没有输出密码、复制私钥或把凭据文件放入源码及 PC2。
- 2,418 个冻结构建文件与当前源码逐 hash 一致。官方 Dockerfile、锁文件、
  Rust `1.98.0` 和 release 优化保持；没有复制真实 routes、`.env`、`.ng` 或数据库。
- `context-manifest.json` SHA-256：
  `1112adf7580cf461cd365362dcc84cd010e224989f89e6fb3b1b1e1819e195e9`。
- 源码与初版 harness 的归档为 3,401,560 字节，SHA-256：
  `126e364b1d9b0a369578565164d69616740c746fb9c0e7ef3602e033ab193d13`。
- PC2 独立 owner 目录：`/home/mjc/linshi/gateway-wreq-20261001-r1`。

## 传输的实际结果

普通 SCP/SFTP、SSH stdin 和大 exec 参数均曾出现 reset、broken pipe 或停滞。
关闭 stdin、增加 `ssh -n` 后，串行 16 KiB exec 分块校验到 57 块，随后失败。
旧的块、坏文件和原回执都保留，没有拿局部 ACK 代替完整归档校验。

单文件临时端点也有失败：Windows 端记录完整发送，但 PC2 未在预算内校验完成；
NAS 普通传输只留下部分文件。NAS SSH TCP 转发被现有策略拒绝，未修改该策略。
另一次利用 PC2 已有 CIFS 挂载的有界归档读取也超时；该进程随后已退出。

最终仅在 NAS 本次临时 HTTP socket 上限制 MSS 为 512，使用有界发送和 Range
续传。r3 留下 1,519,292 字节；r4 先复制该部分文件，保留 r3，再续传缺失数据。
PC2 的完整归档 SHA-256 与上述冻结归档一致，`nas-transfer-r4-receipt.json`
记录 `status=verified`。端点只允许 PC2 的随机单路径，不提供目录浏览。
各临时监听均已关闭，未修改主机 MTU、网卡 offload、SSH、VPN 或防火墙配置。
此结果证明传输闭环，不足以确认导致 TCP 停滞的具体系统或链路原因。

在解包前检查了全部 2,424 个归档成员：仅常规文件、无绝对路径、无 `..`、
无反斜杠、无重复成员，展开总量 18,648,070 字节。PC2 再次校验整包 SHA，
使用 `tar --keep-old-files --no-same-owner`，防止覆盖既有文件。
构建脚本又校验了全部 2,418 个源码文件及精确文件集合。

中转文件保留于本地 `linshi`、NAS `project/AI/GameEditor/linshi`，以及已有
FPFData 挂载下新建的 `linshi/gateway-pc2-20261001-r1` 和
`linshi/gateway-pc2-20261001-r2-harness`。没有覆盖采集数据、创建新网络盘映射
或新 CIFS 挂载。中转文件未自动删除。

## 预检修复及构建失败

第一次预检发现 PC2 有 270 个容器记录，其中 16 个运行中；原脚本的 128 个
记录上限触发失败。该次尚未创建 builder，原日志保留。

只修改临时保全 owner：最多 512 个记录，每批最多查询 16 个，仍限制每个 Docker
响应为 1 MiB，既有 Config/HostConfig 只保存 hash，不保存凭据。
新增 270 个历史记录的分批查询回归。5 项保全测试全部通过，远端也运行了同一测试模块。

第二次构建的实际窗口为 `2026-10-01T17:59:55Z` 至 `18:04:57Z`：

- builder 元数据：`gateway-wreq-pc2-20261001-r2`。
- 预定镜像：`neuro-gateway:wreq-20261001-pc2-linux-candidate-r2`。
- 配置参数为 memory `3584m`、memory-swap 总额 `6144m`、CPU 2，
  BuildKit parallelism 1、Cargo jobs 1。builder 容器未创建，不能声称实际 cgroup
  限额已验证，也没有 release 编译的内存采样或 OOM 结论。
- `buildx create` 成功；`inspect --bootstrap` 停在
  `pulling image moby/buildkit:buildx-stable-1`，300 秒预算到达后结束自己的客户端。
- 本次 `buildx stop` 返回 0。原 result 中随后出现 `builder_stop_error`，原因是
  查询尚不存在的 BuildKit 容器；原失败回执未改写成成功。
- 独立收尾查询确认没有本次 BuildKit 容器、候选镜像或构建 supervisor。
- 从 PC2 对 `https://registry-1.docker.io/v2/` 的 IPv4 连接检查也在 5 秒超时：
  `curl: (28) Connection timed out after 5001 milliseconds`。
  这是该端点的当前连接失败，不宣称所有公网端点都不可用或已找到网络根因。

## 保全、验证与剩余工作

`build-r2/result.json` 及前后 snapshot 实际记录：

- `preservation.identities_and_configuration=true`；
- `dynamic_changes=[]`，本次窗口内 270 个既有容器的记录保持；
- `context_unchanged=true`；
- `status=failed`、`linux_runtime_verified=false`。

没有停止、替换或重新部署既有业务容器，没有重启 Docker/WSL、修改网络配置、
降低正式优化、覆盖 Windows release、提交或推送。

Windows 非交互命令 helper 增加 DEVNULL stdin，并避免目标已退出时的 taskkill
非零返回掩盖原超时。3 项聚焦测试通过：正常输出、关闭 stdin、超时清理自己的
RTK/Python 进程树。保全 owner 的 5 项回归通过。临时 Python 语法和 UTF-8
无 BOM 检查通过；官方 lexer 测得本轮 harness 为 17–160 有效行，无大小例外。
没有生产代码或依赖修改，不把这些临时检查当作 Linux 编译或 API 运行验收。

收尾再次核对本机当前源码的全部 2,418 个冻结输入，`changed_paths=[]`。
Neuro 根仓库和独立 Gateway 的 `git diff --check` 均通过；两仓库原有 dirty
worktree 保留，本轮仓库内仅新增本状态文档。既有 Windows 4200 服务的
`/healthz`、`/readyz` 再次实测均为 200；没有把旧服务作为新候选的运行证明。

下一步需要确认 PC2 当前可用的代理或镜像仓库，再按明确配置边界恢复 BuildKit。
没有对相同网络路径重复启动构建，也没有未经确认修改 Docker daemon 配置。
构建成功后仍须执行原样复用的 19 项公开 API 断言，核验实际限额、保全及清理。

本地证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-pc2-linux-20261001`。
主要证据为 `nas-transfer-r4-receipt.json`、`short-launch-r1-receipt.json`、
`harness-effective-lines-final.json`、`pc2-final-check-0.log`、
`pc2-final-check-1.log` 和 `pc2-build-evidence-r2/` 内的原始失败回执与日志。

## 最新续接：认证路径与工具验证

PC2 Docker daemon 当前没有 HTTP/HTTPS 代理；已有 mirror 为
`https://docker.m.daocloud.io`，没有修改它或 systemd 环境。
现有 EasyProxy 位于 `192.168.15.201:22323`，匿名访问返回 407。
只在内存读取 PC2 已部署容器的 canonical 认证，HTTPS CONNECT 验证官方
Registry `/v2/` 返回正常的 401 challenge，外部证书验证始终开启。
这证明认证路线可达，不是完整镜像拉取证明。

主机没有 Skopeo，未使用 `apt install`。根据 Debian 已缓存 APT 元数据，
在本次临时 owner 下载、续传、校验并解包官方包：

- `skopeo version 1.18.0`，APT `1.18.0+ds1-1+b5`。
- 包 7,556,560 字节，SHA-256：
  `7ca44a3f31f3cddca9d9dc858029dd08c30faf6e93456753825509e6a59192e2`。
- 原下载 300 秒只取得 6,553,600 字节；原失败与部分文件保留。
  续传到新文件并完成整包 SHA 校验，不把原失败回执改写为成功。

本次 loopback 桥仅将内存认证注入现有上游代理，builder 只接收无凭据的
`127.0.0.1` URL；HTTPS 由客户端验证，不做 MITM。
目的地、连接数、header、buffer、idle 和总连接时间均有边界。
本轮还修复了桥退出时未登记的 header/connect 阶段 socket：先登记资源、
检查 stop event，再执行阻塞操作。新增 partial-header 退出回归。

## r3 EOF 与 r4 路线对照

r3 在 `2026-10-02T00:27:43Z` 至 `00:28:24Z` 取得 index，但 child manifest
请求返回 `EOF`；没有进入 blob copy、Docker load 或 Rust 构建。
原始失败保留于 `preload-r3/`。

对同一个 linux/amd64 immutable manifest 做了有界单次路线对照：

```text
sha256:98cc6a3fc46220d00f8224ae483f3274fc874e9be8d7dd1e2e2c5481209228b5
```

管理 API 的只读查询当前返回 43 个可用节点。根据本次查询选出
`tw-ipv6-p1-2`，没有套用旧节点健康记录、变更共享 pool 或使用业务 lease。
canonical 与请求级 `pin-strict`/`nosplit` 路线均取得 2,261 字节 manifest，
SHA 与上述 digest 完全一致，分别约 5.57 秒与 5.47 秒；桥异常列表均为空。
因此 r3 EOF 没有在此次小请求上复现，具体根因仍未确认。

## 大流量对照与镜像站边界

同一 strict 节点、同一公开 layer 的前 1 MiB 对照：

| 每连接设置 | 实际结果 |
| --- | --- |
| MSS 512、接收窗口默认 | HTTP 206，1,048,576 字节，43.55 秒 |
| MSS 512、SO_RCVBUF 8192 | HTTP 206，1,048,576 字节，49.13 秒 |
| MSS 默认、接收窗口默认 | 49,152 字节后 TimeoutError，17.54 秒 |

两次完整 prefix 的 SHA 相同：
`ba58bacdfbd707aadeba0f2a30299ff7ca0bf8ca8702610cbc683942199d10bb`。
缩小窗口没有证明性能改善，不能据此调整全局网络。
这些均是本次 socket 设置，不是网卡、路由、DNS 或 sysctl 修改。

已存在的 DaoCloud mirror 可在约 1.74 秒返回相同 manifest digest，
但 Skopeo 的有界 layer copy 随后在 Docker CloudFront 的 IPv6 连接上
发生 `connection reset by peer`，并未得到可验证的完整镜像。
原始日志可能包含 Registry 临时签名 URL，只保留在隔离 owner，
本地证据摘要仅下载结构化结果，不复制这份原始错误日志。

补充的 IPv4 辅助验证也没有完成 prefix 下载。需要区分辅助脚本问题：
镜像站的 401 challenge 只提供 realm/service，不提供 scope；使用官方
Registry token 的尝试不适用于镜像站 service，初版脚本也曾因缺失 scope
发生 KeyError。根据真实 challenge 修正后，独立控制请求成功取得匿名
token，token 仅留内存；最后一轮仍遇到 TimeoutError。
不能把所有这些辅助失败都归因于 IPv4 网络，或宣称已找到 GRO 根因。

## r4 取消、保全与当前验收状态

r4 预加载窗口为 `2026-10-02T00:46:32Z` 至 `01:09:04Z`。
在持续缓慢的 layer 下载阶段，核对 parent PID、cmdline、cwd、child PPid
及本次路径后，先保存其临时流的前缀快照，再发送 SIGTERM 取消自己的
supervisor。没有等待它占满预算，也没有重启服务或重复完整构建。

`preload-r4/cancel-receipt.json` 实际记录：

- partial snapshot 24,117,248 字节，SHA-256：
  `1ebecddc796a683661cdf7eb8e20cb33b303faee3ed75c2d37708b84af513a40`。
  明确 `complete_blob_claimed=false`，不是完整 layer 或候选镜像证明。
- `parent_stopped=true`、`child_stopped=true`。
- 本次 listener 端口 49819，独立连接检查确认关闭。
- 原始临时流没有删除，已有部分 archive、失败回执与旧脚本备份保留。
- r4 result 保持 `status=failed`，错误来自主动取消，不冒称下载成功。
- `preservation.identities_and_configuration=true`、`dynamic_changes=[]`。

独立收尾查询没有 r4 BuildKit alias、候选镜像、`build-r4/` 或 `runtime-r4/`。
因此 **Rust release 构建仍未开始，19 项公开 API 运行验收仍未执行**。
临时 `pc2_runtime_r4.py` 仅准备了新镜像/结果路径，原 `exercise()` 未改，
没有运行或把静态准备算作验收通过。

本轮最低验证：本地代理/保全测试 10/10，PC2 代理测试 5/5；42 个临时
Python 文件语法及 UTF-8 无 BOM 检查通过，官方 lexer 测得本轮新增/调整的
临时 owner 均不超过 160 有效行；桥 owner 由 95 增至 111 行，新增资源退出边界。
不把这些结果当作官方 Linux build 或产品 API 验收。
收尾再次比较当前 Gateway 的全部 2,418 个冻结输入，`changed_paths=[]`。
PC2 也重新校验相同 2,418 个文件的精确集合及 hash，完全匹配冻结 manifest。
所有本轮已登记诊断 PID 的独立查询均已退出。Neuro 根仓库与独立 Gateway
的 `git diff --check` 均通过，已有 dirty worktree 和行尾告警未擅自清理。
已有 Windows 4200 的 healthz/readyz 都为 200，但不是新 Linux 候选证明。

本地结构化证据新增于
`linshi/gateway-pc2-linux-20261001/pc2-network-evidence-r3-r10/`。
未修改生产代码、依赖、Dockerfile、release 优化、共享代理、网卡参数、
路由、SSH、防火墙或 Docker daemon；没有提交、推送或清理既有缓存。

下一步是解除大流量路径限制，不是继续重复慢路径构建。
临时关闭 PC2 `enp2s0` GRO、最多 60 秒对照后恢复的请求仍未得到明确同意，
因此未执行。即使对照通过，仍需完整 BuildKit digest/config 验证、官方
Dockerfile release 构建及原 19 项运行断言，才能称 Linux 候选完成。
