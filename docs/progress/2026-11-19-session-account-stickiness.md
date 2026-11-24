# AG 账号轮询与会话账号黏性修复

## 运行取证与根因

只读确认 `127.0.0.1:4200` 为 Gateway PID 42720，EXE 来自
`../release/Gateway/gateway-devtest-20261118-pool-toolbar-v2`。
`192.168.15.20:4200` 是 Crow relay PID 11188，不是另一个 Gateway。
本轮读取实际 `.ng-devtest-4300` 日志和 SQLite，没有写入账号、密钥、配置或数据库。

最近 5,000 条审计的固定窗口是 UTC `2026-11-16T18:29:24.090143Z` 至
`2026-11-19T02:37:43.4759063Z`。其中 `custom-api-ag` 有 1,539 条，明确凭据的
账号 1／2／3 分别为 537／499／501 条；最近完成记录只见账号 1。
AG 在此处是 AgentRouter，不是 Antigravity OAuth。
近期审计 `sessionId` 为空，不能仅凭这些记录严格证明两个请求属于同一 session。

源码中可独立证实两个机制缺口：

1. HTTP 入站已经把 session 请求头校验后存入 `client_session_hints`，而智能路由
   只读 `canonical_req.explicit_session_key`。Responses 归一化只从 body.user 取得
   这一字段，常见的 header-only Codex 请求因此没有路由会话黏性。
2. `SmartRouting` 原本只记录服务商。即使 body 带 session，同一服务商的凭据池
   仍先轮询，后续排序又只比较服务商 ID，不能优先具体账号。

## 改了哪些内容

- 路由 session scope 接入已有的校验后 session 请求头，仍以认证后的租户、项目、
  access key、API key 和用户凭据限定作用域。session 别名一致；不采用 request ID。
- 原黏性 owner 增加具体凭据 ID。在发送预算准入后、实际发送开始处记账，非流式和
  流式共用此路径；不依赖 HTTP 200 或异步晚完成写入。
- 静态池先通过原模型、授权和可用性筛选解析记住的账号，再保留有界备用候选。
  即使池大于 6 个，合规的原账号也不会被队列截断遗漏；新 session 仍轮询。
- 智能排序优先合规的精确账号，再沿用原服务商黏性和可用性／智力／价格排序。
  既有失败释放及 request ID 并发围栏、发送预算、no-replay、冷却、手动禁用、授权
  和流式终态语义不变。沿用 4,096 条／一小时的进程内缓存，不增加持久化或迁移。

## 验证边界

初次旧源码回归编译完成，但新测试夹具的目录名前缀不符合既有清理 owner 的约定，
两个测试在清理断言失败；不能把这次失败声称为生产缺陷的先红证据。已修改测试
夹具使用 `gateway-local-` 前缀，不削弱既有清理边界。

定向回归覆盖 header-only 三账号、显式 session 八账号、租户／项目／key 隔离、
request ID 不形成会话、账号冷却回退、非流式和完整流式实际 loopback 发送，以及
手动停用后的切换与复用。最终命令和结果见本轮回执；未执行真实付费生成、
供应商缓存优惠账单验证、PostgreSQL／Redis live、多进程亲和或桌面人工验收。

最终结果：

| 检查 | 结果 |
| --- | --- |
| 新增路由回归（`--lib session_accounts`） | 4 passed |
| 同一已编译 lib runner 的关联回归 | 21 passed（候选池 2、发送门 15、session 头 3、延迟排序 1） |
| `pipeline_send_runtime_contract`，含新增 3 个实际 loopback 账号测试 | 63 passed |
| `smart_routing_contract` | 7 passed |
| effective-line checker 测试／最新源码 ratchet | 31 passed／passed |
| scoped rustfmt／UTF-8 无 BOM／`git diff --check` | passed |

Rust 命令使用 `--locked --offline --no-default-features --features runtime-defaults`
以及 `--config profile.test.package.gateway.debug=0 -j 2 -- --test-threads=1`；
路由回归增加 `--lib session_accounts`，运行合同增加
`--test smart_routing_contract --test pipeline_send_runtime_contract`。
关联回归复用本轮编译的 `neuro_gateway-b4ada4e6bb209310.exe`，没有为过滤词重新构建；
SHA-256 为 `964294a61ef56bdc3ab85257f348535dc187fca0ab7328c4b4e20d97598d55be`。
编译保留已有 warning，未以 `cargo fix` 扩大范围。合计 95 个 Rust 测试通过。

UTC `2026-11-19T03:09:28Z` 只读复核原 PID 42720 和原 EXE 路径不变，
`/healthz`、`/readyz` 均为 200；此结果不代替供应商业务或缓存优惠验收。

所有修改源码低于 500 有效行；未更新 baseline 或软上限例外。
证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-session-account-20261119`。
以上为源码修复阶段的记录：当时没有生成新包、替换 EXE、重启 4200、修改账号开关、
提交或推送。后续用户明确授权“编译新的包，并替换和重启”，部署结果如下。

## 后续编译与部署

通过 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4` 从当前真实工作树
生成新目录 `../release/Gateway/gateway-devtest-20261119-session-account-stickiness`。
类型为桌面 **devtest 手测包**，不是正式 Release；既有 unrelated dirty edits 保留并纳入
构建，没有提交、推送、升级依赖或使用未经检查的旧 EXE。使用已安装的 Node 22.22.2。

成功构建及打包耗时 540.98 秒；后端 Cargo 增量构建 4 分 45 秒，桌面 Cargo 10.21 秒，
不将这一单次记录当作冷／热构建速度对比。首次外层日志脚本因 PowerShell 将 native
warning 当成终止错误而中断；修正临时日志脚本后成功，不修改构建 profile 或源码。
源码指纹 `e6daeb8e77f5f0a4759bf58c5602e75aa540c1f9fba90b11d43694f2f615a47d`，
构建前后及打包来源检查、完整 manifest／checksum 检查通过。EXE SHA-256：

- `gateway.exe`：`6f381326ac9b8b4d1e4b3b79a61fe3ab8f7c8c9a8ce485fce87be8915007f473`。
- `gateway-ui.exe`：`d90b030d342d6d129508bd6ce77538f8ebfbc25d43f265df52a472cd72aea739`。

现有 `smoke-gateway-local-storage.ps1` 使用 linshi 内独立数据目录和随机端口，验证
SQLite 空路由启动、额度拒绝、重启持久化、失败退款、密钥轮换、无 Redis／PostgreSQL
连接及两个进程正常 drain。六项检查通过，真实模型调用为零；未把隔离检查写入线上数据。

独立切换 helper 在 UTC `2026-11-19T04:42:20Z` 请求旧后端 drain，退出码为 0，
随后关闭旧桌面。在所有 Gateway 退出后复制原数据，3,733 文件共 2,650,203,739 字节，
逐文件 SHA-256 一致，备份 SQLite `quick_check=ok`；备份 ACL 仅允许当前用户访问。
没有删除旧包或重写数据库，Crow relay PID 11188 未停止。

切换前最新配置实际为 AG 账号 2 启用、账号 1 和 3 停用，与早先调查时的状态不同；
本轮保留这个最新状态，不根据旧快照强制修改账号开关。新桌面 PID 8256、新后端
PID 32424，均来自新包；loopback 4200 listener 与后端路径／哈希一致，原数据根
仍为 `C:/Users/vmjcv/.ng-devtest-4300`。

首次切换后检查因对所有 SQLite payload 字段做完全相等比较而报失败；保留原始
`deploy-result.safe.json`。逐字段比较备份和在线数据库证实，差异仅为恢复流量更新的
`local_access_keys.payload.lastUsedAt` 与 `local_balance_accounts.payload.updatedAt`。
密钥 ID、token hash、sealed secret、权限、余额模式及数值、余额绑定均未变。
修正临时核验的活动时间戳判定，未为此再次重启或修改生产代码／数据库。

UTC `2026-11-19T04:47:07Z` 最终核验 passed：`/healthz`、`/readyz`、`/ui/` 均 200，
路由 revision `r514-1068abb9bd5e`、39 个服务商及 1,079 条模型路由、所有账号开关、
凭据、连接设置、storage-version 和 seal 文件均保持不变。修复源码哈希仍与前述
95 个 Rust 回归的最终回执一致；这一节是出包后补写的部署记录，不是额外源码修改。

恢复后的自然 AG 流量已有 completed 记录，仍使用账号 2；同时出现供应商
`rate_limit_exceeded`／`upstream_stream_error`，不等于启动失败或再次轮询。本轮没有
擅自启用其他账号，也没有为核验额外付费生成。启动日志另有既有 Antigravity OAuth
永久失败围栏及统计慢查询警告，不扩大为本次修复范围。

部署证据和仅切换程序版本、保留现有数据的回退脚本位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-session-account-deploy-20261119`：
`deployment-verification.safe.json` 为最终结论，`durable-state-diff.safe.json` 解释活动
时间戳差异，`data-backup` 是私有备份。旧包保留在原目录；`switch-package.ps1 -Rollback`
可请求正常 drain 后重新打开旧包，不自动恢复旧数据库。真实多账号在线黏性、缓存优惠
账单及桌面人工操作验收仍未执行；多账号行为的验证来自前述实际 loopback 回归。
