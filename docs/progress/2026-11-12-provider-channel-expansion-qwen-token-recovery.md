# R07：token 欠额后的追加额度与恢复调用

日期：2026-11-12（Asia/Shanghai）。仅测试与文档改动，无生产计费、协议或路由修改。

## 同一 Key 的连续流程

新增1项测试，循环 Chat/Responses × buffered/SSE 四种组合。每个组合使用生产build_app_state、隔离SQLite、正常管理创建的同一Key，完整执行：

1. 初始购买总量/余额10000，成功请求报告10001 tokens，结算为-1。
2. 欠额后再次请求返回429/token_balance_exhausted，上游仍只有初次create+completion。
3. 正常余额管理接口仅发送 `tokenDelta:1`，不设置totalTokens/remainingTokens。总量10001、余额0；再次请求仍429，上游不增加。
4. 同接口仅发送 `tokenDelta:10001`，总量20002、余额10001；公共请求恢复成功，再报告10001 tokens后余额0。
5. 再次请求429，最终总量仍20002、余额0。两次成功累计消耗20002，与累计购买一致；没有通过覆盖余额消除既有消费。

最终恰好5条审计：2条completed各保存2/9999/10001、正确Key/exact/native model；3条failed无usage。上游总共4次调用（两个create+completion），指定Bearer/Cookie/UA保持，三次拒绝不新增上游。管理追加额度本身不产生模型调用审计。

这是管理接口追加token额度，不是支付订单、现金充值、支付回调或供应商退款验收。

## 代码与检查

- 新增recovery.rs 116有效行；fixture194→205，新增正常管理tokenDelta方法；父prepaid116→118；overage124不变，仅将既有settled检查开放给同级测试复用。
- locked/offline、runtime-defaults、4 jobs lib构建通过；Qwen prepaid共13 passed、0 failed、0 ignored，包括新1项及原12项。未重复未改动的Grok/前端测试。
- checker31、ratchet、rustfmt、限定whitespace与UTF-8无BOM通过；无baseline/exception修改。沿既有超时、异步审计等待和资源关闭机制，无生产测试特例。
- 构建前后仅并行 `src/protocol/lobsterai/inference_tests.rs` 漂移，本批直接输入不变。结果绑定冻结EXE，不覆盖其他工作线最新修改。
- Cargo compiler-artifact定位并复制测试EXE，源/副本哈希一致：`1f5c5ee5662162c13d5014d1ab4cb642e1afbf49a49fdc7f8054a82777b20f88`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-token-recovery-20261112`，包含inputs.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、checker.log、ratchet.log、resource-final.json。TEMP/TMP、合成配置和SQLite均在该目录；测试进程退出，fixture/peer正常结束。

## 范围结论

补齐本地单Key欠额→追加额度先抵欠额→恢复调用→再次耗尽拒绝的连续合同。仍未覆盖支付/现金业务、多个已获准请求同时超额结算、PG/Server、崩溃预扣恢复、真实TCP计费断连、供应商计费及成品包。下游仍为router oneshot，上游是合成peer。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、出应用包、提交或推送。
