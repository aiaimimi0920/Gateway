# Accio 旧 DB 材料的非破坏性读取兼容：限定专项完成

日期：2026-11-09。承接 R01 新导入归一化之后的“未变 source hash 已有 DB 兼容”缺口；总目标、DB 刷新调度和 P50/P60/P70 不缩小。

## 最新结果

已实现并专项验证已知旧 folder normalizer 形状的普通 DB 读取兼容，不需要修改 source hash、强制重导或更新存储版本。**这是读取兼容，不是数据库写迁移，也不是 DB Accio refresh scheduler。** 本批不登记整卡 `offline_ready`。

| 检查 | 实际结果 |
| --- | --- |
| 最终普通 reader 新回归 | 8 passed，包括 canonical access 清空后不得回退旧 body token |
| 同一冻结 r2 lib：Accio / DB payload / folder / refresh / masking | 128 / 20 / 195 / 152 / 3 passed；相互有重叠，不相加 |
| 上述 lib 按完整测试名称去重 | **461 passed / 1 failed / 18 既有 opt-in ignored**；8 项 reader 已包含其中 |
| 真实专用 PG/Redis integration | **3 passed / 0 failed / 0 ignored**，独立 harness，不与 lib 去重数混算 |
| 生产 lib：开启 Accio | passed，`--locked --offline --no-default-features --features runtime-defaults -j 4` |
| 生产 lib：只关闭 Accio leaf | passed，从 runtime-defaults 展开列表中仅移除 `line-accio-web-reverse-api` |
| 格式与代码规模 | 9 项定向 Rustfmt passed；checker 31 passed；最终 ratchet 5163文件通过，governed source >700为0 |

相邻回归唯一未通过的是 `token_refresh::tests::nous_lifecycle::nous_scheduler_preserves_receipt_before_publishing_the_exact_successor`，在 `nous_lifecycle.rs:12` 预期 mock 出站一次、实际零次；同一冻结 r2 EXE 单线程独立运行仍失败，不作为 flaky 跳过。首轮另一个 Nous cache fixture 失败在 r2 中已通过，原失败证据仍保留。Nous 与共享 refresh 有并行 owner 和多项构建期间输入变化，本批没有修改这些文件、替其调整断言或声称已确定当前 Nous 根因；**扩大回归不是全绿，也不是当前共享工作区的全量联合验收**。

最终 lib EXE SHA256：`9b77e91c081b3348cc9ccf99c115b5c79c5f9579f222b14f96ccc94bdaab3812`。integration EXE SHA256：`b80b94c4fe71857efffb98411416e6317f36f58975844de8d26df1ca967ebe5a`。它们是冻结测试程序，不是 Gateway 可交付手测包。证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-accio-db-reader-20261109`。

两种生产 lib 检查均在本批源码不变下完成，仍有既有 unused/dead-code warnings，不执行无关 `cargo fix`。关线只做生产编译，新增 compiled-out reader 合同尚未单独执行。checker测试和最终格式按未变消费输入复用，不虚报重跑。最终范围/编码/来源、diff以及资源终态由`final-proof.json`、`scope-audit-final.json`和`container-closeout.json`记录；上述1个相邻失败不因此变为通过。

## 当前 owner 与方案

真实 folder import 同 hash 分支在 hydrate/normalize 前跳过，这是已有其他渠道的性能和行为合同，不改为强制重导。普通完整 DB reader 从 inline/object payload 构建 view，list/get/source-path/active runtime lookup 均复用同一 owner；兼容投影已接在此入口，源文件已不在场但仍保留 DB 行时也走同一路径。真实 PG 测试验证重复读取不写整行或 xmin；正常管理列表经 router oneshot 验证脱敏，不称为真实平台或原生 UI 验收。

选用向后兼容 reader 而不是后台 DB 写迁移：不改原存储 JSON/object、source hash/path、稳定 ID、状态、运行诊断、时间戳或 xmin，不增加 schema/storage version，不要求用户编辑数据库。回滚或混版本 reader 仍可读取原存储；后续正常显式保存才由既有 writer/CAS 决定是否持久化已投影材料，迟到 snapshot 仍必须冲突，不获得新写权限。

只识别 folder provenance、Accio surface、固定业务受众、rawSource 与准确已知旧 body-copy/device 形状。原 access/refresh 未轮换且现存 SG/client/endpoint/expiry 仍匹配原材料时，从其原始归档补齐被旧 normalizer 丢弃的字段；新材料、不同 grant、显式 null/clear 或同义字段不得被历史材料覆盖。部分 clear 或不同上下文也阻止恢复其他字段。清理已识别的旧 body 副本后，普通 planner 仍必须从 canonical access/device 绑定业务认证。源归档不是已验证账号身份，不能制造 client、期限、SG 或额度。

材料解析复用上一批严格 parser 与协议 expiry owner，不维护两个可漂移的算法。共享 parser 已移入协议材料 owner，folder normalizer 和 DB reader 只作边界接线；源码移动前原 bytes 已归档，不接管共享 refresh/Nous/Dim/Vertex owner。

## 验证过程与首轮证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-accio-db-reader-20261109`。已保存直接 preimage/hash。初始6项普通 hydrated reader 合同取得真实红测终态：1 passed / 5 failed，非编译失败；随后实现生产修复并新增第7项 partial-clear 合同，该新增项没有独立红测。首轮7项 reader 测试全过，Accio127、DB payload19、folder195/14 ignored、共享refresh150/1 failed/4 ignored、mask3；同一冻结lib按完整名去重458 passed / 1 failed / 18既有opt-in ignored，不相加。唯一失败是并行Nous的`nous_direct_cache_cas_rejects_preexisting_expiry_drift_and_preserves_quotas`：假Redis fixture固定expect `gw:cred:account`，实际请求`gw:cred:exact-build`，本批不修改其owner或用改断言掩盖失败。

首轮绿测编译沿原handle等锁后完成，未重启或另开同类编译。2026-11-09 01:05（UTC+08）补充源码冻结及前后进程快照证明首轮lib最终修改发生时，自己的Cargo仍没有rustc子进程；01:07观测到该Cargo实际启动rustc。首轮`scope-audit.json`核对本批7个lib源码在此补充快照与编译结束一致。最初启动快照不同的两项本批文件不假报稳定；并行Nous两项测试输入变化、随后新增integration文件的调整单列，不冒称整个共享工作区来源冻结。首轮冻结EXE SHA256为`fda071f9abc77374aa88b6db0edf07ab9fd079be49e2651bbffc92a9f02d0e95`。

随后阅读真实planner及其legacy-token-only回归，确认canonical access清空时仍可能回退旧body token。已将精确认形与“当前access非空”脱钩：原`apiKey`字段仍须存在，null/空串也清理已证旧body副本，但不恢复SG/client/expiry；新增第8项普通reader→planner拒绝空access合同，并扩大轮换测试的clear/null分支。此两项源码变化发生于integration Cargo仍排队、没有rustc子进程时，补充快照见`integration-queued-final.json`。之后另做稳定本批输入的r2编译/8项测试，最终证据见顶部，不以旧EXE代验；新增clear分支没有独立红测。

新增3项真实PG/Redis opt-in合同已经全部通过，覆盖同hash不重导、正常多种DB reader、管理列表脱敏、仍保留DB行的源文件不在场、迟到CAS与轮换、object原bytes/mtime及普通planner。integration编译前的补充快照证明本批最终源码变更发生在其实际rustc启动前；完整共享输入中的并行Nous变化单列，不声称全仓冻结。最终8项lib的r2编译沿原handle完成，只有真实源码变化才新增r2，不因超时重启编译。

## 改动范围、来源与审查

普通 reader 的最小接线位于 `src/db/provider_credentials/payload.rs`，兼容职责独立在 `payload/accio_legacy.rs`；共享材料 parser 从 folder 迁到 `src/protocol/accio_oauth/import_material.rs`，实际实现除可见性外按忽略空白比较相同，folder 只改引用。共用 `payload.rs` 在 preimage 后新增的 `merged.remove("nous_portal");` 是外部 owner 的编辑，已保留，不归为本批。

官方 checker 有效行：共享 reader 182→193（含外部1行），兼容 owner 172、新 reader 回归285、共享 parser131、协议入口115→117、folder入口117→116、原 Accio normalizer251→251、integration199及fixture68；旧 parser133行移入协议后不再保留第二份。285行测试只围绕一个普通 reader 兼容合同，251行 normalizer 保持原单一职责；本批没有大文件/例外/基线放宽。

兼容 owner 只有纯 JSON 投影，无网络、DB/object 写入、后台任务、额外队列或重试。固定受众和 provenance 是认形边界；材料共用原固定字段/长度验证，错误不回显秘密，不从 rawSource 制造身份、client、SG、TTL 或额度。已核对取消/资源路径不新增任务，真实 fixture 的 schema、专属 key、临时文件与测试对象由原 owner 清理。`scope-audit-final.json`、`scope-final.diff` 和阶段源码快照保留本批9个现存Rust文件及迁移旧文件的直接证据；并行文件变化由各 receipt 单列。

本机Docker daemon与缓存PostgreSQL16/Redis7镜像已核实。收到本批新明确同意后，只创建run `26ebfbf1fdc04a57842d0e180d46f80a`的一对tmpfs/loopback容器，无现有卷/持久化。首次guard初始化碰到PostgreSQL镜像临时socket服务关闭的启动竞态，原报错和失败保留；改用TCP readiness后只恢复同一对容器的初始化，未再创建容器、重启现有服务或重复消费授权。三项测试通过后已按ID/name/label/mount校验，仅停止并删除此新对；清理前后既有容器集合都是同样44个，卷集合完全一致，无新增遗留卷。`container-closeout.json`与原命令日志保留。未读取真实凭据、调用供应商、改现有实例、出包、提交或推送；不复用已消费P34授权，本批授权也已消费。

## 明确保留的未完成项

DB Accio refresh scheduler 尚未接入；读取兼容或能读 camel 字段的 cache repair 均不能代替实际刷新任务。完整 Accio 取消 audit、真实 client/wire/模型/额度、完整运行模式/进程/包，以及其他渠道和 P50/P60/P70 继续。模板仍17，不登记新增整卡 offline_ready。
