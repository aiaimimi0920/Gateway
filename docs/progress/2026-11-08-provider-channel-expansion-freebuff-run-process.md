# FreeBuff：原 run 六阶段独立进程强制退出验证

日期：2026-11-08（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承 dirty/untracked 修改保留；HEAD 不是本批完整来源凭据。

承接[原 run 事实持久化](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-run-journal.md)。本批只新增测试和回执，不改变生产 wire、存储格式、管理权限或任何真实凭据。状态：六阶段定向、相邻回归和收尾审计已完成；不是完整渠道或远端恢复验收。

## 1. 真实进程边界

新增 `claim_owner_tests/run_process.rs` 与独立 `run_process/child.rs`，沿已有 lib harness 的 `current_exe` 启动真正的 child process。child entry 默认 ignored，只有父测试以精确名字、隔离环境显式调用，不把空运行记作功能通过。

父进程创建唯一临时根和随机回环 peer，child 打开同一 Local SQLite，以固定合成账号准备原 claim、发送一次 admission，并沿生产 `ActiveAdmission::start_catalog_run`、模型解码与 FINISH 持久化 owner 执行。准备提交后有明确 ready/armed 握手，不靠固定 sleep 猜启动完成。

| kill 时的持久阶段 | 合成上游的实际边界 | 已观察 credits |
| --- | --- | --- |
| `start_pending` | START 已到达，响应 body 未交付 | 0，只指没有模型步骤，不代表购买免费 |
| `started` | 真实 START 身份保存后，尚未发模型 | 0，同上 |
| `model_pending` | 模型已到达，响应 body 未交付 | unknown，保持 null |
| `model_observed` | SSE 已交付真实费用观测，仍未到实际 EOF | 13，不冒称完整模型成功 |
| `finish_pending` | 原 FINISH 已到达，ack body 尚未交付 | 13，FINISH 仍未知 |
| `finish_accepted` | 同次 FINISH acknowledgement 和本地保存已完成 | 13，接受不当结算 |

每阶段同时等待管理 DTO 中的持久 phase 与 peer 请求数，再实际确认另一个 runtime 无法取得原 OS fence，才执行 child kill 和 wait。kill 后先确认原 response body 被释放，再关闭父进程旧 runtime、从原目录新建 runtime，逐字比较原 claim row、run row 与 seal，以及管理事实 DTO。

随后仅显式 heartbeat 原 attempt；新 observation 不授予第二次 START。新 START 被 `RunJournalUnavailable` 拒绝，peer 请求数只增加这一次 heartbeat，没有 FINISH、end、重买或隐式重放；原行和事实不变。正常路径全部 child wait、peer body drop、peer close/join，失败 unwind 仍有 `kill_on_drop` 与 peer Drop abort，失败现场不自动清理。

## 2. 远端恢复合同复核

本批重新计算并核对两份已归档固定公开源码的 SHA-256，见证据根 `source-references.json`。固定 commit 为 `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`，未重新联网，也不把固定客户端合同冒称当前线上事实。

- `sdk/src/impl/database.ts:408-473`：START 为 `POST /api/v1/agent-runs`，body 是 action/agentId/ancestorRunIds，没有发送前 client request ID、claim/attempt 或幂等键；只从返回值得到 runId。
- 同文件 `:475-531`：FINISH 使用原 runId 与累计 credits/steps，返回 void，不解析接受版本或结算 receipt。SDK 的 fetch retry 不是已证服务端幂等保证，Gateway 不跟随重放。
- `common/src/types/contracts/database.ts:60-79`：内部 GetAgentRunFromId 类型只证到按已知 runId 读取 agent/ancestor/status；没有证到面向客户端的 URL/method/授权合同或累计金额，也不能找回未知 START 丢失的 runId。

因此，本批不编造查询接口、自动补 FINISH 或用 session heartbeat/end 代替 run 对账。未知 START/FINISH 的安全执行恢复仍待取得足够的远端合同证据；这是该能力的边界，不阻止其他无凭据开发继续。

## 3. 验证与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-run-process-20261108`。Cargo locked/offline、runtime-defaults、关闭 management-ui、jobs4，测试 threads1；TMP/TEMP/TMPDIR 均指向该根的 tmp。

| 检查 | 结果 |
| --- | --- |
| 六阶段真实 kill / wait 定向 | 1 passed，包含6次真实 child 强制退出；`process-focused` |
| FreeBuff 相邻回归 | 463 passed / 0 failed / 4 ignored；含本批1项，定向不与总数重复相加 |
| 三份 Rust 官方格式 | 通过 |
| checker tests / ratchet | 31 passed；4955文件通过，governed source >700 为0 |
| 最终来源、编码、diff 与资源 | 通过；见 `final-audit.json`、`scoped.diff`、`effective-lines.json`、`diff-check.json` 与 `resource-audit.json` |

本批没有生产/前端修改，不无故重复已有检查。正常/代表性关闭 FreeBuff 的生产 lib 编译及前端209项/类型检查，只能按实际消费输入同 hash 复用，不声称本批重跑。未改有效行 baseline/exceptions；根生成报告另存后从直接 preimage 恢复，不覆盖旧失败证据。

范围为3份 Rust 与4份文档，共7份 UTF-8 无 BOM 文本；唯一旧 Rust 接线229→230有效行，新增两份分别197/65有效行。6阶段检查各4420项选定输入前后不变，并与最终快照逐项相同；相对原4418项仅多两份 test 文件和原 cfg(test) 模块声明，生产消费输入未变化。实际 lib harness 的 SHA-256/bytes 与完整测试名另存；根报告恢复及原 legacy guard/baseline/exceptions 字节保护已核对。选定输入不是外部 path dependencies、node_modules、运行期资产和宿主的 hermetic 保证。

本批没有失败的行为测试或为了凑红绿而改生产语义；checker31项、格式与 ratchet 均正常通过。4个 ignored 为3个既有浏览器联合夹具及本批独立 child entry，不能把其计为正常测试执行；child entry 已被父测试在6个实际进程中显式使用。最终只读资源检查为精确 lib harness 0个活进程，未终止其他 PID。

## 4. 没有退役的边界

实际结束的是执行 owner 的独立进程，恢复读取是在既有父测试进程中新开的 runtime；不冒称第三个全新恢复进程、整个测试宿主/OS 重启、跨版本兼容或原生 EXE/整包验收。六个稳定持久边界不代替 SQLite commit/seal 写入指令内部的全部 crash 窗口；panic 兜底 kill 不冒称失败路径本身已有显式 wait 证据。

本批沿真实内部 owner 与原管理 DTO 验证，不是正常管理 HTTP/浏览器的新字段联合，也未独立重验全部 method/path/body/签名；原 wire 由相邻测试保护。没有真实供应商或远端 lease/幂等/settlement receipt。未知结果对账、持久 history/完整工具 child、其他公共协议/模态/参数、新设备/目录 UI 联合、Server/PG/Hosted、原生包及真实资格/额度仍未完成。

全局仍17个新增模板、无新增整卡 `offline_ready`；P50全范围无凭据集成/devtest、P60集中录制、P70逐渠道真实验收和总目标继续。本批不读取真实凭据、不启动 Redis/Docker、不操作既有实例，不重放或删除前批失败现场，没有出包、提交或推送。
