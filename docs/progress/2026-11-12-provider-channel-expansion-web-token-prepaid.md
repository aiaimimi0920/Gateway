# R03/R07：Grok 与 Qwen 普通 Key token 预付费合同

日期：2026-11-12（Asia/Shanghai）。本批仅扩展已有公共 Key 测试 fixture 和新增 token 测试，不修改生产计费规则。

## 既有规则与实际结果

源码 `pipeline/stage_finalize/buffered.rs` 和 `streaming.rs` 的既有规则为：成功有 usage 时按 total_tokens 结算；成功无 usage 时保留 quota_pre_deducted_tokens；失败/取消走退款 owner。保留预扣不等于上游已报告使用量，不能写入 audit 冒称实测 tokens。

沿生产 build_app_state、隔离 SQLite、正常管理创建 normal Key 的已有 fixture，通过余额管理接口设置 token_prepaid，总量及余额为 10000，再调用公共 Chat/Responses。没有绕过路由直接调用 reserve/settle/refund。

新增 6 项测试共 18 组合：

- **成功流**：Grok 两入口共 2 组合；Qwen 两入口 × 有/无 usage 共 4 组合。在公共响应 body 尚未读到终态前读取余额，必须已有大于零的预扣。收完合成答案和正确完成标记后，Qwen 有 usage 的余额精确为 9995（2+3=5 tokens），无 usage 的 Qwen 与 Grok 均保持先前观察到的预扣后余额。唯一 completed audit 的 usage 分别为 2/3/5 或全部 None。
- **失败**：两渠道分别 Chat/Responses × buffered/SSE，共 8 组合。缺终态的关闭/截断不得伪造完成，唯一 failed audit，最终余额恢复 10000。
- **取消**：两渠道分别 Chat/Responses 流式，共 4 组合。上游保持打开，读到部分答案但未完成，确认余额已下降；drop body 后观察上游关闭，唯一 cancelled audit，余额恢复 10000。

购买总量保持 10000，每次仍只发送原有两次合成上游请求，无重放或账号切换。审计归属对应 Key/exact 账号和正确 native model。既有 message_prepaid 的 6 项也在同一冻结 EXE 下通过，防止扩展 fixture 破坏按次数合同。

## 检查与证据

- locked/offline、runtime-defaults、4 jobs 的 lib 测试构建通过。
- `web_pipeline_tests::prepaid` 实际运行 12 项，12 passed、0 failed、0 ignored；其中新增 token 6 项、既有 message 6 项。未无故重跑未修改的前端或网页宿主。
- checker31、ratchet、rustfmt、限定 whitespace 和 UTF-8 无 BOM 检查通过；baseline/exception 未改。
- Grok prepaid.rs 87→89、fixture.rs 153→175、新 tokens.rs 101 有效行；Qwen prepaid.rs 110→112、fixture.rs 169→191、新 tokens.rs 104 行。
- 编译前后输入核对只有并行 `src/protocol/lobsterai/inference_tests.rs` 漂移；本批直接测试、公共计费和两渠道生产输入无变化。证据绑定冻结副本，不覆盖其他工作线的新修改。
- 由 Cargo compiler-artifact 定位并复制测试 EXE，源与副本哈希一致：`6ee3082f8974a9949a5b020d280ddc19b6ce868d1d0300cc8e28c500257862b1`。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-token-prepaid-20261112`，含 inputs.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、checker.log、ratchet.log、resource-final.json。TEMP/TMP 和合成数据留在该目录。每个 fixture 释放状态引用、请求 shutdown、关闭 SQLite/freebuff，合成 peer 正常结束；测试进程已退出。

## 验证边界

本批新增 token **成功路径仅覆盖流式**；buffered 成功的 token 余额结算未在此新增端到端证据。失败同时覆盖 buffered/SSE。下游是 router oneshot/body drop，不是新增真实 TCP 计费断连测试。未知 usage 保留预扣的既有行为已证实，不代表其等于真实消费或新的产品定价决定。

不覆盖余额不足/并发竞争、实际用量高于预扣的欠额边界、现金账本与倍率、供应商真实扣额/远端取消退款、PG/Server、崩溃中的预扣恢复、完整服务启动或成品包。新增模板仍 17，无新增整卡 offline_ready，P50/P60/P70 未完成。未使用真实凭据/付费上游，未修改在线实例、提交或推送。
