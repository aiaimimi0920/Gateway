# R03：Grok 普通 Gateway Key 按次数预付费合同

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。本批仅新增测试和模块声明，没有修改生产计费、路由或协议行为。

## 实际闭环

生产 `build_app_state` 创建隔离 Local/SQLite 状态，禁用静态公共 Key；通过正常管理路由创建 normal Key，再设置 `message_prepaid`，购买总量和剩余次数均为 2。公共请求携带创建接口返回的 token，经正常 router、鉴权、路由、预扣和终态结算，访问合成 Grok 身份 GET 与 WebSocket peer。不是直接调用私有结算函数，也不是管理 probe。

新增 3 项测试，共 10 个组合：

- 成功：Chat/Responses × buffered/SSE，共 4 组合。收到合成答案，唯一 completed 审计归属新 Key、exact 账号和 grok-chat-fast；剩余次数 2→1，购买总量不变。
- 上游终态前关闭：同样 4 组合。buffered 不返回成功，流不伪造完成标记；唯一 failed 审计，最终剩余次数恢复为 2。
- 客户端收到增量后取消：Chat/Responses 各 1 组合。实际读取“合成答案”，取消前剩余 1，drop 响应 body 后 peer 观察 WS 关闭，唯一 cancelled 审计，剩余恢复为 2。

所有组合都确认恰好身份 GET+WS 两次调用、三个客户端 WS 事件、exact Cookie/UA、无 Authorization，无账号或渠道重放。审计 prompt/completion/total tokens 均保持 None；未知 token usage 不妨碍已有按次数成功收费语义。审计结束并释放异步状态引用后才读最终余额；每个 fixture 请求 shutdown、关闭 SQLite 和 freebuff，peer 正常结束。

本批下游使用 router oneshot/body drop，不声称新增真实下游 TCP 计费断连证据；旧 TCP 合同已作为相邻测试复验。`build_app_state` 不等同于成品 EXE、完整服务启动和全部后台 worker 验收。

## 验证与来源

- locked/offline、runtime-defaults、4 jobs 的 lib 测试构建通过。最初两次构建因嵌套测试模块缺显式 path 声明失败，保留原日志；修正测试声明后第三次构建通过，未改生产可见性。
- 新 prepaid 3 passed；既有 pipeline/TCP 6 passed、网页 opt-in 宿主 1 ignored；管理 probe 2 passed；access_balance 相邻 3 passed。合计 14 passed、1 ignored；本批未重复网页验收。
- checker 31 passed、最终 ratchet、rustfmt、限定 whitespace、UTF-8 无 BOM 检查通过；没有改 baseline 或 exception。
- 父模块有效行 251→253；新增 prepaid.rs 87 行，prepaid/fixture.rs 153 行。
- Cargo compiler-artifact 实际返回的测试 EXE 复制并核验，SHA256：`1c66389db052193cd4262203a6a1e4326e109d53822058a16d7356c4ff287553`。
- 最终全 src 输入核对仅 `src/token_refresh/tests/nous_probe/inflight.rs` 有并行漂移；Grok 和公共计费直接输入无漂移。结果绑定冻结测试 EXE，不宣称其他工作线当前内容全部验证。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-prepaid-20261112`。包含 build-3.jsonl、tests.exe、prepaid.log、pipeline.log、probe.log、balance.log、checker.log、ratchet-final.log、inputs-final.json 和 input-drift.json。TEMP/TMP、合成配置和数据库均留在该目录；不删除用户数据。

## 未验边界

本轮关闭的是 Grok 普通 Key 的 message_prepaid 成功、失败、增量后取消的本地合同。Qwen 同等级普通 Key 验证、token_prepaid、现金定价和账本、供应商真实扣额、PG/Server、进程崩溃中的预扣恢复、成品包及真实平台验收仍未由本批证明。

新增模板仍 17，无新增整卡 offline_ready，P50/P60/P70 未完成。没有真实凭据或付费上游调用，没有修改在线实例、构建应用包、提交或推送。
