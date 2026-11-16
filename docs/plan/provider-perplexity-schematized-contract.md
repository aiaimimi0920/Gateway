# Perplexity schematized 响应：公开证据与实施边界

日期：2026-11-12。R12/W08 的联网只读调查；不是新响应格式已经实现或真实平台验收。

同日后续：有限单轨 markdown 入站解析已实现，最终58项定向检查及生产lib编译通过，详见 [实现回执](../progress/2026-11-12-provider-channel-expansion-perplexity-schematized.md)。本文下列“本轮/尚未实现”保留为调查阶段记录；实现没有开启主动 schematized 请求，也没有扩为 workflow 或多轮。

## 固定来源

本批匿名联网读取 GitHub API 与 raw 文件，没有登录或发送 Perplexity 请求，没有安装、运行或复制参考实现。搜索摘要仅用于发现，以下结论以实际下载的固定版本文件为依据。

- [已合并 PR #4001](https://github.com/cheaper-inference/OmniRoute/pull/4001)：GitHub API 确认 closed/merged；head 为 `artickc/OmniRoute@ab817382a430d4c7f1540323cbdb0f0b113966e0`，base 为 `6b8a122e8ff7318116385ec7d09889fc59b27b5d`。
- [PR 固定实现](https://github.com/artickc/OmniRoute/blob/ab817382a430d4c7f1540323cbdb0f0b113966e0/open-sse/executors/perplexity-web.ts)，对应 `tests/unit/perplexity-web.test.ts`。
- 当前默认分支经 API 解析为 `release/v3.8.54`，固定 commit `b71881de3ebb104e3e4962a603df712ffa397eb7`；[当前协议文件](https://github.com/cheaper-inference/OmniRoute/blob/b71881de3ebb104e3e4962a603df712ffa397eb7/open-sse/executors/perplexity-web/protocol.ts)，对应同路径测试文件。
- 两版本 LICENSE 均为 MIT，Copyright (c) 2026 diegosouzapw。当前仅行为调查，未复制受保护源码；后续采用须保留许可及修改说明，不能把代码许可当作 Perplexity 服务自动化授权。

现存快照目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-perplexity-protocol-20261112`；`pr.json` / `current.json` 保存仓库、完整 commit 与实际获取时间。

| 文件 | SHA256 |
| --- | --- |
| PR LICENSE / 当前 LICENSE | `5f3caa52b2c2ce5dfb40c85404a370d23b6dc3e26a1447d17dc7e56f773b91de` |
| PR executor | `948a845c4a19ddb426a8ac19b66c12fef1a5a7999cc8b5d0531b2c2321094079` |
| PR tests | `4556acce435f595c91bd39d09d2108bc215b696a4b1759257e733509d63be008` |
| 当前 protocol | `73e3747e1b85bf1e45910803ab0f3d99e39447f34b7b6da46312a74da078a0fa` |
| 当前 tests | `371d73a1934472cdaefa1313a811ea2a4131dfff5e20127949c9e081153d58ec` |

## 已获得的具体协议线索

1. PR 实现 425–442、测试 245–318 给出了 `field=markdown_block` 的具体序列：根路径 `replace` 携带 `chunks`，随后 `add /chunks/1`、`add /chunks/2`。因此“不存在明确 op/path 公开样例”的旧停点已解除，但这仅证明该序列，不证明任意 RFC 6902 操作。
2. PR 测试的 `ask_text_0_markdown` 在 `PENDING/final:true` 后仍有 `COMPLETED` 完整正文。`final:true` 不能单独触发成功。当前 protocol 975–993 仍保留这一判断。
3. 当前 protocol 413 设置 `use_schematized_api=true`，与 Gateway 当前 `request.rs` 的默认 auto 请求不是同一声明。不得仅放开 `snapshot.rs` 的 diff 拒绝就宣称兼容两套响应。
4. 当前 protocol 594–678 又有 `workflow_block`：step/item materialization、逐 item chunks 及 text。只有明确 `variant=answer` 的已初始化 item 可以接收 chunk；没有初始身份的 chunk 可能属于 thinking，不能当答案。来源称为 August capture，但本批没有原始平台录制，不将源码注释当独立实测。
5. 当前测试 944–1046 明确存在没有 `COMPLETED` 的 PENDING/text_completed 序列。当前实现 1000–1025 会在迭代结束后回收旧文本并无条件给 done。**这不是可直接复用的成功规则**：Gateway 必须区分显式应用终态和网络 EOF，不能以保留的 partial 掩盖断流。
6. 当前实现会选择最长答案 track，并按文本长度产生 append delta。它不能证明改写/缩短/撤回语义；Gateway 已有可替换 snapshot 扩展应继续保留，不把最长文本猜作最终答案。

## 最小可实施范围与验证要求

下一实现应为独立、有界的 schematized 状态 owner，不接管旧 auto/legacy decoder，也不顺带开放 Research、多轮、附件或其他模型：

- 先只处理有明确证据的 markdown 根替换和连续索引追加，严格检查 op、field、path、字符串类型、块身份和索引；越界/稀疏/未初始化/未知操作明确拒绝。不照搬参考的 String 强制转换、忽略 op、稀疏数组写入。
- 中间产物只能是可替换 snapshot；标准正文和成功结算仍需要严格终态。先支持明确 materialized COMPLETED 的有限 profile，text_completed-only 与 workflow 分支另列，不将其静默判成功。
- 先确定请求 profile 选择及终态兼容合同，再接公共 Chat/Responses 原 owner。保留原总 deadline、响应/事件/答案/引用预算、permit、Drop 与失败退款，不另造 producer/retry。
- 红绿验证覆盖根初始化→追加→materialized 完成、PENDING/final 不提前完成、EOF 失败、改写/缩短、同事件重复/跨块歧义、错误 op/path/index/类型、超限与取消。来源列表只能按已证实归属处理，不从其他 track 或旧快照推断。

这些是实现准入规则，不是本轮已完成的测试。新 profile 的终态与来源完整性尚未实现；多轮 read_write_token 的私有所有权、账号隔离与持久恢复也未闭合，本批不为此传入调用方任意会话 ID。

## 当前状态

生产代码、现有保护、模板数量和 devtest 包均未改变。本轮完成“找到并固定足够具体的 patch 行为参考”，没有完成新解析器或 R12/P50/P60/P70。第三方测试没有执行，不能计入 Gateway 通过数量。无账号材料、真实模型调用、费用、现有实例操作或依赖变更。
