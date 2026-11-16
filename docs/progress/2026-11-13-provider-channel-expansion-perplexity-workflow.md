# Perplexity 有限 workflow 最终文本

日期：2026-11-13。R12/W08 的有限入站兼容增量，不是多轮功能或整卡验收完成。

## 改了哪些内容

- 新增独立 `src/protocol/perplexity_web/workflow.rs`。只解析完整终态中的唯一明确文本 answer；诊断标题、思考/非文本 item、重复答案、混合轨道、未知版本、未完成步骤、正文/chunks 冲突均拒绝。
- PENDING patch 只检查已观察的操作/path/类型，不应用或保留状态树，不输出中间内容。EOF、end_of_stream 或单个 final 标志不构成成功。decoder 首次识别便锁定 profile，阻止前后切换。
- 复用现有响应/帧/事件/deadline、HTTP body ownership 和 Complete 路径，无新任务、队列、重试、账号轮换或持久数据。
- 新增7项解析/边界测试及1项公共链测试，公共链内部覆盖 Chat/Responses × buffered/stream 四组合；复用原合成回环设施，两项测试辅助函数仅扩大到父测试模块可见。
- 保存 [脱敏观察合同](../plan/provider-perplexity-workflow-contract.md)，更新 [产品合同](../provider-perplexity-web.md)。不复制公开参考实现。

## 真实观察与本地测试边界

11月12日用户授权两次官网问答，第二问正确返回“蓝色纸船”；只第二轮有完整响应缓冲观察。原始 token/正文未落盘，浏览器观察内存已清空。本批没有新增真实请求。

测试是按已观察字段人工合成，不是原始抓包回放。真实网页为 copilot/turbo + schematized，Gateway 请求仍为原单轮 concise/turbo；本次只证明新的入站解析和公共 pipeline，不能声称实际 Gateway 账号调用或续接成功。私有 token、UUID 不存储、不返回。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-perplexity-workflow-20261113`。

- `red.log`：先运行新增3项测试，1 passed / 2 failed；合法 workflow 被旧 schematized owner 拒绝，命中预期缺口。
- `green.log`：首轮实现回归因新增测试未引用已有 `values` / `wait_closed` 编译失败（E0425）。只修测试可见性/导入，没有放宽生产协议。
- `green-final.log`：68 passed / 0 failed / 0 ignored，包含新增8项与既有Perplexity相关测试。公共四组合均验证最终文本、正常终态、token不回显、精确账号审计和HTTP body释放；未知usage保持未知。
- `checker.log`：行数检查器31 passed / 0 failed。
- `ratchet-final.log`：有效行ratchet通过，没有修改policy、baseline或例外。
- official rustfmt scoped check、仓库 `git diff --check`、新增/未跟踪源码 `git diff --no-index --check` 通过。全仓有其他文件CRLF提示，未改写这些文件。
- `source-hashes-final.json`：8个本批源码/测试文件SHA256；测试完成后逐项核对一致。
- `production-check.log`：非测试生产lib检查通过，exit 0；111项现存warning保留，没有为本批顺带清理或自动cargo fix。耗时10m27s，包含共享锁等待。

测试命令（锁定、离线、4 jobs）：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 4 perplexity -- --test-threads=2
cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4
```

两次编译含共享构建锁等待；最终测试编译显示17m06s、测试执行1.83s，不能当成纯热增量性能或承诺提速。

## 有效行与审查

| 文件 | 本轮前 | 本轮后 |
| --- | ---: | ---: |
| decode.rs |159|171|
| perplexity_web/mod.rs |57|62|
| workflow.rs |新增|170|
| workflow_tests.rs |新增|118|
| workflow_boundary_tests.rs |新增|107|
| perplexity_web_pipeline_tests.rs |229|231|
| perplexity_web_live_pipeline_tests.rs |297|297|
| perplexity_web_workflow_pipeline_tests.rs |新增|78|

解析只遍历有界JSON；正文采用strip_prefix比较，不额外拼接大字符串。所有错误使用固定文案，不输出原始事件；成功仅构造text/sources。没有文件路径、命令执行、网络目的地或凭据格式变化。独立只读审查未发现最终接线的确定缺陷，不替代上述运行证据。

## 未完成与未执行

未实现多轮账号/调用者绑定、token失效/轮换、取消后续接状态、通用JSON Patch、多答案、Research、多模态或workflow引用。Suno原账号资格阻碍保持不变，不重复授权或绕过。

未运行付费上游测试、全量Rust回归、桌面UI验收或正式发布审计；未出新EXE/包、未更新运行实例、未读取/修改真实凭据或数据库、未提交/推送。此前手测包不包含本批修改。P50/P60/P70和总目标仍执行中。
