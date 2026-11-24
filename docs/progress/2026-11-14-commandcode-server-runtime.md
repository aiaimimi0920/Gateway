# Command Code 隔离 Server 验收

下述定向 Server 用例已通过，不是整卡完成回执。证据根为
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

## 环境与数据边界

用户已允许启动 Docker Desktop 和创建隔离测试容器；现有容器不修改，测试数据不删除。
PostgreSQL 使用 `gateway-first-three-pg-20261114`、`127.0.0.1:55147`，Redis 使用
`gateway-first-three-redis-20261114`、`127.0.0.1:56456`。健康检查均通过。

执行前确认隔离 `gateway_fixture` 数据库的 public 表数为零，随后通过 `psql` 的
`ON_ERROR_STOP=1` 成功应用仓库 initdb 脚本 `001`、`002`、`004`、`005`。
未应用 standalone 外键放宽脚本 `003`，保留 provider 审计外键。未使用真实账号或上游。

## 新增长期测试

`src/protocol/commandcode/wire_tests/server.rs` 新增 opt-in ignored 测试
`commandcode_server_db_two_states_native_send_audit_and_redis_slot`。

它通过管理 API 创建合成账号及凭据，使用两个独立 AppState 连接真实 PostgreSQL/Redis，
经公共 Messages router 和生产固定目标校验后，将网络发送限定到现有测试专用 loopback peer。
断言包括原生路径、精确 Key、模型、无重放、数据库完成审计/用量，以及 Redis 原子槽唯一胜者。
每次运行使用唯一 namespace/模型和独立证据目录，不删除数据库行或 Redis 键。

边界：这是进程内两个独立 AppState，不是两个 EXE；公共入站使用真实 router 的 oneshot，
不是 TCP listener。上游 peer、PostgreSQL 和 Redis 为真实网络连接。
该用例本身也不证明完整调度 worker 周期、全部原生端点、缓存失效或桌面交互已通过。

## 当前执行状态

定向 `rustfmt`、`git diff --check` 通过；checker 测试 31/31、effective-line ratchet 通过，
日志为 `commandcode-server-checker.log` / `commandcode-server-ratchet.log`。

首轮编译通过（30m29s），测试失败于第二条审计未完成。两次公共请求与原生发送断言均通过；
PostgreSQL 日志确认第二次合成响应重复使用 `native-msg`，违反
`gateway_request_audits_response_id_idx` 唯一约束。第一次审计 completed、5 tokens、1 attempt，
第二次仍为 running。修正夹具为每次生成独立响应 ID，没有放宽约束或修改生产逻辑。
修正后编译通过（33m02s），定向测试 **1 passed / 0 failed**（7.29s）：

```powershell
cargo test --locked --offline --lib -j 4 commandcode_server_db_two_states_native_send_audit_and_redis_slot -- --ignored --nocapture
```

首次日志：`commandcode-server-two-states.log`；重跑：`commandcode-server-two-states-v2.log`。
失败数据与日志保留。仅使用显式 `COMMANDCODE_TEST_DATABASE_URL`、
`COMMANDCODE_TEST_REDIS_URL` 和 `COMMANDCODE_TEST_EVIDENCE_ROOT`；前两者需指向上述专用服务。
未修改生产行为，未重打包或提交。

重跑日志显示测试通过，但外层 PowerShell 会话返回 1，尚未归因；没有将其写成 exit 0。
随后直接执行本次生成的 `target/debug/deps/neuro_gateway-6e27ac2b80ab1c06.exe`
（`--exact protocol::commandcode::wire_tests::server::commandcode_server_db_two_states_native_send_audit_and_redis_slot --ignored --nocapture`）
复核，**1 passed / 0 failed、3.01s、exit 0**。每次使用新 namespace，未覆盖旧数据库数据。
首个成功结果为 `commandcode-server-ca7c571dc75542dab23c547ad0098455/result.json`。
两个独立 AppState 均完成原生 Messages 发送和 PostgreSQL 审计（各 5 tokens、1 attempt）；
共享 Redis 槽并发竞争恰有一个胜者，后续申请拒绝。

## 当前 devtest 包的双 EXE 验收

一次性 `commandcode-packaged-server.ps1` 使用现有 `gateway-devtest-commandcode-20261114-closure1`
包启动两个真正独立的 `gateway.exe`，Server storage、独立目录/端口、共享专用 PostgreSQL/Redis。
`commandcode-packaged-server-v4/result.json` 为 **passed**：

- 两个 `/readyz` 均明确 `database=true`、`redis=true`、`redis_required=true`、`degraded=false`。
- 第一实例通过管理 API 创建 Command Code 账号、凭据、System One provider capability、
  platform access、bundle 和绑定该 bundle 的 `neuro` 格式现金 Key；第二实例读取相同账号 ID。
- 同一 Key 分别调用两个进程的 `/v1/systemone`，均为 HTTP 400 / `cash_usage_bound_unavailable`。
- 第一实例调用前与第二实例调用后的现金对象完全一致；两个进程均通过 drain 正常退出。
- manifest SHA-256 为 `63e740cb696bac623a10d352edb6f3eb8c60e4e50448290920f1c694ae14ba41`，
  未更改已有包。所有数据保留。

前三次夹具失败均保留：v1 错把 Local readiness 的 `storage_backend` 当作 Server 字段；
v2 沿用 Local 夹具的 `sk-gw` 前缀，没有得到可用 Server token，公共请求 401；v3 使用
`neuro` Key 但未绑定 Server access bundle，公共请求 429 / `access_candidates_exhausted`。
v4 按生产授权合同补齐配置后通过，没有放宽鉴权或访问表规则。

这证明双 EXE 的管理可见性、真实 Key 鉴权/授权、现金提前拒绝、共享余额和正常退出；
不证明包内成功上游生成或调度 worker 周期。合成生成与 Redis 槽的源码测试结果以上节回执为准。

## 自动调度周期补验（11 月 14 日历史记录）

2026-11-21 已完成重新执行，当前结果见 [Server 自动探测与审计复核](2026-11-21-commandcode-server-scheduler.md)。以下保留当时的执行状态。

新增 `src/protocol/commandcode/wire_tests/server_scheduler.rs` 的 opt-in 用例
`commandcode_server_scheduler_two_workers_real_ticks_and_restart`。复用 Server 配置夹具，
两个独立 AppState 运行未经修改的生产 worker 和真实 60 秒时钟；共享 namespace、计划和 Redis。
预期观察两个到期周期合计两次发送，而不是每周期由两个 worker 各发送一次；同时核对
PostgreSQL 完成审计、跨实例 Redis 结果、重建后的结果与仍有效的调度槽。

保留真实 provider 外键；DB 账号由管理 API 创建，调度目标仍来自生产 route document，
不把 DB provider 路由与 route-document 调度混为同一权威来源。所有数据保留。
当前 `cargo test --locked --offline --lib -j 4 commandcode_server_ -- --ignored --nocapture`
正在执行，同时回归前述 Server 发送用例；日志 `commandcode-server-scheduler.log`，
进程结束后另存 Cargo 原始退出码至 `commandcode-server-scheduler.exit.txt`。
新增用例尚未取得通过回执，不提前关闭周期验收。
