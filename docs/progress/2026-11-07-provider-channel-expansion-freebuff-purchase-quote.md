# FreeBuff：冻结报价与持久购买准入子批

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承大量未提交实现，HEAD 不是本批源码交付凭据；没有提交、推送、出包或操作运行实例。

## 实际实现范围

本批承接[原 claim/run 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-run-context.md)，增加内部的冻结报价、确认、重新检查与 LocalRuntime 持久 claim 组合。**没有增加管理购买按钮、正常 UI 确认或公共请求 receipt；目录公共 guard 仍返回 `409/freebuff_business_admission_required`。**这不是整个 FreeBuff 渠道完成，也不是 P50 验收。

- [purchase_quote.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/purchase_quote.rs)：首次 signed inspect 按当时的价格规则物化报价。不可 Clone/Deserialize 的 quote/confirmation 绑定 provider、credential、revision、token/origin/client、registration key ID/公钥与稳定 model key。60 秒单调 TTL 不因用户确认或 fresh GET 延长；显示 DTO 不是许可。
- 确认后再次 signed inspect，比较全部余额、每日余额、钱包、当前价格、grant、discount 及两个 flags；不是只比较一个布尔。保留 fresh raw body，在 commit 前及 POST 前重新按当前时间投影并比较**原冻结数值**，不同时重算旧报价。价格未知/缺失、余额不可承担、foreign active 或 ended grace 不产生新购买能力。当前只支持 CLI，不猜 Desktop purchase identity。
- [purchase.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime/freebuff_sessions/purchase.rs)：内部 `admit_freebuff_for_request` 先恢复已有 row，核原模型并对原 claim GET；恢复不要求新的购买许可。没有 row 才消费一次确认；fresh GET 不持 SQLite 写事务或 OS guard。进入原 prepare 后的并发 row 直接用其返回的 recovery guard，不重入锁。
- 原 LocalRuntime prepare 的生产入口变为 private checked；commit 前两处及 commit 后 POST 前重验 quote 与调用者提供的 fence，POST 返回后再验 fence。原签名的便捷 prepare 仅留 `cfg(test)`，既有 journal 测试继续检查同一个实现。没有第二套数据库、锁、签名、重试或授权系统；管理 fence 仍须由真实入口复用现有 Grant/revision owner。

## 费用与失败语义

公开固定客户端使用 `balance + claimableGrant >= max(0, price - soloDiscount)` 作可开始投影，`max(0, price - daily.remaining)` 作钱包支出参考。实现保留标价，不把 discount 或 `usage.cost` 猜成最终 Freebucks 扣额；`spendWithoutAsking` 不跳过本批的显式确认。Gateway 当前采用比官方 intent 比较更保守的完整 meter 变化重新确认，未知报价不采用 legacy 免费兜底。

上游没有已证实的金额 ceiling 或报价 CAS。即使所有本地检查通过，最后 GET 与 POST 之间仍可能发生远端余额/价格竞态；不能宣称锁价或金额强保证。已知计划价格边界可用 fresh raw 的当前时间投影检查，不等于原子锁定供应商账单。

commit 后撤权、过期或取消可能保留一个**尚未发送 POST 的本地 claim**，不代表已购买、已扣费或远端结果未知。它仍阻止自动换身份重买；后续只能对原 claim GET。本批不引入清理、终态 closeout 或自动下一次购买。POST 后撤权也不能回滚远端操作；保留原 obligation，不偷偷 DELETE 或重试。

## 来源与证据

参考 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的既有公开原文存档：admission hook、model-selection、session-pools、price-changes。仅借鉴协议事实与费用语义，未运行/安装参考项目。官方 hook 比较 intent，并且旧/新 snapshot 均可按确认时刻重算；本批没有继承该行为来冒充不可变报价。固定客户端不是当前服务端合同或真实额度有效性证明。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-purchase-quote-20261107`。`before.json`/直接 preimage、阶段命令和源码 hash、原始 logs、范围 diff、有效行数及最终 audit 在该目录独立保存，不覆盖上批回执。

新功能入口的第一轮 red 是缺少类型/方法的**编译红测**（16 errors），不是运行时行为红绿证明。随后定向 8/8 通过，再补并发/取消测试后执行最终 FreeBuff 范围。所有原始失败/警告保留；不以编译成功或 HTTP 200 代业务成功。

最终验证：

- FreeBuff 名称筛选 **277 passed / 1 ignored**（10 项新增）；LocalRuntime **49/49**。按完整测试名去重 **317 passed / 1 ignored**，不重复累计前一轮 8 项。唯一 ignored 是既有的专用目录/浏览器 UI fixture。
- 新测试覆盖缺确认零网络/零 row、八项金额/flag 变化、账号/版本/authority/model/registration 绑定、单调过期、dated/off-peak 冻结投影、commit 前后模拟 fence 撤销、一次持久购买与无新许可恢复、真实第二 LocalRuntime 插入 row 的竞态，以及 commit 前回滚/commit 后取消保留。价格边界是纯投影/能力检查，不声称真实 commit-await 跨价边界或已验证真实管理 Grant 撤销；真实管理/API 仍未接。
- 正常 runtime-defaults 与从该集合仅关闭 FreeBuff 的代表性生产库检查通过；**不是完整 feature 矩阵**。本批未改前端，没有伪跑 UI 联合验收或 devtest 包。
- 官方 Rustfmt、checker **31/31**、ratchet **4773 文件**通过；14 项 immutable runtime 单列，`governed source >700=0`。10 个新增/修改源码最大 **242 有效行**，未改 baseline/exception；根有效行报告已按直接 preimage 逐字节恢复。
- 最终 6 个适用阶段各 **4755 项选定输入**阶段内无漂移且终态一致；checker 的实际 owner 输入另验未变，不把其较早的完整输入集合也说成终态一致。4 份固定公开存档 hash、15 个选定文件 UTF-8/无 BOM、172 条本地链接、范围 diff 与 `git diff --check` 均通过。公共 guard 原 hash 未变，详见 `audit.json`；所选输入不等于所有环境/动态输入均冻结。

生产检查仍有 warnings，包含当前内部 owner 尚未接正常调用所产生的 dead code；没有用 suppression 或修改测试隐藏。

## 明确未完成

真实管理身份/Grant 到 UI 逐次确认、public caller 与一次性 receipt、exact candidate/context 传递、真正排队请求触发仍未接。现有媒体 probe `confirmCharge` 仅授权该次 probe，不能复用为后续公共请求许可；不能通过普通 body/header 布尔绕过路由。

完整 root/harness、decoder、费用累计/FINISH、持久 run/lease、终态 closeout/probe、Server/跨进程/原生 EXE、真实平台/费用及全范围 P50/P60/P70 继续。模板仍为 16，没有新增整卡 `offline_ready`。本批没有读取真实凭据、购买会话、发起真实模型请求或新建 Redis 容器；历史 P34 授权已使用并清理，不重复使用。
