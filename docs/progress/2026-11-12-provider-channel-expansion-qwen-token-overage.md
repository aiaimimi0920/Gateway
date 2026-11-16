# R07：Qwen 已报告用量超预扣及欠额结算

日期：2026-11-12（Asia/Shanghai）。本批只新增 overage 测试模块及声明，不修改生产计费实现或产品定价。

## 既有行为的端到端验证

`access_balance/policy.rs::settle` 对 token_prepaid 按 reserved-actual 调整余额，不将负余额钳制为0；后续 reserve 按剩余是否足够预扣决定准入。本批沿生产build_app_state/SQLite、正常管理Key/余额、公共router与合成Qwen上游验证该行为，不直接调用结算方法。

新增2项测试，共8组合（两种用量 × Chat/Responses × buffered/SSE）：

- 上游报告input=2/output=99/total=101，明确高于当前请求预扣量；成功后余额从10000精确结算为9899，购买总量仍10000。
- 上游报告input=2/output=9999/total=10001，超过剩余余额；请求仍按真实成功终态完成，余额精确为-1，没有截成0或只扣预估量。随后同入口再次请求，必须429/token_balance_exhausted，余额仍-1，上游调用数仍为初次create+completion两次。最终恰好一条completed和一条failed审计，拒绝请求usage为空，不抹掉既有欠额。

流式路径在消费终态前另确认余额已减去估算预扣；完成审计保存准确的2/99/101或2/9999/10001，归属正确Key/exact账号/native model。buffered响应JSON也核对total_tokens。各路径仍只有一次实际合成生成链，没有重放或换号。

这是已存在的本地token结算语义，不是新的现金透支授权，也不将负token余额解释为现金负债。没有给Grok制造它本来没有的usage报告。

## 检查与证据

- locked/offline、runtime-defaults、4 jobs lib构建通过。检查类型时将新测试期望值显式转换为审计的i32类型，最终源码输入快照为inputs-final.json。
- Qwen prepaid共12 passed、0 failed、0 ignored，新增2项及原10项均通过。Grok及前端未改，未重复运行。
- checker31、ratchet、rustfmt、限定whitespace和UTF-8无BOM通过；无baseline/exception变更。
- 父prepaid有效行114→116；新增overage124行。测试使用合成固定用量、3秒审计等待和既有45秒总预算，沿既有fixture关闭资源。
- 最终源码输入快照与执行后比较无漂移。由Cargo compiler-artifact定位实际EXE，复制后源/副本哈希一致：`f988ea22be65e1810cd5fbc014341952e338f51c557b1a58264671553a28a6fc`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-token-overage-20261112`，包含inputs-final.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、checker.log、ratchet.log、resource-final.json。TEMP/TMP和合成数据均在此处，测试进程退出，fixture/peer正常关闭。

## 剩余范围

本批补齐Qwen单请求已报告usage高于预扣/余额及欠额后的下一次拒绝，不证明多个已获准并发请求的超额合并结算、欠额充值恢复、PG/Server、崩溃预扣恢复、现金账本/倍率、真实供应商计费或成品包。下游为router oneshot，不冒称新增真实TCP收费验收。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、出应用包、提交或推送。
