# FreeBuff：正常浏览器原会话收尾与真实 Local TCP 联合验证

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。本批承接[正常管理收尾实现](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-managed-closeout.md)尚未执行的浏览器→真实管理 TCP 联合验证。未修改生产代码、提交、推送、出包或操作现有实例；本批 HEAD 不是未提交实现的交付凭据。

## 1. 实际完成的用户流程

正常 `apps/desktop/src/main.tsx` → 隔离静态 helper → 真实回环管理 TCP → 生产 `build_app_state` / `build_router` → Local SQLite、原 sealed claim/end/history owner。前端没有替换 API client、注入管理响应或伪造 Grant。

1. 使用合成管理 token 正常登录，打开已保存的 `pool / b` 账号；另有 `a` 账号用于完整配置保留比较。两个账号禁用调度，当前模型列表为空。
2. 点击“检查本地会话”，显示原 `attemptId` 和原模型 `open`。读取只访问本地 journal，不请求供应商、不创建新设备；不要求原模型仍在当前账号模型列表。
3. 打开“结束原会话”确认框，核对账号、业务站点、CLI、模型和原 attempt；先点击“取消”。后端实际断言 active=1/history=0、上游请求=0。
4. 再次打开确认框，明确确认结束。真实管理 owner 重新读取 signed catalog，再发送唯一原 attempt 的 DELETE；body 为空，Bearer、instance ID、attempt ID 和 device signature header 绑定原身份。签名算法本身沿用前批保护，本批 TCP helper 只检查 header 存在及固定身份，未重新独立验签。
5. 正常页面显示“原会话已收尾，最终退款：0 Freebucks”；再次点击检查，显示无本地待处理会话和原 attempt 的最终退款 `0`。后端 actual active=0/history=1，累计只有一条 DELETE，无 admission POST。
6. 正常停止并同目录、同端口重建完整 AppState/runtime；核对完整配置 document/revision 与原值相等，旧 Grant 被真实 TCP 403 拒绝。重新加载正常页面，管理验证/Grant 由生产接口重新取得，再从同账号检查历史：同一 attempt、模型、退款 `0` 仍可见，无新增上游请求。

这里的重建是**同一个测试进程内的完整 runtime 重建**，不是 OS/应用进程重启、原生 EXE 或跨进程恢复。原 claim 是隔离 fixture 经正常设备注册和内部 sealed prepare 边界准备的合成待处理记录，没有执行真实购买，也没有开放 idle 管理购买。

## 2. 长期测试与边界

- [测试接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim/tests.rs)只增加 `browser`、`tcp` 两个测试模块，由既有 `cfg(all(test, feature = "line-freebuff-web-reverse-api"))` 包含，不增加生产 origin 配置或测试 HTTP 端点。
- [TCP host](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim/tests/tcp.rs)明确 Local、新绝对目录、无 PostgreSQL、Redis pool closed、禁用 folder/refresh；只在 private task-local seam 将外部 FreeBuff transport 指向合成随机回环 peer。这个 seam 不是全进程网络隔离的证明。
- [自动 TCP 测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim/tests/tcp/roundtrip.rs)覆盖真实 HTTP/read/end/final refund 0、持久历史、完整配置/revision 保留、旧 Grant 失效、重新取得 Grant，以及重建后重复 end 只返回历史而不新增上游请求。
- [显式 browser host](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim/tests/browser.rs)默认 ignored，只在明确新 root 下执行；文件式 sequence 命令仅核对/控制本次 host，不提供虚假管理响应。取消、结束、重建三个 checkpoint 均由后端对实际 SQL 和合成 peer hits 断言。
- [seed](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim/tests/tcp/seed.rs)正常注册后只准备原 pending claim；不发送 admission POST，不新增 consent 绕过入口。所有临时数据、脚本、构建和日志在本批 `linshi`，不读取用户真实 `.ng` 或历史凭据。
- 正常停止先 drain/await HTTP，再请求 shutdown、关闭 FreeBuff executor/Local SQLite，最后 await 合成 peer，全部完成后才写 `completed.json`；异常 Drop 的 abort 不冒充正常 join。

## 3. 可复核结果

证据根：[gateway-freebuff-claim-local-ui-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-claim-local-ui-20261107)。

| 检查 | 结果与边界 |
| --- | --- |
| 新自动真实 TCP | `tcp-v1.json/.log`：1 passed，包含一次同 root/port 重建；初次 lib test 编译约 14m53s，连同运行/回执约 899s，不据此推断热构建提速 |
| FreeBuff 定向库测试 | `freebuff-v1.json/.log`：317 passed / 0 failed / 2 ignored，其中包含上面同一个新 TCP 测试，不重复累计 |
| LocalRuntime 相邻选择 | `local-runtime-v1.json/.log`：`local_runtime::` filter，49 passed；与 FreeBuff 选择有交集，不代表不同 filter 的旧 50 项被删除 |
| 显式 browser host | `browser-host-v1.json/.log`：ignored host 显式执行 1 passed；真实正常 main 页面参与，不是 API client 代替浏览器 |
| 去重结果 | 按同一 lib harness 与完整测试名为 358 passed / 0 failed / 1 ignored；2 项新增已实际通过。剩余 ignored 是旧登录 browser host，本批不重跑 |
| 正常 main 构建 | `web-build-v1.json/.log`：隔离构建 exit0，15 项 bundle 文件有 hash；`cleanDistPath:false`，未使用会清理 live dist 的 publisher |
| 浏览器 HTTP | `browser-http.jsonl`：3 次 claim read、1 次 exact end、2 次 verify 和 2 次 confirm-secret-access，均 200；这是浏览器流量，不把后端 seed/独立旧 Grant 检查算浏览器动作 |
| 后端实况 | `browser-runtime/completed.json`：取消零上游、唯一签名 DELETE、active0/history1/refund0、同 root/port/revision 重建、旧 Grant 拒绝、joined=true |
| 页面证据 | `ui-original.jpg`、`ui-confirmation.jpg`、`ui-ended.jpg`、`ui-history-before-rebuild.jpg`、`ui-final.jpg` 和两个 DOM 快照；最终截图 1280×720，当前 warn/error 捕获为空，不代表所有浏览器诊断全覆盖 |
| 静态检查 | 5 项 Rust 官方 rustfmt、checker31/31、ratchet4861文件通过，governed source >700=0；14 项不可变 runtime 资产分列。未修改 baseline/exceptions |
| 来源与差异 | 8 个执行阶段各5175项所选输入与终态逐项同hash。只有既有测试接线变化及4项新测试源；直接preimage、UTF-8无BOM、范围diff/独立新增文件空白检查和公共guard/根报告原bytes保护见 `audit.json`、`scoped-diff.patch`、`diff-check.json` |

本批未改生产或前端源码。上一批正常/代表性关闭生产 lib 检查仍可复用：5171项旧输入与终态只差本批 cfg(test) 文件，4项新输入也全部在该测试目录；既有前端 typecheck/101项组件测试所选 `apps/desktop` consumer 输入未变，因此不无故重跑。精确差异见 `reused-checks.json`，不声称本批重跑了这些检查或完整 feature 矩阵。所选输入 hash 不等于全部环境、依赖安装、宏动态输入和外部共享 UI 均被冻结。

5份Rust源码最大212有效行，4个新增文件分别212/108/70/46；唯一既有 `tests.rs` 为126→128。已复核新 fixture 的输入/秘密、loopback边界、无无限重试、bounded peer hits/body、取消与任务/SQLite资源生命周期；没有放宽生产鉴权、Grant/revision、receipt/CAS、公共guard或迁移存储版本。

## 4. 保留的工具失败与清理事实

本批 Rust/浏览器行为没有失败用例。一次性 `audit.py` 初版误把 `git diff --no-index --check` 的正常内容差异 exit1 当成空白错误；实际 stdout/stderr 均空。只修正临时审计脚本的退出码解释，没有修改测试或生产语义；终态逐文件检查通过。

临时静态 helper 最初以非 PTY 启动，收尾 `write_stdin("stop")` 返回 `stdin is closed`。后端 host 与合成上游已正常 await/join 并退出0；静态 helper 没有获得优雅 stdin stop。核对 PID43868 的 executable/完整本批脚本路径/创建时间后，只终止这一个自建进程；不称它优雅 join。`process-closeout.json` 确认 PID23952/43868 已不在，60103/57210 无 listener，测试页已关闭；浏览器SQLite/脚本/产物现场保留，未清除该浏览器现场或操作无关进程；自动TCP测试只按既有fixture规则清理其本批linshi临时root。

## 5. 下一接力与完成口径

退役的缺口只有“正常账号 UI 原claim read→显式确认/end→历史重读→同目录 runtime 重建”的上述联合场景。不是新购买、实时公共业务、Server/PG、独立进程/EXE 或真实平台/退款验收。

下一步仍接**由真实公共调用触发的 server-side pending/逐次价格余额确认及 caller/receipt/candidate 绑定**，复用既有 quote/claim owner，不新增闲置购买按钮。public closeout/下一次购买、完整root/harness/工具/子agent、多步与取消费用FINISH、实时公共三协议、持久run/远端lease/probe/完整取消继续；新设备/目录应用的正常UI→真实管理TCP也不能由本次原claim流程冒充完成。

累计模板仍17，无新增整卡 `offline_ready`；其他渠道及P50/P60/P70和总目标均未完成。无真实凭据/供应商/Redis/Docker/现有服务变更，不复用已消费的P34单容器授权。

![同目录 runtime 重建后，正常账号界面读取到原会话结束历史与零退款](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-claim-local-ui-20261107/ui-final.jpg)
