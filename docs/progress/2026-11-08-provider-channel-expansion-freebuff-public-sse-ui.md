# FreeBuff：正常网页与三公共协议活 SSE 的购买、费用事实、结束和再购买联合

日期：2026-11-08。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承 dirty/untracked 保留，HEAD 不是本批实现来源凭据。

## 完成范围与改了哪些内容

承接[三公共协议 SSE 接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-protocol-sse.md)，本批新增测试和联合证据，不改变生产执行、费用、权限、存储版本或依赖。

- 新增显式 opt-in 的正常 main 浏览器宿主，实际依次调用 Chat、Responses、Messages 的单轮文本 SSE。
- caller 使用真实 TCP 和正常 Key；观察已交付 bytes，最多保留 128 KiB 合成输出。原上游 EOF 与原 FINISH ack 有独立 gate，验证不是先完成再展示。
- 复用原 UI 控制的有界文件读取、严格顺序和不重放约束；只推进 caller/EOF/ack 或断言事实，不授予控制脚本购买批准、取消或管理 end 权限。原非流式 UI fixture 的动作顺序保持不变，pending 等待仅泛化 JoinHandle 类型。
- 浏览器加载真实 `apps/desktop/src/main.tsx` 构建，静态 helper 只透明代理随机回环端口的生产 Local bootstrap/router；没有 mock fetch、管理响应或 API client。

本批退役的是**目录 CLI / Local 的正常 UI 与三协议单轮文本活 SSE 联合缺口**，不是全部参数、工具 UI、完整 FreeBuff、Server、原生包或真实平台验收。17 模板、无新增整卡 `offline_ready`，P50/P60/P70 和总目标仍未完成。

## 实际执行的正常流程

| 正常页面操作及原公共 caller | 实际结果 |
| --- | --- |
| 真实管理登录，进入 pool 的已保存账号编辑页 | 正常 Secret Grant；不修改账号、业务站点、路由或 revision |
| 请求等待批准，读取报价并明确取消购买 | public 409 `freebuff_purchase_cancelled`，0 admission/model/FINISH/DELETE；余额恢复 10，active/history 为 0/0 |
| Chat 新请求，页面查看完整报价后明确批准 | 原 signed admission/START/model；EOF 阻挡时 caller 已读到 partial/native thought；permit 仍为 1 |
| 页面检查正在执行的原 claim | 显示“已观察模型费用”、1 step / 13 reported SDK credits；公共成功终态尚未交付 |
| 释放真实 EOF，阻挡原 FINISH ack，再正常读取 claim | 页面显示“FINISH 待确认”；caller 仍无公共成功终态，permit 仍为 1 |
| 释放同一次 FINISH ack，读完 SSE，再正常检查 claim | 恰好一个标准终态，Local 成功结算；页面显示“FINISH 已接受（未结算）”，不是 session 自动 end |
| FINISH 后再次发公共请求 | 409 `freebuff_public_existing_claim`，出站 attempt 为 0，不买、不发模型，也不扣次数 |
| 页面打开原 end 确认再取消，然后重新明确确认 | 取消不发 DELETE；明确确认只结束原 attempt，一次 signed DELETE，final refund 0，历史可正常重读 |
| Responses、Messages 分别重新读取新报价并批准，重复活流/EOF/ack/原 end | 使用不同 attempt；三协议都完整经过上述真实公共链，最终历史 3 条 |

最终恰好 **3 admissions、3 model requests、3 completed FINISH、3 signed DELETE**；3 个 attempt ID 不同，active/history 为 **0/3**、pending 为空。每步 reported SDK credits 为 13；合成 Freebucks 的报价/退款与 Local 次数不是同一种计费。普通 Key 次数为 **10→9→8→7**，额外取消和旧 claim 拒绝不扣次数。7 条最终 audit 的成功 attempt 为 1，取消/拒绝为 0；完整配置 document 与 revision 不变。

浏览器在默认宽屏、**390×844**（Responses）和 **1280×900**（Messages）执行；16 张截图包含报价、live、FINISH pending/accepted 与逐次历史。正常 UI 动作流程用时 131.187 秒；最终宿主完整测试 213.87 秒，host/peer 正常 join。不是全 UI 响应式、有效函数工具执行或原生 EXE 验收。

## 最终验证与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-sse-ui-20261108`。成功正常浏览器证据位于 `browser-v2`，合成 SQLite 原样保留。

| 检查 | 结果与证据 |
| --- | --- |
| 新三协议正常网页宿主 | **1 passed / 0 failed**；`browser-host-v2`，该项默认 ignored，但本批明确实际执行 |
| FreeBuff 相邻 lib | **508 passed / 0 failed / 5 ignored**；`freebuff-final`，串行测试，Cargo jobs 为 4 |
| 同一 lib / 完整测试名去重 | **509 passed / 0 failed / 4 ignored**；新增 fixture 的 ignored 身份由本批 opt-in 成功替换，不与中间或历史重复累计 |
| 隔离正常 main 构建 | 实际通过；`frontend-main-build`，未注册会清理现有 dist 的 publisher |
| 官方 Rustfmt | 6 个范围 Rust 文件通过；`rustfmt-final` |
| Effective-line checker / ratchet | 31 passed / 通过，4,993 文件；14 immutable，governed source >700 为 0 |
| 两生产 lib | **按输入核对复用**上批正常与仅关闭 FreeBuff 的成功编译，不宣称本批重跑 |
| 前端定向 209 项与 typecheck | **按输入核对复用** run-journal 批成功证据；937 个选定前端消费输入逐项同 hash，不宣称本批重跑 |

7 个本批成功构建/宿主/回归/格式/checker/ratchet 阶段各 **4,458 项**选定输入 before/after 与最终源码一致。范围包括 Rust src/tests/build_support/.cargo/crates、根 Cargo/build/toolchain 与前端 src/public/tools/package/config；不含 node_modules、外部路径依赖或完整宿主环境，不是 hermetic 构建声明。

生产编译的复用对账恰好只有 6 个 `#[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]` 测试子树输入不同（3 旧/3 新）；其余被记录的生产输入不变。对应原 receipt 与差异在 `reused-checks.json`。没有重跑不受影响的共享预算/tracker/lifecycle/Bedrock TCP、全仓测试或正式发布审计；上批证据不冒称本批全仓绿色。编译仍有既有 unused/dead-code 警告。

6 个 Rust 文件最大 200 有效行，新文件最大 144；原 `ui.rs` 188→188、`purchase_cycle.rs` 190→187，接线 199→200。policy/baseline/exceptions 和生成报告按本批 direct preimage 原字节保护，不用 Git HEAD 代替。`final-audit.json`、`scope-counts.json`、`scoped-vs-preimage.diff` 记录测试身份、来源、UTF-8 无 BOM、范围和 diff 检查。

## 真实失败与安全收尾

首次 `browser-host` 为 **0 passed / 1 failed**，不是成功证据。临时 CLI 脚本错误地按 `dialog` 定位，实际确认框是无 accessible name 的 `alertdialog`；未匹配元素，CLI 返回 `### Error`，wrapper 正确失败。原报价随后到期，caller 实际收到 `freebuff_purchase_quote_expired`，fixture 没把它当取消成功。

该失败现场只读对账为 active/history 0/0；没有购买、模型调用或 end。随后正常页面的一次取消决定得到过期拒绝，未重放旧决定或改写合成库。旧日志/DB/未确认控制命令均保留。只修临时脚本为当前真实 `alertdialog` locator，使用全新 root、命名独立 Edge 和 helper 完整执行成功；生产期限、权限与测试断言均未放宽。

成功和失败两次浏览器及各自 helper 均已关闭；最终核验本范围构建/runner/浏览器/helper 无匹配在途 actor，四个已记录前后端端口均无监听，见 `resource-final.json`。未按 PID 猜测清理其他项目，没有删除合成现场或停止现有实例。

接力收尾再次核对 7 阶段的 before/after 与当前 4,458 项选定输入、复用检查的实际消费输入、浏览器观察与 completed 事实，并补齐 `final-audit.json`、完整测试身份及范围 diff；总计划和状态总览已同步本批限定完成范围。首次临时审计因将 Git 既有 CRLF→LF 提示当错误而失败；保留并严格分类这些提示后，实际 `git diff --check` exit 0、范围新文件检查均通过，没有为此改写源码换行或 Git 配置。额外只读资源复核见 `resource-rechecked.json`。

本批没有真实凭据/供应商、Redis/Docker、现有实例、出包、提交或推送操作；不复用历史已消费的 P34 授权。

## 下一棒

继续完整参数/模态、实际 host 工具和 child 与费用、持久完整 history、未知 START/FINISH 的明确对账和安全恢复、远端 lease、精确 probe、新设备/目录应用正常 UI 联合、Server/PG/Hosted、跨进程/原生包和真实平台。当前只验证单轮文本的正常浏览器联合，不拿它代替上述范围，也不把原 FINISH 已接受当 session 已结算或恢复执行许可。
