# 服务商分渠道扩展：第 28 批，授权材料与路由生命周期分离

> 2026-11-03（Asia/Shanghai）；R05/W03/P20 的进程内交接定向验证已收尾。Server 持久 owner/重启/跨实例缺口转 P29；不是完整渠道或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；不提交、推送、打包或接管运行实例。

## 1. 证据与拥有范围

继续 [P27](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p27.md) 源码复审确认的两种 Server 路径：交换中 policy/enable 替换导致已轮换结果丢弃；刷新成功后仅 policy 替换从旧 document 恢复旧材料/缩权上限。该缺口不靠取消 generation、复用整个旧 routing Arc 或只写 Redis 处理。

本批拥有 routing/config 的刷新材料、交换 ticket、publication 接线，以及 token_refresh 的一致性快照、pending绑定和相关定向测试；不修改 S06/Gemini。子代理只读核对调用点和测试设施，主代理写代码。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p28`。`source-before.json` 已在新增测试/改源码前记录哈希与有效行；新测试先使用既有 API 执行 Server `local=None` 分支，不读取真实凭据、不连接现有 Redis。

## 2. 实施不变量

- 授权材料 owner 统一持有 token、context、expiry、generation、single-flight、失败退避/terminal；路由 wrapper 单独拥有 active。
- policy/enable替换退休旧wrapper，但相同授权沿用材料owner；已发起交换可安全完成，旧wrapper不能重新选中或发起新交换。
- 真实授权/受众变化或删除撤销旧材料owner，迟到成功/失败和pending不能污染新授权。
- 单次交换所需的generation/token/context/previous access必须来自同一个锁下快照；明确缩权保持后续上限。
- 同一single-flight `OwnedMutexGuard`从交换ticket移动到pending，覆盖缓存故障后的跨周期重试及整个Local提交窗口；不只依赖某个调度循环的pending map来防第二轮grant。
- Server document作为配置anchor可以落后于runtime rotation，不得据此回滚runtime。Local正常automation保存须精确匹配当前已发布材料才认作同授权推进，不能接受任意历史材料或仅凭账号ID复用。
- 原raw-entry Redis CAS与正常local revision/CAS不变，不新增scheduler、存储版本、依赖或凭据迁移。逐字相同材料的重新授权操作仍受现有schema表示能力限制，不声称可以区分。

## 3. 实测状态

`red-server-policy-v1.log` 首次在新增测试自身的临时借用处出现E0716，已按编译器建议绑定Pending变量；这不是产品缺陷或产品红测。生产代码仍未修改时，`red-server-policy-v2.log` 实际编译11m07s，**1 pass / 3 fail / 0 ignore**：确认Server交换中rotation丢失、交换后policy替换回滚材料/expiry、迟到失败退避被policy重置。真正重授权/删除的负向保护本来就通过，不计作修复。

生产实现已应用：共享材料owner、独立routing wrapper、一致性RefreshAttempt、pending代际与持锁、automation-only当前rotation精确推进；Any策略候选的legacy fallback也排除已退休wrapper，不只检查OAuthOnly。额外固定比较有效adapter/base/auth mode/header/token及session transport，避免把受众变更当policy变更。

`source-frozen-v1.json` 冻结21项源码。`green-refresh-v1.log` 已终态：编译14m13s，39 pass / 2 fail / 0 ignore，不能报通过。两个Server缓存场景先在RESP witness遇到`unsupported fixture Redis command: UNWATCH`，另有`pending.is_empty()`断言失败；此前“执行中”的状态已过期。

已亲读锁定依赖`deadpool-redis 0.18.0`的`Manager::recycle`：连接复用发送`UNWATCH`及`PING <nonce>`，要求回显同一nonce。v2只修测试夹具这两种准确回复，并新增cache值/commit计数不变的回归；未修改生产刷新语义，也未放宽失败断言。`source-frozen-v2.json`冻结21项源码，在同一冻结源码上完成以下验证。

| 检查 | 实际结果 / 日志 |
| --- | --- |
| OAuth refresh定向 | 42 pass / 0 fail / 0 ignore；`green-refresh-v2.log`，编译13m23s，运行2.41s |
| routing/config相邻 | 119 pass / 0 fail / 0 ignore；`routing-adjacent-v2.log` |
| Grok授权/会话相邻 | 60 pass / 0 fail / 0 ignore；`grok-adjacent-v2.log` |
| credential payload相邻 | 8 pass / 0 fail / 0 ignore；`hosted-adjacent-v2.log` |
| OAuth request wait相邻 | 2 pass / 0 fail / 0 ignore；`wait-adjacent-v2.log` |
| 去重审计 | 231次执行、231个不同完整测试名、0重复；`test-result-audit-v2.json` |
| 代表性feature关闭编译 | `--no-default-features --features line-gemini-web-reverse --lib`通过，2m01s；`feature-disabled-v2.log`，不代表全feature矩阵 |
| 静态 | Rustfmt、checker31/31、ratchet4076文件/14 immutable/governed source >700=0及tracked whitespace通过；`*-v2.log` |
| 冻结/文本审计 | 21项源码hash、26份UTF-8无BOM文本、85个本地链接、围栏及包含未跟踪文件的独立whitespace通过；`source-doc-audit-v2.json` |

新增材料121、ticket96有效行；新测试最大197，缓存witness由173到196，受影响现存源码最大429，没有新增500行以上源码，也没有修改baseline或豁免。测试命令统一使用`--locked --jobs 4 --no-default-features --features runtime-defaults --lib`，仅给测试子进程16MiB `RUST_MIN_STACK`；未改变生产线程设置或运行默认management-ui发布/清理hook。

### 3.1 新验证的准确边界

Server测试显式传`local=None`；随机loopback OAuth和RESP witness只用合成材料。缓存测试走生产GET、SCRIPT LOAD、EVALSHA与原raw-entry CAS；witness逐字核对生产Lua脚本并实现该脚本的比较语义，但不是实际Redis引擎、Lua执行器或TTL长期验证。正常结束显式stop/await，并shutdown/join连接任务；异常Drop只作为信号/abort兜底，不宣称已await所有异常清理。

新增覆盖交换中policy/disable、交换后policy及disable→enable、scope ceiling、迟到失败、真正授权/受众变更、独立pending map不能重复grant、缓存故障恢复后仅重试存储、正常Server第二轮使用runtime预期材料、删除重加和Local正常保存保留授权owner；相关测试已包含于上述实际通过记录。不是Server全新进程重启、真实Redis或正常OAuth管理登录验收。

配置owner检查与Redis CAS并非跨系统原子事务；缓存写入依赖既有精确原材料/raw CAS。当前witness集成场景不主动注入GET与CAS之间的并发raw冲突，不能外推该重试分支或分布式撤销时序的全面验证。逐字相同的“重新授权”若无删除/显式授权变化仍受现有schema表示能力限制。

## 4. 后续完整范围

R01–R16、必要的Vertex Express/Luma Agents分支、Qwen/Bedrock剩余保真与Server/PG审计取消、正常OAuth管理登录/保存、媒体生命周期、P50包/隔离启动、P60集中录制与P70逐渠道真实验收全部保留。九个新增前端模板数量不因本批底座改变。unknown不合池，Web不是兜底。

### 4.1 P29 已确认的 Server 持久 owner 与跨实例缺口

主代理亲读`src/runtime.rs`、`src/token_refresh.rs`、`src/token_refresh/persistence.rs`及正常`RouteConfigRuntime`提交owner后确认：Server scheduler从Console active revision/legacy mirror/YAML构建材料，但`local=None`写回目前只更新credential cache，没有将rotation/context/expiry推进启动时使用的持久配置。请求候选的cache reader和credential内存cache失效任务都不负责恢复scheduler。故P28进程内交接验证通过，仍不能证明Server重启使用最新refresh token或缩权上限。

只读专项进一步确认Worker与Standalone均启动本scheduler；Worker的配置只读gate不阻止HTTP grant，Splitter滚动替换有新旧Worker重叠窗口。进程内single-flight、配置revision CAS、cache raw CAS均不是发送grant前的跨实例互斥，不能用写回CAS证明只交换一次。

P29必须一起确定正常持久配置owner、只读Worker的授权职责及多实例grant所有权；获得所有权后重新读取最新权威材料。配置已保存/cache待重试、授权已撤销和全部完成需分开表达，保留ticket原expected材料及pending guard，不在cache故障后重发grant。租约过期/丢失、实例崩溃和上游已消费token但结果未知的窗口必须明确，不宣称普通租约提供上游exactly-once。当前未实施这部分，不以禁用Server刷新或让Worker绕过管理只读权限代替最终功能。

可复用`RouteConfigRuntime::with_backend`、`RouteConfigRedisBackend`及`tests/console_transaction_support`的合成backend形态验证正常automation commit、全新runtime恢复、重复恢复与失败重试；现成fake的Activated分支不严格模拟expected-revision CAS，不将它冒称真实Redis引擎验收。所有临时目录继续显式定位linshi。

后续正常可写Server的authority→cache前置与合成恢复已取得 [P29](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p29.md) 定向证据；只读Worker、跨实例grant及崩溃/结果不明场景仍未完成，不用该前置抹去本节的完整退出要求。

没有读取真实`.ng`、历史凭据或在线数据库，没有向实际上游发出授权/刷新/推理/生成请求；没有提交、推送、构建devtest包或替换在线实例。所有后端测试已终态，不留“仍在运行”的旧状态。
