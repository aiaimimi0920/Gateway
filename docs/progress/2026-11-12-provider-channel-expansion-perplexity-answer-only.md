# Perplexity 完整终态 answer-only 兼容

日期：2026-11-12；承接有限 schematized 入站解析，不是新增请求模式或整卡完成。

## 修改与合同

固定参考 `b71881de3ebb104e3e4962a603df712ffa397eb7` 的协议代码已记录缺少 chunks、只携带 answer 的 materialized 形式（来源见前批 schematized 合同）。旧实现只认 chunks，新增回归实际得到 `perplexity_web_invalid_schematized`。

现仅在 `COMPLETED`、markdown 的 chunks 字段确实不存在时，接受256KiB内的字符串 answer。正文仍须非空；显式 null/错误类型/空或冲突的 chunks 不走兜底。PENDING answer-only 不作为 chunk 初始化，不能为后续 add 虚构索引或靠旧 partial 完成。原协议状态、错误、来源、取消、未知usage与秘密隔离不变。

修改 `schematized.rs`、`schematized_tests.rs` 与原公共 live pipeline 测试；没有新增状态 owner、依赖、请求参数或测试框架。有效行分别163→170、132→177、289→297，所有文件低于500。

## 实际验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-perplexity-answer-only-20261112`。

- `red.log` / `red-exit.json`：新增成功路径修复前1 failed/0 passed，明确复现旧代码拒绝该终态。
- `green.log` / `green-exit.json`：最终 Perplexity 过滤范围 **60 passed / 0 failed / 0 ignored**。包括2项新增测试；不是与前批58项相加。
- 新成功用例覆盖直接完整返回与先有partial再完整返回；负向用例覆盖空白、null、数字、超限answer，显式坏chunks、冲突正文和未完成状态。
- 原公共测试扩为 Chat/Responses × buffered/SSE × chunks/answer-only，共8个内部组合；正常router、真实合成loopback上游与Local SQLite验证最终正文、completed审计、无secret回显、保持socket时及时释放原连接。下游仍为oneshot，不称客户端TCP或真实平台验收。
- 测试使用锁定离线依赖、runtime-defaults、4 jobs；仅根包测试调试信息通过 `--config profile.test.package.gateway.debug=0` 调整，不改仓库配置/依赖或测试断言。最终编译13m50s，执行2.88s，不据此泛化构建提速。
- checker31/31、ratchet（5485文件，14 immutable runtime artifacts，governed source >700=0）、范围Rustfmt、`git diff --check`通过。范围输入hash在运行后保持一致；不宣称并行dirty工作树整体无漂移。
- 测试程序SHA256：`3fedcb6bb675b26bff6d39fb238d94cd792bf1a2eef25af69147dfd5fa9a30ad`；这是测试EXE，不是用户手测包。

生产非测试编译 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过（exit 0，2m23s），证据为 `production.log` / `production-exit.json`。保留111项warnings，不宣称warning-free或完整feature矩阵通过。

## 边界

不主动开启schematized请求，不支持多轮/Research/workflow/通用patch或新模型目录；当前平台、真实账号/额度仍未验。本轮无供应商调用、费用、运行实例/现用数据修改、提交/推送或新包。上一轮devtest包保持不变且不含这些源码增量，P50/P60/P70仍未整体完成。
