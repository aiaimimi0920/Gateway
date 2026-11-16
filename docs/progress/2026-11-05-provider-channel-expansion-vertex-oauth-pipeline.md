# Full Vertex 公共协议与响应准入子批

日期：2026-11-05。对应 [总计划 R11/W07](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。

本批定向无凭据合同通过，不是 R11/W07、P50 或总目标完成。它接续
[本地 UI/TCP/SQLite 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-local-ui.md)，
不重复授权/保存/刷新流程，也不借 Express 测试将 Full Vertex 标为完成。

## 1. 改动范围

- 在现有三适配器准备/最终发送链中，为 typed `vertex_user_oauth` 接入独立响应 owner；旧 generic Vertex/Gemini 不自动转入新语义。
- Express 与 Full Vertex 的相同 REST/SSE 结构检查提取到 `src/upstream/vertex_response.rs` 及其 `sse.rs`。
  共享的只有有界读取、完整 frame、单 candidate、真实 finish 与 usage 准入；label/profile/error code 和投影仍由产品 owner 指定。
- Full Vertex 当前投影是文本子集：不将 thought 混入答案；过滤终态清除此前积累的内容；生成失败明确报错。
  工具、签名和其他 Part 显式拒绝，不经 generic decoder 丢字段后制造空成功。这不是完整 decoder，也不是所有模型已可用。
- HTTP 错误保留已有分类/恢复提示，公共及 audit 不透传任意 provider 诊断；HTTP-200 响应体损坏/读取超时不能证明生成未开始，以 Abort 禁止重放。
- `PipelineContext.observed_usage` 区分未观察与已知 unknown，避免 canonical 重包为 Messages 的默认零 usage 后被重新当成真实报告结算；native 观察失败仍保留旧 fallback。
- canonical → 聚合 Chat SSE 桥接保留 cache-read/cache-creation；总量使用上游报告值，保留显式零，不用 prompt+completion 重算。

本批重新联网只读核实了 [GenerateContentResponse](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse)：
空 finish 不表示已完成，报告 total 包含文本之外的 thought/tool-use tokens。完整 HTTP-200 HTML、抓取来源/hash 与方法页摘录另存证据目录，不据此推断实际账号/模型资格或额度。

## 2. 验证身份与边界

新增 Full Vertex 自己的 pipeline fixture：生产 Local bootstrap、正常管理创建/充值 Key、公共 router oneshot、真实 SQLite 和正常 route/finalize。
配置含 typed OAuth context、固定 project/location 与由 context 派生的 quota project；独立不匹配账号用于检查 exact credential 与模型映射。
只有最终产品校验之后，task-local seam 才将 transport 改为拒绝外网转发的 loopback peer；逐项检查 Bearer、quota、完整 resource、请求体及 caller Cookie/Key 隔离。

本批不执行真实 Google OAuth/生成，不读取历史凭证或真实 `.ng`，不创建 Redis 容器，不修改在线实例，不出 EXE，不提交/推送。
这是 router oneshot，不是本批新跑的真实 Gateway TCP/UI 或原生进程验收；上批的 TCP/UI 证据仍单列。

## 3. 已记录的失败

- `red-incomplete`：0 passed/1 failed，夹具错误地在 provider 直接设置 quota header，违反 credential context 派生合同；只删除夹具 header，未放宽生产校验。
- `red-behavior`：E0308，新增 token 断言的 i32/i64 不匹配；仅修复测试类型。
- `red-behavior-v2`：冻结 2987 项 Rust 输入无漂移，默认栈 **2 passed/4 failed**。
  真实复现不完整流被成功封装、HTTP 诊断泄漏、body 超时未在 fixture deadline 内终止、显式零 usage 不一致。
  已通过的两项是普通三协议消息结算和 caller Drop；不将其重复累计为新增通过数。
- 临时 source-audit 脚本最初用 Windows `C:` ESM import 导致 `ERR_UNSUPPORTED_ESM_URL_SCHEME`，改为 `file:///` 后通过；不改生产代码或测试来掩盖此工具问题。
- 第一轮 `green-vertex`：183 passed/1 failed/2 ignored，2991 项输入无漂移。parser 已保留显式零，但公共 Chat SSE 的共享 usage tap 将 total 重算为 12。
  最小修复位于原 `upstream/stream/usage.rs`：优先保留本次明确报告的 total；明确报告的零计数可覆盖旧正值；native 分段缺省字段继续合并。
  Anthropic `message_start` 的 output=0/total=0 仍是 provisional，不据此提前制造完成账单。新增五项实际分片字节回归；不改测试来接受错误总量。

## 4. 验证状态

| 最终阶段 | 实际结果 |
| --- | --- |
| `green-vertex-v2`，含 Full Vertex/Express 受影响合同 | 184 passed，0 failed，2 ignored |
| 原 stream owner，含新增五项 usage 回归 | 52 passed |
| finalize / send | 12 / 69 passed |
| OpenAI / Responses / Anthropic | 63 / 57 / 61 passed |
| Gemini API / bounded body | 16 / 12 passed |
| 生产非测试 `cargo check`，`runtime-defaults` | exit 0 |
| 代表性关闭 `cargo check`，仅 `line-gemini-web-reverse` | exit 0，不是完整 feature 矩阵 |

按测试名去重 **524 passed / 0 failed / 2 ignored，19 新增**；两项 ignored 是显式人工浏览器 host，不算通过，不把失败阶段的通过数再累加。
全部最终阶段均默认栈、固定 `--locked`、Cargo jobs 4；同一组 **2992 项 Rust 输入**在阶段内与阶段间均无漂移。
每项命令、通过名与来源对比见
[verification-summary.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-pipeline-20261105/verification-summary.json)。

主要可复现命令（仓库根目录）：

```powershell
cargo test --locked --no-default-features --features runtime-defaults --lib vertex_ --jobs 4 -- --test-threads=1
cargo check --locked --no-default-features --features runtime-defaults --lib --jobs 4
cargo check --locked --no-default-features --features line-gemini-web-reverse --lib --jobs 4
```

真实本地合同覆盖：

- Chat / Messages / Responses 的 JSON 与聚合 SSE，经正常消息 Key 结算 2→1；真实 SQLite 记录 requested/resolved model、provider/exact credential 和报告 usage。
- 相同六类入口使用正常 Token Key：保留报告 total=0 / 17、prompt=9 / completion=3 与 cache-read=2，余额分别 100→100 / 83，不重算总量。
- 无 total 的 usage 保持 audit unknown，使用原预留结算策略，不把客户端协议的默认零当真实零账单。
- 三协议 JSON/SSE 的坏流拒绝与调用等待 Drop：不泄漏 partial，不出现成功终帧；失败或取消退款，取消不处罚模型，源 body 关闭。
- HTTP 307/401/403/429/503 的 Chat JSON/SSE 分类/脱敏/退款与不跟随 Location；429/503 仍使用框架已有已知拒绝重试，不宣称全局禁止重试。
- HTTP-200 body 超时，即使已经出现 finish 也须验证完整 EOF；实际只有一次上游请求，失败退款，不重放未知消费。
- MIME/完整帧/finish/多候选、过滤/生成失败、thought 与未支持 Part 的精确边界另有 Full Vertex 自己的 parser/owner 测试；不把这些纯解析测试冒称全部多轮流程。

20 源码均 UTF-8 无 BOM，scoped Rustfmt/whitespace/diff 与全仓 `git diff --check` 通过；少数无关文件的 CRLF warning 保留，未为此改写。
最大 491 有效行，新文件最大 169；原 `client.rs` 489→491、planner 480→483、Express response 230→33、stream usage 239→257，其余清单见 source audit。
checker 31 passed（checker 源码未变，沿用本批已完成运行）；最终 ratchet 4486 文件、14 immutable runtime 单列、governed source >700 为 0。未改 baseline/exception。
源码前后与含新增文件的 scoped diff、错误现场、所有命令 receipt 均留在证据目录；未增加线程栈、另起响应 task 或升级/重装依赖。

## 5. 接力与未完成

Full Vertex 的完整工具/history/签名/Preview/模态投影、多 choice、Server/Worker 真实 Redis、PG/Hosted、跨进程 receipt、原生 EXE 和真实模型/额度验收仍未完成。
本批仅 text owner；完整 functionCall、签名 text 和媒体 Part 当前明确拒绝，不能将其描述为支持全部 Gemini 模型或多轮工具。
下一批应以独立 Full Vertex 的工具身份/history 与 source-scoped 签名合同补全这些产品能力，不直接使用 Express 的 ordered-turn source。
本批不能把对单候选的 Abort 验证外推为全部跨候选恢复与所有协议/feature 组合。
官方线单独开启时，共享 Gemini Web 的既存 15 项缺符号问题仍另列，不能报告全 feature 矩阵绿色。
累计新增模板仍 15；不增加整卡 `offline_ready`，不标 P50、R11/W07 或总目标完成。

证据目录：[gateway-vertex-oauth-pipeline-20261105](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-pipeline-20261105)。
