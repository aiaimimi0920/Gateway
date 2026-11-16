# R03/R07：token 精确阈值与重叠请求准入

日期：2026-11-12（Asia/Shanghai）。本批只新增 Grok/Qwen 的 admission 测试模块及父模块声明；复用既有生产本地状态、正常管理 Key/余额和公共请求 fixture，未修改生产实现。

## 新增证据

新增4项测试，共12组合：

- **精确阈值8组合**：两渠道 × Chat/Responses × buffered/SSE。余额设为单文本请求的既有估算预扣量（12 UTF-8 bytes按现有估算公式及配置比例），均正常返回合成答案及完成终态；采用未知usage响应，审计保持None，成功后余额精确为0。每次只有既有两次上游调用，账号/native model与Key归属正确。
- **重叠请求4组合**：两渠道 × Chat/Responses。余额只够一次预扣，以tokio::join!并行轮询两个公共流式请求future。获准的上游流不发送终态，确保请求仍占用预扣；必须恰好一个200和一个429/token_balance_exhausted。读取获准请求的实际增量后余额为0，合成上游仍只有一次生成链，证明拒绝请求没有误退另一请求的预扣。随后drop获准body，上游关闭；最终审计恰好一条failed、一条cancelled，都归属同Key且usage未知；余额精确恢复原预扣量，没有重复退款。

该重叠测试在current-thread Tokio中并行轮询两个请求，验证单实例本地状态的重叠准入，不冒称多线程压力测试、跨进程或PG并发证明。没有断言哪个请求必须获准，避免依赖调度顺序。

## 检查

- locked/offline、runtime-defaults、4 jobs lib构建通过。
- 公共Key prepaid测试共20 passed、0 failed、0 ignored，新增4项及原16项均通过。
- checker31、ratchet、rustfmt、限定whitespace、UTF-8无BOM通过；baseline/exception未变。
- Grok父prepaid有效行89→91、新admission134；Qwen父112→114、新admission136。无生产路径特例，增量读取有超时及16KiB上限，异步审计有3秒预算，沿既有fixture关闭SQLite/freebuff和peer。
- 构建前后仅并行 `src/protocol/lobsterai/inference_tests.rs` 与 `src/protocol/lobsterai/inference_tests/fixture.rs` 输入漂移，本批直接输入不变。结论绑定冻结测试EXE，不覆盖其他工作线最新修改。
- Cargo compiler-artifact定位真实EXE，复制后源/副本SHA256相同：`28b13047668811e0f50856be27add813c34c0530d5c8e85a531d68e9d0802db4`。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-token-competing-20261112`，含inputs.json、input-drift.json、build.jsonl、tests.exe、prepaid.log、checker.log、ratchet.log及resource-final.json。TEMP/TMP与合成数据均在此处；测试进程退出，各fixture和peer正常结束。

## 剩余范围

本批关闭精确预扣阈值与单实例两个重叠请求争用最后一份额度的限定证据空白。实际用量高于预扣/余额、现金账本/倍率、多线程或跨进程压力、PG/Server、崩溃预扣恢复、真实TCP计费断连、供应商扣额/退款及成品包仍不由本批证明。精确阈值成功测试使用未知usage，不冒称有usage后的欠额处理。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未使用真实凭据/付费上游，未修改在线实例，未出应用包、提交或推送。
