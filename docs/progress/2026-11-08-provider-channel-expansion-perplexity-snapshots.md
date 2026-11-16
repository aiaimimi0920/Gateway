# Perplexity Web：实时可替换快照与晚置标准正文

日期：2026-11-08；范围 R12 / W08 的单轮文本增量，不是整卡 `offline_ready`、P50/P60/P70 或总目标完成。

## 本批改动

沿原三适配器与正常公共 stream/tracker owner，Chat/Responses 可在原生终态前交付 `gateway.perplexity.snapshot` 产品事件。完整快照可以缩短、改写或清空；标准正文仍只在原严格 decoder 确认合法应用终态后交付最终答案，不能把临时文本伪装成标准追加。最终来源不继承旧快照，usage 保持未知，访问 metadata 不进入返回值。

`Exchange` 复用原固定目的地、Cookie/UA、零重定向、单次 POST、严格 decoder 与120秒总 deadline；buffered 与 live 共享，不另造解析/刷新/重试 owner。live 无后台 producer，每次只取一个完整帧；32MiB 公共序列化累计预算及最终标准帧整体准入、最多3帧待发队列保护交付。合法 `end_of_stream` 是原有应用终态，不是 TCP EOF；收到后释放 body。Drop/取消释放同一原连接，标准成功不早于严格最后快照检查。

Responses 转换器保留同名产品事件与 response ID，验证载体字段/顺序及答案/来源预算；标准正文、工具、usage 或终态后不允许新快照，快照-only EOF/DONE 拒绝。公共精确字段与临时替换语义见[实现合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-perplexity-web.md)。

新增真实 live 验证暴露了原普通 streaming 在 headers 后 `drop(permit)` 的问题：旧 Perplexity 聚合时这时已完成，新快照流此时仍在生成。只为该 adapter 将 permit 转给原 `stream_terminal::Owner` 回调，在成功、错误或 Drop 同步释放；其他普通 adapter 的既有行为不变。FreeBuff cooperative future 已自行持有 permit，其接线明确为 None，没有转移或重复释放。

## 验证与失败记录

证据目录：[本批调查与运行产物](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-perplexity-snapshot-owner-20261108)。每次 `run.py` 命令记录前后选定输入hash、退出码及原日志；最终通过范围在验证结束后补齐，不以源码存在或编译中视作完成。

- `red-live`：旧实现终态前不能返回；定向1项预期失败，错误为 `Headers and native snapshots must not wait for the terminal event: Elapsed(())`。
- `green-live`：首轮34 passed / 2 failed，源码4464项选定输入前后一致。两个失败分别为旧收集器在 headers 后 body 错误上 unwrap，以及 fixture 未固定主备优先级却检查指定池并发；不是全绿。保持有可用 backup 的 fixture 改为显式优先级；原 buffered HTTP 错误断言保留，新增 live headers 后错误/失败审计/禁止标准终态与重试测试，不为测试放宽生产协议。
- `green-complete`：39 passed / 2 failed，4466项输入前后一致。原失败两项已过，新增 early-drop 在快照可见后抓到上述真实 permit 过早释放；另一个失败为测试读到 body Err 后仍持有 Body，导致 fixture 的 state 收尾等待超时。新增测试明确 Drop 错误响应，生产 permit 沿唯一 tracker 收尾，不删除失败断言或将其当随机故障。
- 首次 checker wrapper 使用无后缀 `npm` 发生 `[WinError 2]`，没有执行检查；改为已确认的 `npm.cmd`。失败现场保留，不计通过。

`green-terminal` 最终 Perplexity 定向 **41 passed / 0 failed / 0 ignored**，包含9项本批新增，不与中间34/39项相加。两个公共端点实测改写/缩短/清空后只投影最终标准答案和最终同事件来源；headers 后四类失败只中断 body，不出成功正文/terminal，不切仍可用的 backup。终态前真实快照可见，持有原 permit；Drop 后上游 body 释放、cancelled 审计、permit归还和健康中性均已断言。正常公共 router 与 Local SQLite 接真实合成上游 TCP；不把这个内存 downstream router 测试说成真实下游 TCP、浏览器 UI 或原生 EXE 验收。

### 最终检查结果

| 检查 | 实际结果与边界 |
| --- | --- |
| Perplexity 定向 | 41 passed / 0 failed / 0 ignored；9项新增均已通过 |
| 共享 Responses/来源、stream tracker/usage/archive、pipeline lifecycle 与 FreeBuff 相邻 | 625 passed / 0 failed / 5 ignored；其中508项通过名含 FreeBuff。4个旧浏览器宿主和1个由父测试拥有的独立 child 入口没有显式单独执行；父测试已按正常筛选执行，不把 ignored 当通过 |
| 同一 lib harness 按完整测试名去重 | **664 passed / 0 failed / 5 ignored**；上述两组重叠2项，不能直接相加。首轮36个测试身份全部保留 |
| 正常生产 lib | `cargo check --lib --locked --offline --no-default-features --features runtime-defaults -j 4` 通过，不是 EXE/管理网页构建 |
| 代表性关闭 Perplexity 的生产 lib | 仅从 `runtime-defaults` 展开的 features 移除该 leaf 后编译通过；没有宣称关闭配置单测或完整 feature 矩阵通过 |
| Rust 官方格式 | 22个范围 Rust 文件 `rustfmt --edition 2021 --config skip_children=true --check` 通过 |
| 代码行数 checker | checker 测试31/31；ratchet扫描5001文件通过，governed source >700为0；14个既有immutable runtime artifacts另计，不是本批消除了这些资产行数 |
| 差异检查 | 独立仓库 `git diff --check`（另经 `rtk bypass` 复核）通过；原工作区其他文件的CRLF提示保留，未为提示改动它们。新增未跟踪文件另经范围UTF-8/尾部空白与直接写前diff检查 |
| 源码与范围 | 最终各验证阶段选定4466项输入前后及相对最终源码一致；22 Rust/4 Markdown 均UTF-8无BOM，最大496有效行；没有调整policy/baseline/exceptions，checker根报告按直接写前镜像恢复 |

本批修改的最大文件 `to_responses.rs` 为478→496有效行，专用快照状态/转换及测试独立成模块；原 streaming 为237→243、terminal为276→279、FreeBuff接线179→180。没有触碰新增/修改源文件的软上限例外。`audit.json`、`counts.json`、去重身份及直接写前diff保存在上述证据目录；只将这些选定输入作为来源证据，不将hash核对当真实业务成功。编译保留仓库已有 warnings，不报 warning-free。没有跑前端/浏览器、全仓所有测试、完整matrix、devtest/正式包或真实平台。

本批两个 lib test harness 和串行编译队列均已退出；合成上游监听随测试进程结束，未保留后台 producer、helper 或浏览器，不停止其他来源的任务/实例。一次性审计脚本更新曾发生 PowerShell quoting `ParserError`，未写入文件；改用 apply_patch 后范围审计通过，不将该工具错误计入功能测试。

## 来源与边界

2026-11-08 07:28 UTC 重新联网取得两份 pinned commit 的行为参考与既有 MIT LICENSE，四次HTTP200及最终URL/hash记录于 `reference-receipts.json`；未运行这些项目。固定来源见[原公开合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-perplexity-web-contract.md)。edusu 的 diff value append 不足以证明 op/path 合同，本批不猜测 JSON Patch；已有 MIT 声明保留，P50仍需验证随包 notice。

仍未完成：multi-turn、diff/patch、Research、附件、其他模型策略、目录、当前 session/challenge/refresh、精确账号/订阅/额度证据、Server/PG/Hosted、独立下游 TCP/进程/原生包与集中录制。正常兼容客户端标准正文仍聚合，只有支持该扩展的消费者能提前显示可撤回快照；不宣称标准实时 token 流或整个 Perplexity 渠道完成。

模板总数仍17，无新增整卡 `offline_ready`。未读或试用真实凭据、未调用真实平台、未操作现有实例或 Redis/Docker、未出包/提交/推送；已消费的 P34 授权不复用。
