# Vertex Express 公共调用、终态准入与本地结算子批

日期：2026-11-05。仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：本子批定向无凭据实现与验证完成；不是 R11/W07、P50 或总计划完成。
承接 [Console 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-console.md)。
HEAD 为 `7969f12c85efceacf2a5e384e143b5c39a550981`，保留写前 965 项 dirty entries，不提交、推送或清理。

## 1. 本批边界

公共请求使用生产 `build_app_state` 的 local 模式、真实 SQLite、正常管理 API 创建与充值的普通预付费 Key；
不用无限额 master Key 证明扣费，不手工写数据库，不使用合成 storage backend。
公共 router 在进程内通过 `oneshot` 调用；这不是正常 UI→管理 TCP→SQLite 的完整集成，也不是真正进程重启。
上游仅在原产品 plan/header 校验之后沿既有 `cfg(test)` 接缝转到不转发的 loopback HTTP server。
临时数据与证据目录为 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-pipeline-20261105`。

主配置 Cargo 使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，代表性关闭只移除下述 Vertex leaf feature；显式清除 `RUST_MIN_STACK`。
不新建增栈线程、不调用真实 Google、不读取真实 `.ng`/历史凭据、不启默认 Web 发布清理 hook。

## 2. 实现与已证红测

1. 新增公共三协议 JSON/聚合 SSE、错误、普通消息/Token 额度及取消测试。
   test-only server 以任意七字节边界分片，支持 hold-open 与真实 response-source Drop 计数。
2. 新增 Express 响应准入 owner：沿现有 64 MiB bounded reader，检查 SSE/UTF-8/JSON、完整 frame、真实 candidate finish，
   拒绝空流、仅 `[DONE]`、未结束/损坏尾部、晚错误和 finish 后新内容；不能靠 EOF 补 `stop`。
3. HTTP 错误沿原 bounded classifier 保留 kind、timing 与恢复提示，但对外/入库使用固定产品消息与 code，
   不透传任意上游错误正文或 code。未知/无 total usage 不伪造零；显式零、总量和 cache-read 原值保留。
4. 仍复用未修改的共享 Gemini `parse_response` 和原 canonical SSE bridge，不更改 S06 编码器/parser/translator 内部。
   当前只准入单 candidate；多 choice 显式拒绝而非合并，**不是多 choice 能力完成**。
   `thought=true` 不混为普通答案，但 signed reasoning、完整 tool/schema/阻断保真仍未完成。
5. 默认栈实测暴露真实问题，先在多服务商 buffered/streaming 入口使用同任务 boxed future；
   仅 pointer-size 断言不足以证明实际 poll 安全，后续仍以真实公共路径为准。
   Express 的 SSE 入口直接复用 buffered 聚合与原 canonical SSE bridge，避免先进入第二个多服务商 streaming dispatcher；
   不增线程栈、不创建新 task，不把聚合响应称为实时 token 流。

失败记录均保留，不把编译成功或部分用例通过写成整批通过：

- `red.log`：夹具创建 Key 漏 `publicKeyPrefix`，422；这是测试设置错误，不是业务红测。
- `red-v2.log`：正常 Token/SSE 路径出现 `0xc00000fd / STATUS_STACK_OVERFLOW`，测试进程中止。
- `red-v2-incomplete.log`：单独运行有效夹具，空上游流真实返回 200、空答案和 `finish_reason=stop`，行为红测成立。
- `red-v2-vertex_express_pipeline_http_errors.log`：上游正文中的合成 Key 与私有错误文本进入公共错误，行为红测成立。
- `green-v1.log`：新响应模块的可见性/借用编译错误（E0364/E0603/E0502），已在后续修正，不计行为结果。
- `green-v2.log`：默认栈精确 probe 溢出；`green-v2-success-isolated.log` 证实公共 JSON 已返回 200，SSE 尚溢出。
- `green-v3.log`：原精确 probe 和 pointer-size 用例通过，但正常公共 Token/SSE 仍溢出，不能宣布问题解决。
- `frame-diagnostic.log` 的实际类型大小为 buffered 35456、streaming 32360、direct stream 4920、wire stream 14592、
  native Claude 3800 字节；不是数 MiB 的 future。类型大小只辅助定位，不代替默认栈真实 poll。
- `green-v4.log`：28 passed / 2 failed；三协议 JSON/SSE、取消/source Drop 和 42 种不完整/坏流公共组合已通过。
  正常 Token/SSE 已返回 200，但测试读取 completed audit 后未等最终结算，读到预留后 96 而非结算后 95；
  生产 stream finalizer 的 audit 在余额结算之前，故测试改为有界等待 terminal state owner 释放，不改余额期望或生产顺序。
  HTTP 错误测试另将框架允许的 429 重试误判为 sender 重试，并在默认恢复等待中耗时 300 秒；
  现用既有 `request_wait.mode=disabled` 隔离 HTTP 错误投影，保留正常有界重试，并逐次核对精确账号、Key、路径和 header。
  测试公共请求加 10 秒上限；不因测试而关闭生产默认恢复策略。

## 3. 当前通过的验证

- `green-v5`：30 passed / 0 failed / 0 ignored（12 新增）。包括六种公共成功组合、42 种坏流组合、十种 HTTP 错误组合、
  buffered/聚合 SSE 取消和普通 Token Key 结算；合成 usage=5 时两条路径余额均为 100→95，不是 Google 实际账单。
- `affected-lib-v5`：122 passed / 0 failed / 0 ignored。共享 Gemini API、registry/call_flow、默认栈实际 dispatcher wire、
  DeepSeek/Mistral/xAI/Perplexity API loopback、probe、重试及恢复等待策略定向回归通过；没有复用旧增栈 pipeline runner。
- `affected-contracts-v5`：13 passed / 0 failed / 0 ignored。正常 Express 管理 2、Gemini probe 1、Gemini Web probe 1、
  通用凭据 probe 6、本地 storage 3。后者使用现有普通 API fixture，公共 router/真实 SQLite/同进程 runtime 重建，
  **不是 Express 专属重建或 Gateway TCP/UI 验收**。
- 主配置两批 lib 与五个独立 integration target 共 165 个不同测试通过。
- `feature-off-v5`：从 runtime-defaults 仅移除 `line-google-agent-platform-official`，11 passed / 0 failed / 0 ignored。
  明确 marker 拒绝与 preset 不存在、Keep/材料和纯响应准入通过；与主配置重复项不累计为新的不同测试。
- `green-v5` 冻结 2584 项选定源码/配置/夹具输入；扩大回归另包含全部 Rust integration 输入，冻结 2908 项，
  每阶段前后 SHA 检查未变；最终审计再次核对四阶段已测输入，未变。
- checker 本批已有 31 passed，checker/policy 未改，不重跑；`ratchet-final.log` 扫描 4381 文件，
  14 个 immutable runtime artifacts 单列，governed source >700=0。13 个源码 owner 均低于 500 有效行；不重建 baseline。
- Rustfmt、Git diff/新增文件 whitespace、17 项选定源码/文档严格 UTF-8 无 BOM 与来源核对通过；
  最终无本批 Gateway Cargo/rustc/test 进程残留。具体阶段参数、退出码与 SHA 见各 `*-inputs.json`、阶段 JSON 和 `final-audit.json`。
  Cargo 仍有已有及关闭配置的 unused/dead-code warnings，不宣称无警告、全仓回归或正式发布门禁通过。

## 4. 改动与资源/安全复核

新增五个模块：公共 pipeline fixture、失败/取消/Token 合同、future 所有权回归、Express response owner 及其测试。
已有八个 owner 仅接线/默认栈所有权、test-only 分片 transport 与 candidateCount 网络前准入；未改共享 S06 内部、依赖或存储版本。

| 已有文件 | 写前→写后有效行 |
| --- | ---: |
| `src/upstream/client.rs` | 477→481 |
| `src/upstream/client/request_plan_part_1.rs` | 164→164 |
| `src/upstream/client/request_plan_part_2.rs` | 449→479 |
| `src/upstream/client/streaming.rs` | 382→429 |
| `src/upstream/client/vertex_express_test_support.rs` | 123→160 |
| `src/upstream/client/vertex_express_wire_tests.rs` | 149→172 |
| `src/upstream/vertex_express.rs` | 31→33 |
| `src/protocol/vertex_express.rs` | 153→155 |

五个新增文件分别为 261、209、36、232、119 有效行，完整路径与计数见 `effective-lines.json`。
响应处理保持已有 bounded body owner；固定错误不带上游秘密；同任务 boxed future 保留 Drop/cancellation，不引入 detached task/线程。
loopback transport 仅 test 编译，仍在生产 plan/header 校验之后替换；失败 seam 只能到拒绝性 loopback proxy，不访问真实 Google。
本批临时目录均在 linshi；普通完成/取消测试均关 SQLite/FreeBuff 与测试 server；没有新建 Redis/Docker 或运行线上服务。

## 5. 后续接力

- 继续完整 decoder 的工具/schema、signed reasoning、阻断、多 choice 保真；当前严格准入不能代替这些能力。
- 继续正常 main UI→真实管理 TCP→SQLite 保存、公共调用与 Express 同目录 runtime 重建；不要复跑已完成的 Console mock 保存当作该证据。
- 不以本批替代完整 decoder、正常 UI/管理 TCP/SQLite 保存与 runtime 重建、PG/Redis/Hosted、进程/OS 重启、EXE 出包或真实权益验收。
- Full Vertex OAuth 的独立 client/scopes/redirect、project/location、typed context/Bearer/持久刷新仍独立未完成。
- 累计模板仍 14；不增加整卡 `offline_ready` 或真实验收标记，不复用历史 Redis/Docker 授权。
