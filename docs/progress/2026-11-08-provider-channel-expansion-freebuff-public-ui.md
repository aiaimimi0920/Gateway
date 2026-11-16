# FreeBuff：正常网页与活公共请求的逐次购买、结束和再购买联合验证

日期：2026-11-08（Asia/Shanghai）。本批关闭限定流程的联合验证缺口，**不是 FreeBuff 整渠道完成**。
工作区为 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD
`7969f12c85efceacf2a5e384e143b5c39a550981`；继承大量未提交实现，HEAD 不是本批来源凭据。

## 1. 改了什么

承接[公共 Chat 接线批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-public-chat.md)，没有新增生产 Rust 执行 owner、购买接口、存储版本或依赖。

- 扩展原真实 TCP 测试 Host，允许显式全新合成目录及浏览器等待时间；正常结束可保留合成数据库作为证据。
- 合成 peer 增加精确原 attempt 的空 body DELETE，独立核对 Ed25519、原 instance/attempt、Bearer 与 Cookie 隔离。
- 新增真实公共请求→管理批准→FINISH 后旧 claim 拒绝新调用→显式原 end→新购买的 TCP 回归；读取实际 SQLite audit、attempt count、余额和历史，不以 HTTP 200 代替收尾。
- 新增默认忽略、显式 opt-in 的正常网页宿主。文件控制只触发公共 caller 或检查事实，**不能批准/取消购买或结束会话**；所有管理决定由正常 main 网页发出。
- 修正目录确认框“公共调用尚未接入”的过时文案，改为“本操作不购买会话，实际调用仍需逐次批准”；仍须保存账号，不增加解释型 UI。

## 2. 实际联合流程

用缓存的 Playwright CLI 和独立临时 Edge profile，加载真实 `apps/desktop/src/main.tsx` bundle；
只以静态 helper 透明代理到随机回环端口的生产 Local bootstrap/router，没有 mock fetch、管理响应或 API client。
真实管理登录及 Secret Grant、普通管理创建的次数 Key、SQLite、请求拥有的 pending 和原签名 run 都参与执行。
上游账号、设备、目录、价格和模型响应全为合成材料，没有调用供应商或读取历史真实凭据。

| 正常 UI 操作 / 实际公共请求 | 实测结果 |
| --- | --- |
| 公共请求等待批准，网页读取并确认取消 | pending 清空，active/history 为 0/0，无 admission/model/FINISH/DELETE；Local 次数归还至 10 |
| 新请求，查看完整报价后明确批准 | 原公共请求返回完整 native Chat JSON；首次 admission/model/FINISH 各 1，active/history 为 1/0，余额 9 |
| FINISH 后再发公共请求 | 409 `freebuff_public_existing_claim`；没有新购买、模型调用或 pending，失败请求不扣次数 |
| 网页打开原会话结束确认，再取消 | active/history 仍为 1/0，DELETE 为 0 |
| 网页重新明确确认结束原 attempt | 唯一精确 DELETE，final refund 0；active/history 为 0/1；网页正常重读显示原记录 |
| 再发新公共请求，网页重新读取和批准 | 新 pending/request/quote、不同 attempt，第二次 native 响应；余额 8，active/history 为 1/1 |
| 网页明确结束第二个 attempt 并重读 | active/history 为 0/2，正常网页显示两条原始记录，refund 均为 0；pending 为空 |

最终恰好 **2 admissions、2 model requests、2 completed FINISH、2 signed DELETE**，每个模型步骤 reported SDK credits 为 13。
完整配置 document 和 revision 始终不变，等待批准不占 provider permit；终态 audit 的出站 attempt 为成功 1、取消/旧 claim 拒绝 0。
Local Key 等待期间先正常预扣，取消后归还；这与合成 Freebucks 报价余额及 refund 不是同一种计费。

首次报价使用正常默认宽屏，第二次报价和最终历史在 **390×844 CSS 像素**下操作、截图；
这不是全 UI 响应式或原生 EXE 验收。最终网页操作用时 111.515 秒，宿主完整测试用时 136.38 秒，host/peer 正常 join。

## 3. 验证结果与真实失败

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-ui-20261108`。
所有 Cargo 均 `--locked --offline --no-default-features --features runtime-defaults -j 4`，不触发 management-ui publisher。
正常 main 通过真实 Rsbuild config 构建，但不注册会清理 live dist 的 publisher；输出仅在该证据根。

| 最终检查 | 结果 |
| --- | --- |
| 新购买/end/再购买 TCP 定向 | 1 passed；包含在下项，不相加 |
| FreeBuff 相邻 lib 测试，`--test-threads=1` | 441 passed / 3 ignored |
| 显式正常网页宿主 `browser-host-v2` | 1 passed；上项中的一个 ignored 已实际执行 |
| 同一 lib harness / 完整测试名去重 | **442 passed / 0 failed / 2 ignored**；两项新增均已通过 |
| Catalog/Purchase/Claim/GlobalDialogs 四前端文件 | **97 passed** |
| 前端 typecheck / 隔离正常 main build | passed / passed |
| 5 个 Rust 文件官方 rustfmt check | passed |
| effective-lines checker tests / ratchet | 31 passed / 通过，4,936 文件，governed source >700 为 0 |

没有掩盖以下中间失败，也不将其计入最终通过数：

1. 文案红测确实执行：24 passed / 1 expected failed；改文案后四文件 97 项全过。
2. 过宽的 `cargo check --lib --tests` 被未改动的 `tests/console_provider_toggle_health_contract.rs:23` 挡住：
   `E0560: struct RecordCredentialModelFailureInput has no field named native_transport`。
   未改无关测试或结构体来变绿，**不报告全目标编译通过**；实际受影响 lib 已编译并执行。
3. 首次相邻测试错误地使用 `--test-threads=4`，438 passed / 3 failed / 3 ignored；
   既有 cancellation 测试会占满全局 `provider_discovery::background::SLOTS`，与其他 discovery 测试发生争用。
   改用仓库 README 和上一批的串行测试命令后 441/0/3；Cargo jobs 仍为 4，没有放宽生产限制或改测试断言。
4. 首次浏览器手动多轮运行 `browser-host` 失败：第二次结束确认的元素 ref 已失效，CLI 却返回 exit 0；
   本地控制随后检查到 active/history 为 1/1 而非 0/2，正确失败。只实际发送过一次管理 end，旧合成目录原样保留，未重放第二次 end。
   同期有正常 Secret Grant 更新和 scope 清理，不能把旧 DOM ref 当有效授权或把失败截图当两条历史。
   新的 CLI wrapper 明确识别 `### Error`，使用全新目录、正常 locator/快照和实际结果检查串行完成；未改生产来迁就工具。
5. 最终资源审计初版仅按 PID 判断，误把已复用的 12276 当成旧静态 helper。
   对账确认该 PID 是 05:19:01 新启动的 `node ./start.mjs`，而原 helper 此前已有退出及无监听记录。
   原 PID-only 结果保留；按 PID 加命令身份重新核验，未终止或操作这个不属于本任务的进程。

## 4. 来源、大小和收尾

9 个最终编译/测试/格式/ratchet 阶段各 **4,401 项**选定源码/配置输入 before/after 与最终源码逐项相同。
范围覆盖 Rust src/tests/build_support/.cargo/crates、根 Cargo/build/toolchain 与前端 src/public/tools/package/config；
不含 node_modules、外部路径依赖或完整宿主环境，**不是 hermetic 构建声明**。

5 个 Rust 文件中，原接线 192→194、Host 221→232、peer 173→190、新购买循环 190、新网页宿主 188 有效行。
前端生产文案 71→71，测试 109→111；无新增或增长到软上限的文件。
`final-audit.json`、`effective-lines.json`、`terminal-tests.json` 和 `final-scoped.diff` 保存 direct preimage/new-file、编码、范围、去重与来源证据。
原 guard、baseline、exceptions 的 SHA-256 不变；真正的 `artifacts/effective-code-lines.json` 已先保存本轮报告，再从本轮 direct preimage 精确恢复，见 `report-restore.json`。
UTF-8 无 BOM、范围 whitespace 与独立仓库 `git diff --check` 通过。

失败 v1 退出而非正常 join；只读核对仍为 1 active / 1 history，未删除或“修复”这个合成现场。
成功 v2 的 host/peer、Edge 和静态 helper 正常结束；最终核验自建进程及四个前后端监听均已退出，见 `resource-closeout-final.json`。
没有 Redis/Docker、现有实例、真实凭据或供应商操作，没有出包、提交或推送；不复用已消费的 P34 授权。

## 5. 下一棒

本批只退役**目录 CLI / Local 非流式 Chat**在正常 UI→真实 TCP→活公共请求的批准/取消、原 claim 显式 end→下一次购买联合缺口。
不新增 public end API，不改变 FINISH 不 end 的合同，不承诺公共付费阶段 cooperative FINISH。
仍需公共各阶段取消/费用 closeout、START/FINISH 未知与 durable run/lease、其他公共协议/实时 SSE/模态/参数、
完整 CLI harness 的实际工具/child 与费用、精确 probe、新设备/目录应用的正常 UI 联合、Server/PG/Hosted、跨进程/原生包和真实平台资格/模型/额度验收。

其余 R 项和全局 **17 模板、无新增整卡 `offline_ready`** 的口径不变；P50 全范围无凭据集成/devtest、P60 集中录制、P70 真实逐渠道验收及总目标继续。
