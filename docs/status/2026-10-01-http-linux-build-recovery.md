# 2026-10-01 Linux 候选构建失败与运行服务恢复

接续会话 `01a0f724-0591-7dd0-aaba-df6738b8a597` 的最后任务：
构建新的 Linux/Docker 候选，并在隔离环境验收公开 API。前置范围见
[Linux 预检](2026-10-01-http-linux-preflight.md)。

**Linux 候选仍未完成，没有新镜像或 Linux runtime 通过结论。已完成失败诊断、
本次构建客户端清理、专用 builder 停止，以及原有 Gateway 服务恢复核验。**

## 恢复的真实停点

上轮 `build-r1/result.json` 已记录失败，不是仍在构建：

- 官方 `cargo build --locked --release --bin gateway` 编译 Gateway 主库失败。
- 日志含 `SIGKILL`、`cannot allocate memory`；builder 状态为
  `OOMKilled=true`、`ExitCode=137`。
- Docker VM 日志在 `2026-10-01T15:11:37Z` 记录 `CONSTRAINT_MEMCG`，
  owner 是该 builder 的 cgroup，被杀进程是 `rustc`。
- 上轮失败回执、日志和缓存保留，没有改成成功或覆盖。

## 本轮有限重试与失败

2,418 个冻结输入逐 hash 匹配当前源码。没有在活动 checkout 上运行 Cargo，
没有复制真实 routes、`.env`、`.ng` 或数据库。

只复用并调整本次已有专用 builder：内存从 2,560 MiB 调到 3,584 MiB，
memory-swap 总额从 3,584 MiB 调到 4,096 MiB。CPU 仍限制为 2，
BuildKit max-parallelism 与 Cargo jobs 均为 1。
官方 Dockerfile 和 `release` 的 thin LTO、opt-level 3、codegen-units 1 未改。

重试于 `2026-10-01T16:09:40Z` 开始，未完成主库编译。
共得到 17 次 VM 内存采样；最后一次在 `16:12:31Z`，
`MemAvailable=1049396 kB`，`SwapFree=0`。
保护阈值为 384 MiB，但采样调用随后失去响应，不能宣称保护器已保证服务不受影响。

Docker API 随后返回 500，4200 health/readiness 曾超时；第一次停止请求也失败。
这证明本次构建窗口干扰了服务响应，不能把最终身份保全说成全程无干扰。
尚无证据确认 r2 是再次 OOM；最终状态 `OOMKilled=false`，它是主动停止的。

## 清理与恢复证据

- 核对命令行后，只终止本次 Windows 构建和内存探测客户端，没有按进程名批量清理。
- `16:25:05Z` 向同一 builder 发出的独立 `docker stop --timeout 2` 请求成功。
- 最终 builder 为 `Running=false`、`Pid=0`，无本次构建客户端残留。
- `neuro-gateway:wreq-20261001-linux-candidate-r2` 的 image 查询为空。
- `16:26:10Z` 起，两次相隔 3 秒的 `/healthz`、`/readyz` 均为 200。
  Gateway、PostgreSQL、Redis 的 ID、镜像、启动时间和 restart count 与重试前相同。
- 全部既有容器身份保持；其他项目本来就在 restarting 的动态状态不宣称不变。
- 当前源码、冻结上下文、`deploy/.env`、`.wslconfig` 的 hash 保持。

没有重启 Docker/WSL、停止其他容器、修改 WSL 资源、部署新 Gateway、
覆盖 Windows 候选、提交、推送或删除 `linshi` 外内容。
过程中曾询问 Docker 重启许可；自行恢复并停止 builder 后已说明当前不需要重启，
没有执行该动作。

## 临时验证脚本修复

失败暴露了 Windows 的 RTK 子进程管道问题：`check_output(timeout=30)` 杀掉
RTK 包装进程后，仍运行的 Docker 孙进程可持有输出管道，导致父脚本继续等待。
这次实际挂住的原日志和 traceback 保留。

只修复 `linshi/gateway-linux-candidate-20261001` 内的验证脚本：

- 新增 `bounded_command.py`：文件接收输出，超时只结束自己创建的进程树，
  不调用可能继续等待继承管道的 `communicate`。
- Docker 查询、构建命令和 retry 的取消路径接入上述 owner 清理。
- 两项回归通过：正常输出；RTK 父进程与继承输出的 Python 孙进程超时退出。
- Linux runtime 脚本对齐 r2，并要求构建、来源和保全门禁全通过后才允许运行。
  它本轮没有执行；准备好的 19 项断言不算 Linux 已通过。
- Python 语法、Node 语法、UTF-8 无 BOM、仓库 lexer 测量通过。
  新增/修改脚本均低于 250 有效行，无大小例外。

本轮没有生产代码或依赖变化，不重跑先前源码对应的整套 Rust、UI 或 Windows 发布门禁。

## 下一步边界

不再在相同负载下盲目提高 builder 内存或重复 release 编译。
完成官方 Linux 候选需要有足够余量的独立构建环境，或用户明确批准的资源/时间窗口。
暂停其他服务、调整 WSL 并重启、触发远程构建都需要各自明确授权。
不得关闭 TLS 验证、降低正式 release 优化后仍声称同等官方构建，或将旧镜像冒充新候选。

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-linux-candidate-20261001`。
关键文件为 `build-r1/result.json`、`build-r2/build-receipt.json`、
`build-r2/memory-samples.json`、`build-r2/result.json`、
`build-r2/bounded-stop-receipt.json`、`build-r2/recovery-final.json` 和
`build-r2/harness-effective-lines-final.json`。
