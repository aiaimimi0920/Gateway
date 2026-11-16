# 普通后台 worker 的关闭与错误出口

日期：2026-11-09（Asia/Shanghai）。当前状态：本地离线子闭环完成。此为总计划的生命周期子批，不是全部服务商或整卡完成。

## 本批解决的实际缺口

正常 runtime 丢弃了十一类普通 worker 的 `JoinHandle`。部分循环已有 shutdown，但 server 未等待其返回；另外 bind-host parse、listener bind、serve 的 `?` 会跳过原有关闭段。已复用原有 shutdown、Tokio handle 与 completion 机制，没有替代 OAuth、token-refresh、Accio 独立 owner。

- 新 `BackgroundTasks` 按固定 enum key 保存 handle，重复注册不覆盖未完成任务；注册关闭后不可重开。
- join waiter 串行，短锁内原位 poll 完成结果；等待者取消不取走或 abort handle，已收取失败继续保留。不把 `is_finished()` 等同本次 poll 必定 Ready。
- 静态 worker 名和 `panicked/cancelled` 是错误摘要，不使用可能携带秘密的 `JoinError::Display`。
- server 正常或错误退出都等待 cleanup；原 serving error 优先，background error 不跳过 `local.close()`。外层 FreeBuff cleanup 保留。
- local heartbeat 使用专属 `local_maintenance_stop`，只在 HTTP serve future 返回后停止，不能跟随可能提前到达的内部 global shutdown。先 join maintenance，再保持 finalizers → retire → pool close。
- 有持久化的循环只在空闲等待或下个目标前观察 stop，已经准入的刷新、发布、driver delivery 或同步继续收尾；Redis version GET 是纯读取，可以取消。

| 本次纳入 owner | 保留的职责边界 |
| --- | --- |
| network probe | Weak state；原 network mutex 内同时决定注册与 started/enabled，保留构造失败后可重试语义 |
| local maintenance | Weak state；HTTP 完成前仍可更新心跳；已开始的 maintenance 不 abort |
| credential refill notification | 下次需求创建前停止，已开始的 cycle/task 写入收尾 |
| credential probe scheduler | 下个 slot claim 前停止，原 probe/result writeback 完成 |
| credential cache version poller | 取消只读 Redis 等待，不更改原 TTL/version 语义 |
| folder sync | 原 control listener/native owner 和 admitted sync drain 保留；初始 sync 前补 stop 检查 |
| legacy ChatGPT Web refresh | 保留 rotation → persistence → lock release → result note |
| credential stock | 保留已开始的 event/published/cooldown 链 |
| pool automation | 保留 driver → archive → durable CAS → final state |
| site health | Weak state；原 started/wake 与探测取消语义保留 |
| provider discovery | Weak state；已开始的 save 不 abort，原有 completed slot 语义不改 |

## 证据与失败分类

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-background-lifecycle-20261109/`。

- `red-tests.log`：首轮 0 passed / 2 failed，原因是新 fixture 未创建路由文件；不是有效行为红测。仅补 fixture 文件，没有放宽生产读取。
- `red2-tests.log`：相同旧生产实现下 1 passed / 1 failed。bind failure 实际报 `bind error bypassed runtime shutdown`；正常 HTTP exit 对照通过。
- `production1.json`：初版接线的 API-only/runtime-defaults lib check 通过，93.5 秒。后续纯读取代码整理与测试加强后须再执行最终检查，不能将此旧输入回执冒充最终来源。
- `green-build.log`：加强的事务 fixture 使用 `raw_sql` 时暴露 `implementation of Executor is not general enough`，受 `Send` 准入约束；仅将 fixture 的两条 SQL 改为原事务内分别 `query().execute()`，没有修改生产语义或放宽提交断言。
- `green2-build.json`：最终 lib test harness 构建通过，719.954 秒，记录的编译输入前后无变化。`green.exe` 的 SHA256 为 `2e5717106a9c0860039770be8a3f96f7b6ac61dd5100b556c371f23867df61c4`；这是测试程序，不是 Gateway EXE/手测包。
- `focused.log`：宽 `runtime::` 过滤实际 155 passed / 0 failed / 4 ignored；其中本批核心二十项全部通过，包含十六项新增测试，不将新增数量再加到通过总数。
- `adjacent-launch-failure.json`：Windows 全测试名命令触发 `[WinError 206]`，相邻测试尚未启动。保留已经执行的结果，将未执行的精确名称分成最多 12000 字符的四批；没有重跑已经通过或 ignored 的项目。
- `adjacent-01..04.log`：依次 118/0、89/14、90/0、107/1（passed/ignored），没有失败。与宽 runtime 合同按完整名去重后，共 **559 passed / 0 failed / 19 ignored**。十八项需隔离 PostgreSQL/Redis，另一个站点录制 replay 需显式 fixture；ignored 不算通过。
- `production-final.json` / `production-accio-off.json`：最终 API-only/runtime-defaults 与只关闭 Accio 的两项生产 lib 检查通过，分别 80.329 / 76.329 秒；均未触发 management-ui 发布/清理 hook。
- `scope-format-final.json`：十九个 Rust 文件官方 formatter 通过。
- `checker.json`：官方 checker 31/31 通过；后续 checker 输入未变。
- `ratchet-final.json`：5206 文件检查通过，十四项 immutable runtime artifacts 单独分类，governed source >700=0。十九个 Rust scope 文件最大 446 有效行；继承的报告已保存并恢复原 bytes，未更改 baseline/exceptions。
- `whole-format.log`：全仓 formatter 仍只有未改动 `src/provider_runtime.rs` 的 `dim_model_probe` 模块排序失败；没有接管范围外文件，也不报全仓绿。
- `final-receipt.json` / `direct-scope.diff` / `passed-tests.json` / `skipped-tests.json`：编译、测试、最终生产检查与所选源码指纹一致；变化精确限制为十九个 Rust scope，UTF-8 无 BOM、继承报告原 bytes 与 `git diff --check` 通过。

已经运行的核心测试包含真实 local startup/bind failure、正常后台重复启动、已经开始的 SQLite 事务跨 shutdown 提交、错误优先级、heartbeat 的实际十秒 tick、旧循环首次准入/空闲关闭，以及 owner waiter 取消与错误保留。事务 marker 在关闭并重开 SQLite 后仍存在，旧 heartbeat owner 已退役；内部 global shutdown 后实际十秒 tick 仍更新心跳。不是一百二十秒的真实在途 HTTP 长 drain、独立 OS 进程或 native package 验收。

## 保留的未验证边界

1. 新 owner 的 join waiter cancellation-safe 不代表旧 OAuth/Accio/token-refresh join 或整个 runtime caller cancellation-safe。
2. join 不强制 abort 已准入写入，也不为其虚构统一硬截止时间；各单轮旧 I/O/持久化期限仍需按真实职责验证。
3. folder native shutdown 的旧十秒 deadline 超时后可由原生线程迟到释放；顶层 worker 返回不等于全部 native 资源已经销毁。
4. pool admission guard 的独立 Redis release task、旧 ChatGPT persistence 返回 `()` 且部分写错仅日志等边界未在本批修改。等待任务完成不能代替持久化成功证明。
5. 本批不新建容器，不重跑上一批真实 PostgreSQL/Redis 联合；上一批授权已消费。新增生命周期代码的真实 DB 长 drain、独立 OS 进程、强退及包仍待验。
6. 当前仍十七个新增模板，无新增整卡 `offline_ready`。统一 Key/bundle/prepaid、生产 schema-owner review/迁移、其余渠道业务/协议/媒体与真实模型额度、P50/P60/P70 和集中录制继续保留。
7. network 构造失败/竞争与 coop budget 耗尽的具体反例已按当前锁定 Tokio 源码核对并接线；测试证明 generic owner 完成后重启与 pending 保留，但没有故障注入真实 client 构造失败或强制耗尽 coop budget。

本批未读取历史或真实供应商凭据，未连接供应商，未改变现有数据库、容器、卷或运行实例，未提交、推送或出 Gateway 手测/正式包。临时 SQLite fixture 留在 linshi；lib test harness 不是可交付 Gateway 程序。
