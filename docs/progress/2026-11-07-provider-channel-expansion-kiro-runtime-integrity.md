# Kiro CLI Key / OAuth runtime：事件歧义拒绝与空工具参数一致性

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
继承 `main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981` 的未提交工作；HEAD 不是本子批实现的交付凭据。
承接[正常 UI / 公共 TCP / Local 重建子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-kiro-oauth-local.md)，只修共享严格 decoder 的两个具体问题，不新增模板或完整产品合同。

## 改了哪些内容

1. 严格 CLI Key 与 OAuth runtime 的 EventStream header 在进入 map 前拒绝重复名、空名与非法 UTF-8；要求明确的字符串 `:message-type`，仅接受 `event/error/exception`，普通事件必须有非空字符串 `:event-type`。
2. 三类已知 payload 直接 typed serde 解码，拒绝重复关键字段（包括转义后的同名字段）、错误类型和显式 null；工具续帧缺字段仍可省略。先要求 JSON object，避免 derived struct 意外接受 positional array，但不先转 `Value` 丢失重复键。合法未知事件继续忽略，不要求其 payload 是 JSON，不发明上游文本终止帧。
3. 工具参数只有累计到 stop 仍全为空白时才对外规范为 `{}`；不会在 start 提前补对象，非空参数及 JSON 字符串内部空白保留。按原始输入保留 4096 工具 / 64 MiB retained 预算，每片只扫描新 delta，不反复扫描增长的 prefix；stop 后释放累计参数。
4. `EventStreamParser::default()` 的旧 REST 兼容行为保持，不把严格 runtime 的规则强加给旧链。既有 OAuth runtime 与 CLI Key 继续复用同一个 parser/accumulator/translator，没有新服务或平行 decoder。

源码 owner：

- [有界帧与 header](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/event_stream.rs)。
- [事件 envelope 与 typed payload](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/event_payload.rs)。
- [工具完整性与空参数规范化](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/key_response.rs)。
- [严格/default 对照回归](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/runtime_integrity_tests.rs)。
- [真实公共 TCP / Local 结算回归](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/kiro_oauth_local_tests/integrity.rs)。

## 复现与实际测试合同

生产未修时 `red-v1` 实际完成 **0 passed / 6 failed**，不是编译错误：重复 header、重复 JSON 字段、非法 UTF-8 和空工具参数有行为失败证据。带循环的测试在首个失败即停止，不把后面的每个变体都算作独立已复现红测。
公共 TCP 重复 `content` 场景曾返回错误成功 `"content":"visibley"` 与 `finish_reason: stop`。
缺失/错误 envelope 的后续变体，以及新增的数组/显式 null/非字符串 envelope 保护均以最终回归为准，没有声称它们也在原六项红测中逐一复现。

公共错误测试从正常管理设备流保存 OAuth 材料后，覆盖 Chat / Messages / Responses × JSON/SSE × 三种歧义帧，共 18 个场景；每次断言 failed audit、精确账号/模型、Gateway Local 消息预占退款、上游严格只收到一次请求，且没有成功终态。
这不是供应商实际 credits 的退款或远端取消证明。

工具测试包含两个交错工具、空/Unicode 空白参数和六种公共返回；三协议最终参数均可解析为 object `{}`，Local 余额 10→4，上游严格六次。
Chat 检查 ID/name 与 delta 拼接；Messages 按 start.input、可选参数 delta 和 block stop 重建；Responses 同时核对 delta、arguments.done 和 completed.output 的参数一致性及 call_id/name，不把一个中间事件当完整证明。
单元对照另覆盖真实 JSON 参数与字符串内部空格、重复字段、header、合法未知事件及数组拒绝，旧 default 路径保持原结果。

## 验证与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-runtime-integrity-20261107`。

| 检查 | 实际结果 | 回执 |
| --- | --- | --- |
| Kiro 库定向回归 | **214 passed / 0 failed / 2 ignored**，含本批八项新增；未运行两个浏览器 host | `kiro-final-v4.json` |
| 正常生产库 | passed；`cargo check --locked --offline --no-default-features --features runtime-defaults -j 4 --lib` | `production-final-v1.json` |
| 代表性仅关闭 Kiro 的生产库 | passed；从当前 `runtime-defaults` 展开列表去掉 `line-kiro-official-vendor-api`，不是完整 feature 矩阵 | `disabled-final-v1.json` |
| 七项相关源码官方 Rustfmt | passed | `rustfmt-final-v3.json` |
| checker 自测 | 31/31 passed；消费的 scripts 输入与最终源码另核未变 | `checker-final-v1.json` |
| 官方有效行 ratchet | passed；4791 文件、14 immutable runtime artifacts、governed source >700=0 | `ratchet-final-v2.json` |

按完整测试名称去重仍为214 passed，不叠加中间绿测或上一批浏览器结果。两个 opt-in host 保持 ignored，本批不计通过。
正常/代表性关闭生产编译仍有既有 warnings，不报零警告、全仓绿或完整 feature 矩阵。

五个最终后端/静态阶段各5104项所选输入在阶段内无漂移，且与当前源码 hash 一致；checker 的 scripts 消费子集另核未变。
扫描不是对所有外部工具、动态输入或全部依赖的完整冻结。
七项源码由官方 lexer 复算：`kiro.rs` 360→362、`event_stream.rs` 417→439、`event_payload.rs` 107→152、`key_response.rs` 124→134、`kiro_oauth_local_tests.rs` 56→57；新增单元/公共回归分别215/267有效行，均低于500，没有修改baseline/exception。
UTF-8无BOM、本地链接、直接preimage、限定源码变化、范围diff及最终来源见 `audit.json`、`effective-lines.json` 与 `scoped-diff.patch`。
本批ratchet报告先归档，再从**本批直接preimage**原字节恢复共享报告，保留继承dirty内容；`git diff --check`通过。

`kiro-final-v1/v2` 是主线程因静态复核发现累计 trim 的潜在 O(n²)、以及 derived serde struct 的数组接纳风险而精确停止的编译，exit 101，不是完成的实现红测或绿测。
这两个风险随后分别通过 per-delta `nonblank_arguments` 和 object guard 修正；独立只读复核未发现新阻断，但不替代实际测试。
`rustfmt-final-v2` 首次检查发现测试模块声明顺序不符；官方formatter只调整 `kiro.rs` 的模块位置。
当时在跑的 `kiro-final-v3` 虽214 passed/2 ignored，但记录该一项输入漂移，不作为最终来源一致证明；随后在固定源码上完整重跑 `kiro-final-v4` 才计最终通过。所有中间日志与失败回执保留。

## 未验证与接力边界

本批只闭合严格 runtime 事件歧义和空工具参数的一致性问题，不等于完整 decoder、所有业务参数/取消阶段、当前官方最小 scope/profile 发现/订阅/客户端资格或旧 route/未变 source hash DB 自动升级。
没有重跑上一批浏览器，没有通用 background scheduler、Server/PG/真实 Redis、独立原生 Gateway 进程、OS 重启、跨应用 receipt 或整包验证。
未读取真实 `.ng`/历史凭据、未调用真实平台、未创建容器、未修改现有实例、未出包、提交或推送；P34 历史单容器授权不复用。
累计模板仍 17，无新增整卡 `offline_ready`；R02/W02、其他渠道与 P50/P60/P70、总目标继续执行中。
