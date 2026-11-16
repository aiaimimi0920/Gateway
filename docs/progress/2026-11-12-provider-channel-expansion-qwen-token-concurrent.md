# R07：两个已获准请求的 token 超额合并结算

日期：2026-11-12（Asia/Shanghai）。仅新增测试模块、声明和文档，没有生产实现修改。

## 实际验证

新增1项测试，覆盖Chat/Responses × 先后消费/同时消费两个流式响应，共4组合。复用生产build_app_state、本地SQLite、正常管理创建的同一Key，初始token余额/总量10000。

1. 用tokio::join!同时推进两个公共请求，必须都200；在消费body终态前，余额为10000减去两份预扣。
2. 两个合成上游各报告input=2/output=9999/total=10001。
3. 先后消费分支：先读完第一个响应，等待其结算后余额必须为-1减去第二笔仍占用的预扣；再读完第二个响应。
4. 同时消费分支：用tokio::join!消费两个响应，不依赖谁先完成。
5. 两分支最终余额均精确为-10002，购买总量保持10000。两条不同ID的completed审计各保留准确2/9999/10001、同Key/exact/native model，证明没有用后完成者覆盖先完成者的消费。
6. 第三个公共请求429/token_balance_exhausted，余额仍-10002；审计共2成功/1失败，上游总共4次（两组create/completion），拒绝请求无新增上游调用，指定Bearer/Cookie/UA保持。

测试在current-thread Tokio内并行轮询request/body futures，属于单实例本地的重叠请求与结算合同，不代表多线程压力、跨进程或PG隔离级别证明。只有流式响应，没有把结果外推为所有并发组合。

## 检查与证据

- locked/offline、runtime-defaults、4 jobs lib构建通过；Qwen prepaid共14 passed、0 failed、0 ignored，含新增1项及原13项。
- checker31、ratchet、rustfmt、限定whitespace与UTF-8无BOM通过，无baseline/exception改动。
- 父prepaid有效行118→120，新concurrent_usage112行。复用正常管理/公共入口、既有审计等待与资源释放；中间余额等待3秒，总测试沿45秒预算，无后台无限任务或真实供应商调用。
- 构建前后仅并行 `src/protocol/lobsterai/inference_tests/fixture.rs` 和 `src/protocol/lobsterai/inference_tests/probe_scheduler.rs` 漂移，本批直接输入未变。结论绑定冻结EXE。
- Cargo compiler-artifact定位实际测试EXE，源/复制品哈希一致：`d016b01b447c0c431e825a27878e463ebaaceff4d66d6bba3ac3cea984b162e9`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-token-concurrent-20261112`，含inputs.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、checker.log、ratchet.log、resource-final.json。TEMP/TMP和合成数据均在该目录；测试进程退出，fixture/peer正常结束。

## 剩余范围

本批补齐同Key两个获准流式请求超额累计、保留另一预扣和合并欠额后拒绝新请求。未覆盖混合成功/取消并发、PG/Server、多线程压力、崩溃预扣恢复、现金账本/倍率、真实TCP计费断连、供应商收费或成品包；不因这些未验边界推导生产缺陷。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、出应用包、提交或推送。
