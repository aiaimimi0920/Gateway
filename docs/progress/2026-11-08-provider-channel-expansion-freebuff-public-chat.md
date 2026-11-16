# FreeBuff：目录 CLI 的 Local 公共非流式 Chat 子链

日期：2026-11-08（Asia/Shanghai）。本批完成限定的公共请求接线与离线验证，**不是 FreeBuff 整渠道完成**。
工作区为 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，HEAD 仍为
`7969f12c85efceacf2a5e384e143b5c39a550981`，实现包含继承和本批未提交修改；HEAD 不是交付凭据。

## 1. 这次真正接通了什么

复用[正常账号逐次确认 UI 的管理合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-purchase-ui.md)，
新增 `stage_send/freebuff_catalog`，将真实公共 `POST /v1/chat/completions` 的原请求接到原请求拥有的 pending/批准、Local 持久 claim 和原 signed catalog run。
限定为已保存目录业务目标的 **CLI、LocalRuntime、非流式 Chat 文本/函数工具转发**。
Desktop 不获得 CLI 购买能力；Responses、Messages、流式、其他模态和未支持字段不借 legacy 转换悄悄放行。

调用者原生 system/developer、文本 parts、函数 schema、tool_calls/tool results、reasoning 历史和支持参数进入新的 `RelayStep`。
它不是默认 CLI `RootTurn`：不注入 CLI system/提醒、不虚构 15 个工具、不执行调用者工具、不修复或摘要调用者历史，也不提供本机权限或 agent loop。
`reasoning_details`、call `extra_content` 与原始 arguments 字符串保留；`&amp;` 不被“修复”。

该步骤复用实际 native context estimator 和 runtime 预算预检；没有 tools 就不估一份虚构工具 schema，system 不再被同时计入 history 差分。
这是近似预测，不是 BPE、精确 tokenizer 或已验证的线上模型上限。超预算在报价前明确拒绝，不自动摘要 caller 会话。

## 2. 准入、批准与唯一付费发送

1. 经过原 candidate/model 准入与兼容检查，先验证原生请求形状。只恢复精确已登记 device；公共请求不生成、注册或修复设备。
2. signed catalog refresh 后从同一 snapshot 选 stable model key、opaque handle 和 fetchId，冻结真实 caller intent。
   intent 是批准用的 pre-runtime envelope，不是尚未产生 run/session metadata 的最终 wire bytes。
3. 从当前 route snapshot 和服务端 `PipelineContext` 捕获 `RequestBinding`。已有 durable claim 直接拒绝，不 heartbeat、认领、复用、替换或再次购买。
4. signed quote 进入原有有界 Registry，等待正常管理身份、Secret Grant、revision 与账号 scope 下的显式批准/取消。
   等待时不占 provider 并发 permit，不购买；客户端 body 里的 `user`/确认字段不能授予权限。
5. 批准后才取并发 permit、执行一次原 provider rate admission。消费前重捕获 binding；fresh inspect/价格检查与原 OS slot guard/SQLite transaction 继续工作。
6. `ExistingClaim::Reject` 在购买 owner 内再次拒绝竞态中的旧 claim，不能转成 legacy Restore/heartbeat。
   唯一 `PendingAdmission.send` 前异步重验 mutable eligibility，再同步重验 quote/Grant/revision，最后才 `begin_attempt()`。
   stale quote、旧 claim race 和纯本地拒绝不算真实出站尝试；资格重验不重复扣 provider rate-limit。
7. admission 后使用原 START/run/claim，START await 返回后再重验资格和管理 fence，才发送模型。
   已 START 但尚无模型尝试时被拒绝，可沿原 run 显式 cancelled FINISH（已知零模型步），不释放 claim。
8. 原 run 只添加 host metadata，再由原 serializer 冻结实际 bytes 并签名；strict decoder、reported SDK credits、显式 FINISH 沿用原 owner。

新 catalog 分支直接返回，不进入 `execute_observed`、legacy retry 或 provider fallback 闭包；不会自动重买或改用另一条业务链。
原 `business_target.rs` guard 保持原 bytes，但**不再等于所有公共目录请求都关闭**：上述限定 typed 路径在它之前接管；其他旧执行入口仍受原 guard 保护。
报价不是服务端金额 ceiling/CAS，不承诺锁价。

## 3. 响应、费用与取消边界

成功返回完整 native Chat JSON，保留 reasoning/tool 扩展和部分 usage，不经 canonical repack 降维；原 pipeline 仍负责 source quota、audit、并发和终态结算。
测试 peer 的完整响应含 13 SDK credits；这不是 Freebucks 小时价格，也不是 Gateway 次数 Key 扣费。
普通 Local 次数 Key 成功为 10→9；失败/等待时取消为 10→10，不据此声称供应商退款。

严格解码失败且费用已知时走原 failed FINISH；费用缺失时不伪造 0、不发未知金额 FINISH，也不自动重试。
**FINISH 不 end session，不释放 claim。** 后续公共请求拒绝遗留 active/unknown claim，仍需原正常管理 UI 显式检查/结束原 attempt。
commit 后资格变化可能保留尚未发送的本地 claim；没有证据不能把它删除或换身份重买。

等待批准时 cancel/调用者断开已验无购买、撤销 pending、退款并释放本地资源。
付费执行阶段的 caller/shutdown/timeout 仍由外层 future Drop 取消，保留 journal；传入内部模型步骤的 cancellation future 仍是 pending。
**本批没有接公共 cooperative FINISH 保证**，不 detach 后台补偿、不隐式 end、不声称 START/FINISH 未知义务已解决。

## 4. 最终验证及来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-chat-20261108`。
所有 Cargo 验证均 `--locked --offline --no-default-features`，正常使用 `--features runtime-defaults`，不触发 management-ui publisher。
测试的 TEMP/TMP/TMPDIR 指向该证据根的 `tmp`；peer 与真实管理/公共 router 都绑定随机回环端口，使用合成账号/device/响应，不调用供应商。

| 最终阶段 | 实测结果 | 范围 |
| --- | --- | --- |
| `focused-v3` | 16 passed | 9 个真实公共/管理 TCP 测试函数、7 个 native relay 测试函数；部分函数有多个场景 |
| `freebuff-adjacent` | 440 passed / 2 ignored | 包含上述 16 项，不相加；两个旧浏览器 host 未显式运行 |
| `gate-adjacent` | 15 passed | 包含新增 eligibility 重验不重复消费 rate-limit/attempt 的测试 |
| `lifecycle-adjacent` | 6 passed | 原 pipeline 生命周期 |
| `send-adjacent` | 46 passed | 原 stage_send tests |
| `local-adjacent` | 8 passed | `local_runtime::tests::`，不是整个 LocalRuntime 49 项 |
| `production-final` | passed | 正常生产 lib 编译 |
| `production-freebuff-disabled` | passed | 仅移除 FreeBuff line 的代表性生产 lib 编译；未跑关线单测/完整矩阵 |
| `final-format` | passed | 25 个本批 Rust 文件的官方 rustfmt check |
| `checker-tests` / `final-ratchet` | 31 passed / passed | ratchet 扫描 4,934 文件，governed source >700 为 0；不冒充 strict 全审计 |

按同一 lib harness 与完整测试名去重，**515 passed / 0 failed / 2 ignored**；其中本批新增 17 项均通过。
`terminal-test-names.json` 和 `terminal-summary.json` 保存去重与阶段结果，不累加重复运行或中间失败数。
10 个最终编译/测试/格式/ratchet 阶段各 3,464 项选定输入 before/after 与最终源码逐项一致，覆盖 src/tests/build_support/.cargo、根 Cargo/build/toolchain 和本地 crates 输入，排除生成缓存；不是完整 hermetic 环境声明。
checker 输入未变化，复用本批已通过的 31 项，不因只改测试夹具重跑相同 checker tests。

新增 13 个 Rust 文件、修改 12 个既有 Rust 文件；最大为 `stage_send.rs` 的 **494→499** 有效行，新文件最大 221 行。
Bedrock continuation 预检只原样提到 `stage_send/admission.rs`，不改变其语义；不接管 S06。
`effective-lines.json`、`scoped-rust.diff`、`source-audit.json` 保留 direct preimage/new-file 审计；Git diff 不能独自覆盖继承的 untracked 源码。
UTF-8 无 BOM、scoped whitespace/来源检查与 `git diff --check` 已执行；保留既有警告，不运行 cargo fix。
原 guard、baseline、exceptions 的 SHA-256 不变，无依赖或存储版本变更。

门禁实际刷新的是被忽略的 `artifacts/effective-code-lines.json`，已复制到证据根，未取得刷新前 direct preimage，因此不声称它已原样恢复；保留生成报告。
先前误按 `artifacts/effective-code-lines-report.json` 保存的 preimage 未用于证明真正门禁报告的原状，详见 `report-provenance-note.txt`。

### 真实失败记录

- `red-relay`：未实现 build 的行为红测，1 passed / 3 expected failed；不是编译失败冒充红测。
- `focused-v1`：4 passed / 7 failed；7 个 TCP 场景均到达断言后的 cleanup，但 fixture 目录前缀不符合现有安全 helper。只改 fixture 前缀，未放宽清理保护。
- `focused-v2`：E0609，新增测试误把 route_attempt_count 当 View 字段；改为读取真实持久 route_trace，不修改生产数据模型来迁就测试。
- 最终 `focused-v3` 全过；此前编译、行为或夹具失败均保留，不算进最终通过数。

## 5. 下一棒与全局未完成项

优先补正常账号 UI→真实管理 TCP→本次活公共请求批准/取消→原 claim 显式 end→下一次购买的联合验证；本批由管理 HTTP 驱动，**没有浏览器联合测试**。
继续公共执行各阶段协作取消/费用 closeout、START/FINISH 未知与 durable run/lease；不能用内部模型步骤取消测试替代整公共链。
Responses/Messages、实时 SSE、其他参数/模态、精确 probe、Server/PG/Hosted、跨进程/原生包及真实平台资格/模型/额度仍未验。
完整默认 CLI harness 的实际工具、child 和费用/持久 history 等仍是独立未完范围，不把 caller 函数转发当 Gateway 工具执行。

模板仍 **17**，无新增整卡 `offline_ready`；P50 全范围无凭据集成/手测包、P60 集中录制、P70 真实逐渠道验收与总目标继续。
没有读取历史真实凭据、调用供应商、操作 Redis/Docker、修改现有实例、出包、提交或推送；P34 授权不再复用。
本批复用已在仓的固定公开参考/原执行 owner，没有重新联网，不将合成回环验证当当前线上 wire 已验。
