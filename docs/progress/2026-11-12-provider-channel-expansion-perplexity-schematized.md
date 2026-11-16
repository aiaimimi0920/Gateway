# Perplexity 有限 markdown 增量响应实现

日期：2026-11-12。R12/W08 的入站解析增量；不是完整渠道、P50/P60/P70 或真实平台验收完成。

## 实际改动

- 新增独立 `src/protocol/perplexity_web/schematized.rs`：仅接受单一 `ask_text_0_markdown` 的已证实根 replace 和连续 chunks add。严格检查 op/field/path/type/索引、4096 chunks/patches 与256KiB正文，不照搬参考的忽略 op、稀疏写入和任意 String 转换。
- 接入原 `Decoder`，不改请求 concise/turbo、URL、材料、模型策略或启用 `use_schematized_api`。这证明可处理有限入站格式，不证明请求当前平台必然返回该格式。
- PENDING 始终只交付可替换 snapshot。只有当前事件 COMPLETED 且 materialized chunks 非空、可选 answer 一致才发 `Update::Complete`；缩短/改写不损失。EOF、final/text_completed 标志、旧 end_of_stream 不能完成部分 patch。
- auto 与新 profile 不混用，legacy text/多答案 track/workflow 拒绝。首次独立 workflow 事件也会进入拒绝路径，不能先被旧 unknown-extension 分支忽略再切入新 profile。
- 沿用原有 HTTP/deadline/permit/stream tracker/失败清理，没有新后台任务、缓存或重试。来源仅取本事件既有 web_results，访问 token 不输出，usage 保持未知。

公开来源与 MIT 许可调查见 [固定合同](../plan/provider-perplexity-schematized-contract.md)；本实现自行按有限协议编写，没有复制或运行参考实现。现有 auto/legacy 支持保持；未支持的复杂格式不静默降级。

## 红绿与检查

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-perplexity-schematized-20261112`。

1. 新增最初四项 parser 回归，未接新模块时实际运行：0 passed / 4 failed，均因旧 `perplexity_web_unsupported_patch` 拒绝正常初始化；原始输出存于 `red.log`。
2. 第一版接线后 Perplexity 过滤范围58 passed；只读交叉检查发现前置独立 workflow 仍可被旧解析器忽略，补分派与负向断言后完整重跑。
3. 最终 `green-final.log` / `exit-final.json`：**58 passed / 0 failed / 0 ignored**，不是累加两轮结果。包括5项新 parser 测试和1项新公共 pipeline 测试（内部执行 Chat/Responses × buffered/SSE 四组合）。最终 lib test 编译22m31s，测试1.99s；不以此承诺后续构建速度。
4. 新公共测试使用真实 loopback 合成上游、正常公共 router 与 Local SQLite，验证 partial 不进标准正文、最终缩短正文、完成审计、唯一正确出站、秘密不泄漏、上游持续保持 socket 时仍及时释放连接。下游为 router oneshot，不冒称真实客户端 TCP/浏览器/原生包。
5. 原有相邻错误、deadline、EOF、Drop/permit 与不重试测试包含在58项内。未执行全仓所有测试、真实平台、付费测试或正式发布。
6. checker31/31、最终 ratchet（5480 files，14 immutable runtime artifacts，governed source >700=0）、五文件官方 Rustfmt、`git diff --check` 通过。工作区无关 CRLF 提示保留，未修改 policy/baseline/exceptions。
7. 五个修改范围文件的最终输入哈希 `scope-final-inputs.json` 在测试后逐项相同；这是范围来源核对，不代表整个 dirty 工作区无其他并行变动。

有效行：decode 140→159，mod 54→57，新状态模块163、新测试132，公共live测试226→289（单一公共流测试职责）。全部低于500，不需要例外。mod/live 的前值按精确新增接线/测试段扣除重建，不冒称 Git HEAD 是本轮前像。

生产非测试编译 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 已通过（exit 0，6m10s），见 `production-check.log` / `production-exit.json`。保留111项既有/工作树 warnings，不宣称 warning-free；未改 feature 图，不宣称完整 feature 矩阵已验证。

## 剩余边界

未增加主动 schematized 请求、通用 JSON Patch、多轨/workflow、text_completed-only、多轮、Research、附件、模型目录或 session refresh。没有真实账号/当前平台 wire/权益费用证据。请求与鉴权不变，不操作旧实例或现用数据，不提交、推送或重新出包；上一轮 devtest 包不含本轮与前轮 Grok UI 修复。模板数17、不新增整卡 offline_ready，P50/P60/P70继续。
