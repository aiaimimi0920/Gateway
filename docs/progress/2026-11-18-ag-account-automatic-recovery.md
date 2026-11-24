# 4200 ag-account 错误与自动恢复修复

## 实际运行证据

本轮只读核对 `127.0.0.1:4200` 的 Gateway PID 25176，启动于北京时间
2026-11-18 15:45:18，运行包为
`../release/Gateway/gateway-devtest-20261117-revision-save-r3`。
`192.168.15.20:4200` 是 Crow relay PID 11188，转发到该 loopback 实例。
`/healthz` 和 `/readyz` 返回 200；不将此结果当作上游业务全部可用。

实际数据目录为 `C:/Users/vmjcv/.ng-devtest-4300`。`ag-account` 对应
`custom-api-ag` 的三个凭据账号、AgentRouter (`https://agentrouter.org`)，
不是 Antigravity OAuth。未改在线配置、账号启用状态、密钥、数据库或信任库。

只读 SQLite 最近 3000 条审计的固定窗口为 UTC
`2026-11-17T07:39:52.4867879Z` 至 `2026-11-18T09:52:56.6946443Z`：

| ag 状态／失败类型 | 数量 |
| --- | ---: |
| completed / failed / cancelled / running | 241 / 77 / 5 / 1 |
| 原生发送错误，delivery_unknown | 49（send 46、connection_reset 3） |
| 完整输出前流结束 | 15 |
| Responses rate_limited | 9 |
| 请求预算耗尽 | 2 |
| 上游 session binding 失效 | 1 |
| 上游 encrypted content 校验失败 | 1 |

同一窗口全网关另有 164 条授权候选不可用拒绝，trace 为 `candidateCount=0`、
`routeAttemptCount=0`、`authorized_candidates_unavailable`。它们没有发到上游；
单凭此记录不能反推出究竟哪个健康／启用／授权过滤器排除了候选。

旧日志 `1794952471`、`1794899237` 的 31 次出口探针全部 reachable=true；
通用出口可达不证明 AgentRouter 可达。本窗口没有证明所有历史故障都来自连接池，
也没有证明下面的网络恢复租约缺陷就是当时唯一根因。当前实例重启后仍出现过发送
失败，随后无需再次重启也有成功请求，必须保留这种证据边界。

## 改了哪些内容

1. `model_circuit::finish_at`：网络恢复 generation 已变化时，旧传输失败改为
   Neutral 完成。仍核对 circuit generation 和精确租约，只释放该请求自己的
   Trial 租约、保留 30 秒试探间隔；不重新追加旧网络处罚，不清全局健康。
   修复前回归实际运行 1 项并失败于 `completed trial retained its lease forever`。
2. 原生 HTTP pool：四类 transport 继续由入站连接惰性持有，改为可退休缓存。
   取 client 与 generation 原子完成，可信发送失败仅退休匹配 generation，
   供后续新请求重建；旧失败不能淘汰新池，旧 client／响应不被强制取消。
3. native sender 和 settings sender 接入上述退休；普通／恢复等待、session／
   非 session 的超时及重定向策略保持隔离。Cookie registry、认证与配置不被重置。
4. 新增 loopback 和逻辑时钟回归，保留连接 owner 释放、原生字节／头序、超时、
   session bootstrap、代理恢复、预算取消与 no-replay 的既有保护。

退休不是重试：`delivery_unknown` 不自动重放或切换账号。证书拒绝和代理认证错误
不触发退休。限流、流式输出失败和上游会话／加密拒绝不被伪装为传输恢复成功。
已知 pre-origin connect 错误继续沿用既有的预算内恢复，不新增另一套重试循环。

## 验证回执

命令使用 `--locked --offline --lib --no-default-features --features runtime-defaults`
及 `--config profile.test.package.gateway.debug=0 -j 2 -- --test-threads=1`。
资源评估为约 32 GiB 总内存、14 GiB 可用，Cargo 检查串行执行；没有升级依赖。

- `native_codex`：37 passed。
- `model_circuit`：35 passed，包含上述先红后绿租约回归。
- `native_session`：10 passed。
- `stage_rate_limit`：15 passed，包括代理重启恢复、持续失败次数、未知送达不重放、
  恢复期间取消、原截止时间不延长和账号不被网络故障处罚。
- `native_connect` 是错误的过滤词，实际 0 tests，不能算通过证据；对应有效覆盖
  已由 `stage_rate_limit` 中的 connect_tests / connect_proxy_tests 提供。
- 补上连接池随最后一个连接持有者释放的断言后，pool 最终重跑 3 passed；属于上述 37 项的重复验证，不另计为三个新测试。
- effective-line checker：31 passed；补上最终断言后复核 ratchet passed。修改后的源码均小于 500
  有效行；未更新 baseline 或例外。
- 续接时复核 scoped rustfmt、`git diff --check` 和新增文件 UTF-8 无 BOM 均通过；保留 unrelated dirty edits。
- 续接时只读确认 4200 仍指向旧包 `gateway-devtest-20261117-revision-save-r3`，两个健康端点返回 200；未执行上游业务验证。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-discovery-fix-20261118`。
本轮未生成 EXE／正式包，未部署或重启 4200，未运行真实供应商生成、真实断网、
PostgreSQL／Redis live 或桌面手测。完成的是源码恢复路径及本地定向验证，不能
据此宣称旧运行包已经应用修复或所有历史 outage 已被唯一归因。
