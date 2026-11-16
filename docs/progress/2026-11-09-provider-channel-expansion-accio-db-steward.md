# Accio DB 后台刷新：worker 合同已验证

日期：2026-11-09。接续 R01 已验证的 DB refresh journal；总目标仍为完整分渠道计划，不将本子批当整卡完成。工作区 C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway，main / 7969f12c85efceacf2a5e384e143b5c39a550981；保留其他 owner 的 dirty edits，不提交、推送、出包或改动现有实例。

## 当前实际实现

- 独立 `provider_credential_refresh::accio::Worker` 已接正常 server background spawn 与 server shutdown join；local mode、未启用 refresh、未配置 PG、关闭 Accio leaf 或已经 shutdown 时不启动。代码核对发现正常 drain 早于全局 shutdown，现使用现有 `GatewayShutdownHandle` 作为本 worker 的 maintenance stop，由 `GatewayLifecycleState::begin_drain` 立即请求、server stopped/join 同样请求；不改其他请求的全局 shutdown 语义。现有 ChatGPT owner、表单发送和 TTL 锁不接管 Accio。
- 复用现有 interval / before / batch 配置；按稳定 ID keyset 每次只读一个 active/nonarchived Accio credential，保留单 worker cursor。Fresh/未知 Sending 不通过更新时间排序永久挡住后面的 due 行，不新增无界 ID 队列。
- 先读取永久 grant 的原 Result，之后才做 due 判断。期限来自持久化的精确绝对 expiry；改 interval/margin 或新 runtime 不用 `now + lifetime` 重建期限。
- prepare 验证实际材料后，必须取得明确 reserve commit。永久 Sending 未知或存储 acknowledgement 丢失不证明未发出，不能再次取得 HTTP 发送权。
- 新只读 `authorize_send` 事务重新锁 account → credential → family head → grant，核对原权限及原 Sending attempt，并保持锁直到专用 HTTP 的响应结束。删除/禁用/改材料不能在原权限确认与实际发送之间穿过；管理写可能等待最多原单次 HTTP 的 15 秒预算。SQLx 拥有 Drop rollback/连接回收，不额外维护 TTL lease。
- 复用 `protocol::accio_oauth::RefreshGrant` 的固定 origin JSON、无默认凭据、无重定向、单次 send+read 总期限和 decoded-body 上限；不传 SG/client/device，不落入通用 OAuth form。
- 已确认 Result 先进入单个内存 slot，再释放 permission transaction、等待存储。最多一个未确认持久化 Result；slot 未清不能接新发送。只在 durable Result 明确确认后清 slot；publication 失败由原 journal 恢复，重启不发第二次 HTTP。
- 每个存储阶段最多 10 秒；正常 shutdown 等待原 15 秒 HTTP，再做有限存储和一次 persistence-only flush，无 detached sender/retrier。真正任务 Drop／进程 kill 可能丢失尚未持久化的响应 bytes，但永久 Sending 不消失；不宣称跨 HTTP/PG exactly-once 或强杀后的结果必不丢失。

## DB 缓存职责核对

只读独立核验发现：DB credential 与 hosted Redis credential 是两个身份合同。普通项目路由 `pipeline/stage_route` → `db/routing/candidates` 每次读取 active accounts/credentials；unified access-key 路由虽缓存权限/model/account 索引，仍每次 fresh DB credential map 与 account payload。普通 hydrated reader 每次读取 inline，或通过当前对象指针实际 read/GET；OnceLock 仅缓存 object driver，不缓存 JSON/token。

`gw:cred` candidate 的 account ID 是 `cred:{id}`、provider_credential_id=None，entry 不含 DB account/generation/status/archived_at；没有发现 DB credential→此 hosted entry 的生产投影。`gw:provider:payload` 是 account payload cache，其 getter也无生产调用。**本批不因同 ID/token 误写或创建这些 Redis entry，不制造新的 DB token cache来凑接线。** 必须验证实际 DB publication 后正常 hydrated reader 读到 successor；已构造的 request-local candidate 不因此被强行重写。旧 hosted cache GET→SET 的并发风险是另一合同，不能以此 DB 读回声明修复。

## 验证和证据

证据目录：C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-accio-db-steward-20261109。

- 最终官方 lexer：state 193→196、runtime 392→398、旧 provider refresh 412→413、refresh 入口15→32、journal123→146；新 sweep 212、send authority 39，新增测试最大185。18个本批 Rust 文件均≤500且为 UTF-8 无 BOM；清单为 `final-scope-lines.json`，未改 baseline/例外。
- 四项 due 的真正红测已运行：**1 passed / 3 failed**，失败是过期、精确 margin 和 timezone/restart 到期判断，不是编译错误。构建期间六个尚未注册模块的源码新增，红测已消费的 due/入口/四项测试输入未修改；最终独立构建仍需核对全部实际输入。
- 最终 r2 的 `db-r2-steward`：**19 passed / 0 failed / 0 ignored**，默认线程栈运行。包含五项 due/future size、四项真实 guarded PG flow、三项恢复、七项 lifecycle/lease；覆盖单次专用 wire、普通 DB successor 读回、两个 worker、keyset fairness、fresh Result replay、Result/publication 存储故障、unknown send、shutdown、真正 task abort、reserve 后撤权、正常 runtime drain、snapshot lookup 中 drain 零新 grant，以及通过 `pg_stat_activity` 实测权限锁下 UPDATE 等待和迟到 Result 保存。
- 相同冻结程序的存储相邻合同 **12 passed**，其余 Accio/材料/管理/专用 transport/旧 reader 等定向相邻合同 **133 passed**。三组按完整测试名去重为 **164 passed / 0 failed / 0 ignored**，没有重复计算已通过的19项或12项，不是全仓回归。选择清单为 `r2-adjacent-selected.json`；未选择其他渠道或需要 Redis 的独立测试。
- 独立 r2 测试构建 exit 0，719.063秒；沿原 handle 完成，没有因等待重开。构建与上述三组测试记录的输入均无变化；冻结 `r2.exe` SHA256 为 `8390080dbadef66f682b70b73e8b5e691f14fd335e7d575b980f8e8702b62113`。这是 lib test harness，不是 Gateway 手测包，也不是全工作区的构建来源声明。
- 正常默认特性生产 `cargo check --lib` exit 0，135.750秒；只移除 Accio leaf、保留其余 runtime leaf 和 management UI 的生产检查 exit 0，95.094秒。均使用 locked/offline、4 jobs、串行构建，记录的输入无变化；没有以编译代替整 server 运行验收。
- 18个本批 Rust 文件定向 Rustfmt 通过。全仓 `cargo fmt --all -- --check` 唯一失败是本批范围外 `src/provider_runtime.rs` 的 `mod dim_model_probe` 排序，原失败保留在 `format-check.log`，未擅改该 owner 的文件，不报全仓格式绿。
- Checker **31 passed** 复用本批先前、未改 checker 输入的证据；最终 ratchet exit 0，扫描5197文件，14个 immutable runtime artifacts，governed source >700=0。最终文档更新后的 `git diff --check` 与来源／资源汇总见 `final-receipt.json`。

首轮 `green-build` 已 exit 0（615.062秒），但审查补充 drain/lease 用例发生在该轮编译期间，不能证明最终源码。首轮默认栈 `db-first-green` 的四项 due 通过后，在第一个完整 PG worker 用例发生 `0xC00000FD` / `has overflowed its stack`，进程异常退出；原失败保留。相同冻结 EXE 的单项 16 MiB 栈 probe 和15项大栈诊断都通过，指向大型内联 async future 的栈占用，不是PG/wire断言失败。生产 owner 已将 I/O／HTTP／process future 固定到堆上，保留原单 future、timeout/Drop/不重试语义，不创建后台 task。最终默认栈 r2 的19项通过，含 tick future ≤64KiB 保护；三组最终运行的 `rustMinStackOverride` 均为 null，未用 `RUST_MIN_STACK=16777216` 诊断替代修复。

## 授权和资源

收到本批新的明确“同意”后，仅创建 run `573f8e932db8431d9a56aed78c8686c2` 的单只 PostgreSQL 容器，缓存镜像、仅 127.0.0.1、tmpfs、无既有卷。复用已有 guarded fixture 的真实 schema，fixture 仅 Drop 本次新 schema。最终测试后已按原 ID/name/label/mount 校验，仅停止并删除本只，`container.json.status=cleaned`；45个既有容器的ID集合和全部卷集合不变，按本批label复查无残留。本次授权已消费，旧容器对授权不复用。本批不创建 Redis，不读取真实凭据、不调用供应商。

## 尚未完成

本轮独立 worker → 专用单次合成HTTP → Result持久化／发布 → 普通DB successor读回及新worker恢复的合同已验证，仅退役这一子链的接线与默认栈缺陷。正常完整 server/管理/public caller 联合、OS 独立进程强杀及 restart、原生包、真实第三方 client/当前 wire/模型额度及整卡仍未验；完整取消 audit继续。旧 account object writer/deletion 的锁前 side effects 不因新 send lease 消失。005 仍须 Platform schema owner 审核和正常生产 migration；本批不自动 DDL 或迁移既有服务器。

下一接力先做无凭据的完整 server／管理／public DB credential 消费联合，继续验证原 Sending／Result／Published 在独立进程 restart 下的行为及取消边界，再进入新源码 devtest 包与集中录制。不得重新把已验证的 DB worker 接线当未开始，也不得以此取消完整运行模式、进程、包或真实渠道验收。

17 模板、无新增整卡 offline_ready，P50/P60/P70和完整目标保持 active。
