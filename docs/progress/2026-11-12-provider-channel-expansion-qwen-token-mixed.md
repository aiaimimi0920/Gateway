# R07：混合成功与取消的 token 结算隔离

日期：2026-11-12（Asia/Shanghai）。本批仅新增测试、调整测试fixture初始化和共享检查可见性，无生产计费修改。

## 实际验证

新增1项测试，共4组合：Chat/Responses × 先取消/先成功。同一Key初始token余额10000，两个流式请求均获准并各占用预扣。两个独立合成peer分别提供完整成功响应（usage2/9999/10001）及只有增量、没有终态的保持连接响应。

- 首先确认两个200，余额减去两份预扣；取消候选实际交付“合成答案”且无完成标记。
- **先取消**：drop候选body、观察其peer连接关闭，余额必须恢复为10000减去仍待成功请求的预扣；再完成另一请求。
- **先成功**：完成报告10001 tokens的请求，余额必须为-1减去仍待取消请求的预扣；再drop候选并观察连接关闭。
- 两分支最终余额均为-1，总量保持10000；两条审计恰好一completed、一cancelled，归属同Key/exact/native model，成功usage准确，取消usage保持None。
- 后续请求429/token_balance_exhausted，不抹掉-1欠额；最终2条原审计加1条failed。两个peer各只有一次create+completion，材料正确，无重放或额外上游调用。

仅current-thread Tokio内的单实例重叠流式合同，不是多线程压力、PG或跨进程隔离保证；取消仅代表本地预扣释放，不承诺供应商远端停止/退款。

## 首次失败及责任层修正

初次运行14 passed/1 failed：新测试在正常管理操作之后尝试Arc::get_mut替换HTTP client，AppState已不可独占，返回None而panic。此时尚未进入混合请求业务验证，不能归为生产计费缺陷。

修正fixture增加with_client入口，在任何管理router共享状态之前设置client；原new仍使用原peer client。双peer测试使用无代理的测试client和已有强制task-scoped回环target，缺scope仍拒绝，不允许真实网络回退。未改生产可见性、取消或计费行为。原失败日志和EXE保留。

## 最终检查

- 第二次locked/offline、runtime-defaults、4 jobs lib构建通过；Qwen prepaid最终15 passed、0 failed、0 ignored（新增1及原14）。
- checker31、最终ratchet、rustfmt、限定whitespace、UTF-8无BOM通过，baseline/exception未改。
- 父prepaid120→122有效行；fixture205→208；concurrent_usage112不变，仅开放两个同级测试检查；新增mixed_outcome103行。读取有3秒/16KiB上限，沿45秒总预算及既有资源关闭。
- 最终inputs-final.json对应源码输入无漂移。Cargo compiler-artifact定位并复制tests-final.exe，源/副本SHA256一致：`3fa34eb2fb289c5bfcfb9d4abaa1fc27e25f68826ab206ae83674d121be0ac26`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-token-mixed-20261112`。最终来源为build-2.jsonl、tests-final.exe、prepaid-final.log、inputs-final.json、input-drift.json、ratchet-final.log；另有checker.log和resource-final.json。TEMP/TMP及合成数据留于此，测试进程退出，最终fixture和peer正常关闭。

## 剩余范围

本地token成功、失败、取消、准入不足、精确阈值、重叠准入、超额、欠额追加恢复、双成功及混合成功/取消现均有上述限定证据。不能据此宣称PG/Server、崩溃预扣恢复、现金账本/倍率、真实TCP计费断连、供应商收费或成品包完成。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例、出应用包、提交或推送。
