# 服务商分渠道扩展：第 29 批，Server 刷新权威持久化

> 2026-11-03；R05/W03/P20，继续P28。正常可写Server双阶段、v3有界等待与共享恢复owner已获定向离线证据；P29继续执行，不是完整Server生命周期、渠道或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；不提交、推送、打包或替换运行实例。

## 1. 当前拥有范围和完整退出条件

本轮主代理拥有`src/token_refresh.rs`、`src/token_refresh/persistence.rs`和对应合成测试；子代理只读核对权限、存储合同和测试设施。证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p29`。

P29完整范围不是“cache写成功”或“配置保存成功”二选一：

1. 正常可写Server使用既有`RouteConfigRuntime`持久owner推进rotation/context/绝对expiry，cache作为派生写回。
2. 配置成功/cache故障时保留同一pending和single-flight；重试不再grant、不增加重复revision、不从新document反推旧cache CAS材料。
3. 使用全新的store/runtime及旧bootstrap进行恢复，校验新token/context/expiry/lifetime与权威revision，并证明重复恢复。
4. Worker保留正常管理和非refresh automation的只读边界；若授予能力，只能是有约束的refresh-only提交，不能开放现有通用`commit_automation_document`。
5. 在发送grant前实现跨实例所有权；active revision与完整授权材料必须来自同一pinned snapshot。写回CAS、进程内锁和普通到期lease都不是一次性token的完整防重发保证。
6. timeout、caller drop、缺expiry/校验失败、进程崩溃等上游可能已消费token的结果不明窗口不能自动解除发送栅栏。编辑context而refresh token未换，不足以证明是可并发的新授权。
7. 配置已提交但cache/cleanup尚未完成就退出时，重启须能修复派生状态；无rotation回包、删除重加、迟到旧owner及多实例竞争分别验证。

当前先实现1–3的必要前置；4–7仍为本计划明确未完成项，不把前置阶段改名成完整P29或通过禁用Worker刷新替代它们。原R01–R16、Vertex Express/Luma Agents、媒体、P50/P60/P70全部保留。

## 2. 已确认的责任边界

原scheduler仅在`local_runtime.is_some()`时传配置runtime；Server传None只写credential cache。另一方面，`PendingRefresh::write_to`只要配置persist成功就立即返回，不会继续cache。故单改scheduler参数会丢派生写回，且cache故障时提前释放pending。

Worker的`coordinator()`为None；管理写和现有通用automation均依赖它。后者还服务池维护、补货和发现，不可为refresh直接一并放开。本前置阶段保持Worker旧能力与行为不变，不冒称已修其Server重启/并发缺口。

跨实例方案审查已拒绝“绑定digest变了就接管旧intent”：context/client/scopes编辑可能仍在消耗同一个refresh token。需按实际可消费材料身份处理，并为已完成权威提交但cleanup未完成的状态提供安全恢复；不能仅用时间、policy或revision变化当作新授权。

## 3. 红测和验证状态

修改生产逻辑前新增`authority_fixture.rs`和`server_authority.rs`。fixture使用正常Server coordinator、默认Redis backend source、磁盘journal/YAML与内存active-ID CAS；不是Local backend、真实Redis引擎或完整Redis immutable/prepared协议模拟。restart会从旧document创建全新store/runtime，然后调用真实`recover_startup()`，不clone旧runtime冒充重启。

`source-before.json`及`source-red-v1.json`记录修改前与红测源码hash/有效行。生产逻辑未改时，`red-authority-v1.log`已终态：编译8m06s，0 pass / 2 fail / 0 ignore，分别实际复现authority保存后跳过cache，以及cache故障时pending提前结束；不是fixture编译错误。

已实施正常可写Server的runtime接线与authority→cache双阶段写回；配置成功后再次验证当前授权，保留ticket原cache期望材料。配置失败不先写cache；cache失败不释放pending；重试已保存配置不再生成新revision。Worker coordinator=None保持原能力，不开放现有通用automation。Local无Redis的路径与原Local shutdown语义保持。

测试加强为四项：完整cache access/refresh/context/expiry/lifetime与authority对齐、实际Committed journal与archive材料、从旧bootstrap两次新runtime恢复、独立pending map不得绕过锁、由scheduler自己重试并清理pending、配置writer冲突不先写cache、重新授权后旧cache写回丢弃。fake仅严格比较active revision ID，`store_revision/store_prepared_transaction`不模拟真实Redisimmutable payload/transaction脚本，不外推完整Redis或分布式验收。

`source-frozen-v1.json`冻结5项源码，同一冻结源码上实际通过：

| 检查 | 结果 / 日志 |
| --- | --- |
| 刷新调度及本轮四项authority场景 | 46 pass / 0 fail / 0 ignore；`green-authority-v1.log`，编译9m01s、运行4.77s |
| routing/config相邻 | 119 pass；`routing-adjacent-v1.log` |
| Grok协议/会话相邻 | 60 pass；`grok-adjacent-v1.log` |
| credential payload相邻 | 8 pass；`hosted-adjacent-v1.log` |
| OAuth request wait相邻 | 2 pass；`wait-adjacent-v1.log` |
| 正常coordinator事务/回滚/恢复合同 | 12 pass；`transaction-adjacent-v1.log`，使用原合成backend，非真实Redis |
| 测试结果去重 | 247次执行、247个不同完整测试名、0重复/失败/忽略；`test-result-audit-v1.json` |
| 代表性feature关闭编译 | `--no-default-features --features line-gemini-web-reverse --lib`通过，1m39s；`feature-disabled-v1.log`，不是完整feature矩阵 |
| 静态 | Rustfmt、checker31/31、ratchet4078文件/14 immutable/governed source >700=0及tracked whitespace通过；`*-v1.log` |
| 冻结与文本 | 5项源码hash、10份UTF-8无BOM文本、86个本地链接及围栏/含未跟踪文件的whitespace通过；`source-doc-audit-v1.json` |

生产有效行`token_refresh.rs`180→187、`persistence.rs`153→158、测试接线162→164；新fixture100、新测试228，均未超过250，没有baseline/豁免或依赖/存储版本变更。后端命令使用`--locked --jobs 4 --no-default-features --features runtime-defaults`，未运行默认management-ui发布/清理hook。测试进程仅设16MiB `RUST_MIN_STACK`，不是生产线程设置；TEMP/TMP/TMPDIR明确位于本证据目录的`runtime-temp`，只使用合成材料和随机loopback。

### 3.1 Shutdown 和当前仍未覆盖的边界

v1正常循环不会因Gateway shutdown信号中断已发起的exchange/commit；结束循环后的Server最终flush仍有既有全局10秒预算。本轮将正常authority commit纳入该flush，所以预算到期可drop尚未完成的提交future，之后依赖既有journal/recovery处理事务阶段；不能声称最后一次提交必然完整join，也不能把此timeout描述成只影响cache。Local最终flush未改成这一预算。v1正常循环authority无预算的问题及v2修补见第4节；强制中断/崩溃及派生cache重建仍属于完整退出条件6–7，不以正常恢复测试代替。

当前合成重建验证不是启动真实Server进程、真实Redis引擎/TTL、旧新二进制混跑或Worker滚动替换验收；实际业务和额度也未验证。没有读取真实`.ng`、历史凭据或在线数据库，没有实际授权/刷新/推理/生成、devtest包、提交/推送或在线替换。正常管理登录、其他R项与P50/P60/P70仍未完成。

## 4. v2：Server authority 有界等待和中断恢复

v1后复审确认了实际缺口：正常调度直接等待authority，既有cache的10秒timeout不保护更早的配置保存；默认deadpool/redis连接与响应配置也没有等待上限。若Redis接受命令却不回复，循环无法进入shutdown检查和最终flush。先增加合成stall而不改生产逻辑，`red-authority-timeout-v2.log`实际编译12m10s后为0 pass / 4 fail / 0 ignore；四项均触发外层watchdog，原生产两文件hash保持v1值，不是编译或fixture错误。

修补只作用于有Redis的Server authority attempt，固定20秒内部预算；需要恢复时，`recover_startup()+persist`共用这一份预算。v2先用ticket内存位标记恢复义务，失败、timeout或外层drop保留同一token/context/guard，恢复失败不越过它写cache；该位的生命周期缺陷及v3 owner修正见第4.3节。正常首次成功与仅cache重试不反复扫描journal，避免把最多100,000记录的同步恢复扫描加入每个成功刷新。Local仍走原persist，不改配置、数据库版本或canonical digest。

扩展fixture分别在active load、activation生效前、activation已生效但尚未回复时停止，mutex不跨await持有。四项测试覆盖authority load及其失败后recovery load都须有界、activation前回滚后重试同一rotation、activation后完成同一revision再续cache，以及shutdown在stall期间请求后单次`refresh_once`能返回并保留最终flush的ticket。已生效场景同时覆盖refresh token轮换和返回同一refresh token，均缩减scope；不把“返回同值”写成“省略refresh字段”。测试预算由私有task-local缩为250ms，外层watchdog为3秒；生产仍固定20秒。

shutdown测试调用真实`refresh_once`及原ticket直接flush，检查journal/cache和guard释放，但没有创建完整AppState/scheduler loop，不冒称任务join或真实进程退出验收。同步文件系统调用不受Tokio timer抢占，20秒不是磁盘IO的硬实时保证。两个阶段共用预算由同一个timeout owner保证；现有stall测试证明有界，不单独证明阶段累计耗时。

### 4.1 v2 验证记录

`source-frozen-v2.json`冻结6项源码：`token_refresh.rs`187→188、`persistence.rs`158→199、tests接线164→168、fixture100→147、既有authority测试228→228、新timeout测试211有效行。没有超过250行的新职责、依赖升级或baseline/豁免变更。`green-authority-v2.log`已终态：编译12m28s、运行2.78s，50 pass / 0 fail / 0 ignore；不是相邻或全计划验证。复审又确认pending被supersede时会丢失仅存在该ticket中的recovery位，不能把v2作为最终收尾版本；后续正常重授权/clone重试的行为红测已实际复现，修补与最终门禁见第4.3节，不复用v1的247绿冒称当前源码全绿。

### 4.2 仍未完成的 Redis 迟到执行窗口

fixture中的never-ready future在timeout时被drop：activation前不会另有后台命令迟到执行，activation后则只模拟“已经生效、回复不可见”。真实Redis客户端取消future不撤销已经发送的命令；另一连接恢复先读到旧active并回滚后，原activation仍可能迟到。这一时序还没有服务端fence或完整恢复证明，**不属于本批已关闭范围**，仍须实现及验证，不能把20秒预算或正常journal恢复冒称全部取消安全。只读Worker、跨实例grant互斥、进程崩溃后的cache修复与其他完整退出条件均保持未完成。

下一步设计候选是在正常backend的精确transaction key上原子撤销Prepared，再允许本地rollback；activation已先执行时则恢复提交。现有activation已比较完整Prepared JSON，immutable prepared写入采用SET NX/精确比对，因此撤销需要保留同tx tombstone，不能DELETE；prepared尚未执行时也要防迟到创建。还必须区分当前AlreadyActive的revision相同与本tx确实成功，允许旧tx A撤销后新tx B合法复用同revision，不误撤销B。当前YamlReplaced record不能直接当原Prepared，须用原created_at/backup leaf等完整字段重建；namespace由正常store拥有，不接受外部任意key。这只是经源码复审的候选，不是已实现的fence；abort响应丢失、先后顺序、旧tx/后继tx、精确mismatch和namespace均需新合同。

### 4.3 v3：恢复义务属于 runtime，不随旧 ticket 丢失

v2复审发现旧pending可在重新授权/删除后被scheduler短路移除，ticket内true标记随之丢失。`red-authority-owner-v3.log`已终态：编译10m02s、运行0.47s，0 pass / 1 fail / 0 ignore，实际断言新授权不得越过旧未收敛journal失败（pending为0而非1）；不是测试编译错误。测试通过正常management commit显式替换access/refresh，再使用runtime clone执行新授权刷新，不直接伪造journal或仅检查某个私有位。

修正将恢复义务移到`RouteConfigRuntime`的共享`Arc<tokio::sync::Mutex<bool>>`，所有正常构造汇入with_backend；派生Clone共享同一锁与状态。Server在同一个20秒timeout里等待锁、读取旧状态并置true、必要时recover、persist；只有成功才在持锁时清位。future drop释放guard但保留bool，旧ticket即使被丢弃，新ticket/clone仍须先恢复；恢复不确定则保持pending，不为恢复旧事务而写回旧授权。Mutex还避免并行attempt清掉后继owner的标记；没有把这把锁加入recover_startup内部形成递归锁。正常management写仍沿原writer lock/CAS，不声称被refresh专用锁统一协调。

第二个owner测试持有clone共享锁，验证锁等待也受同一预算约束、等待者不修改占有者状态；释放后仅保存原rotation并完成cache，不重发grant。新freeze为`source-frozen-v3.json`八项：相对v1，token_refresh187→187、persistence158→197、tests接线164→169、fixture100→147、authority测试228→228、runtime381→383，新timeout测试211、新owner测试122有效行。runtime只加共享恢复owner的必要字段与构造，没有拆改其他职责；没有新增大文件或豁免。

同一v3冻结源码上，以下进程均已终态，无本任务仍运行的构建/测试会话：

| 检查 | v3 实际结果 |
| --- | --- |
| 刷新全模块，含6项新增timeout/owner测试 | 52 pass / 0 fail / 0 ignore；`green-authority-v3.log`；编译11m14s，运行3.75s |
| routing/config、Grok、payload、OAuth wait相邻 | 119 + 60 + 8 + 2 pass；对应`*-adjacent-v3.log` |
| 正常coordinator事务/回滚/恢复合同 | 12 pass；`transaction-adjacent-v3.log`；编译6m36s，运行0.28s；非真实Redis |
| 结果去重 | 253次执行、253个不同完整测试名、0重复/失败/忽略；`test-result-audit-v3.json`；不累加v1/v2重复执行 |
| 代表性feature关闭编译 | `--no-default-features --features line-gemini-web-reverse --lib`通过，1m46s；不是全feature矩阵 |
| 静态及有效行 | Rustfmt、checker31/31、ratchet4080文件/14 immutable/governed source >700=0、tracked diff通过；`*-v3.log` |
| 冻结/文本/本地引用 | 8项源码hash、13份UTF-8无BOM文本、86个本地链接、围栏和含未跟踪文件的whitespace通过；`source-doc-audit-v3.json` |

本轮生产改动的输入来自既有受约束ticket和正常runtime，不新增外部请求字段或secret出口；日志仅固定描述、cred ID和现有错误code。共享异步锁无新后台任务/线程/进程；锁等待、恢复和提交共用预算，取消释放guard，恢复位留在runtime。失败重试仍由原scheduler周期驱动，不新建无界重试循环。正常成功不多做journal扫描；失败恢复所需同步磁盘工作及迟到Redis命令边界如上，不虚构硬实时或服务端取消保证。

此状态仅实例内共享且在内存中，独立with_backend/多进程不共享；重启仍依赖正常journal恢复。重授权后的模糊旧事务测试证明安全阻塞并保留新pending，不证明自动解决所有journal歧义。没有读取或试用真实凭据、调用上游、出包、提交/推送或在线替换；不会因此将第4.2节、Worker或完整P29标完成。
