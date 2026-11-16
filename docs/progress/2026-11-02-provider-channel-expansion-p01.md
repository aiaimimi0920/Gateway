# 分渠道扩展：P00 调查与 P01 执行器接缝检查点

> 2026-11-02；目标仍是完成[总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)，不是只完成本切片。
> 分支 main；执行基线 HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。未提交、推送或接管运行实例。

后续材料链和七个 Session 模板已继续实施，见 [第 2 批检查点](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p02.md)；本文的“尚未开放模板”等记录是第一批结束时的历史状态，不代表最新状态。

## 1. 本批实际产出

### P00：有来源的身份与授权合同

- 建立[渠道身份表](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-channel-identity.md)，覆盖原 R01–R16 和同平台额外 API 分支；API/OAuth/Web 不是排他顺序。
- 新增[授权实施合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-auth-contracts.md)，固定 Grok Build、Accio 参考、FreeBuff 官方源码 commit、文件与许可，记录精确 endpoint 和材料差异。
- 已确认 xAI API/Grok Web 分开计费；Qwen 普通 API/Coding Plan 不混用；Bedrock 同 account/region/model/runtime 的 quota 不因认证方式不同自动变成独立份额；Luma Agents 同 client 多 Key 共享 quota。
- 新发现 Accio PKCE/专有 JSON exchange+refresh 参考，FreeBuff 官方 CLI/browser-assisted session 合同，Grok 独立 OAuth CLI proxy。没有把客户端身份许可、未知额度或未实测产品当作已冻结。
- P00 **仍在执行**：Kiro 产品 scopes/profile、Accio 客户端合同、Grok client 使用边界、Perplexity Web wire、若干套餐/额度关系继续调查；原需求没有删除。

### P01：九类现有产品接入显式 typed flow

新增运行时 `SpecialAdapter`：`accio / kiro / freebuff / grok_web / qwen_web / chataibot_web / lumalabs_web / suno_web / udio_web`。
均指向真实既有 preset/adapter，不新造执行器、不把 OAuth 当指纹、不把 Web token 换投官方 API。

前端新增 `providerSpecialAdapters.ts`，统一拥有显示、preset 绑定及产品指纹识别；`providerCatalogWorkflow.ts` 保留原导出入口。
指纹读写按已保存 preset 或显式 flow 判断，不依赖可改 Provider ID。新建与自定义投影均使用产品执行器和 session 指纹。
旧无 flow/auto 产品的界面也锁定其 sender 归属；写入函数拒绝 API 指纹覆盖且不修改草稿。
Codex preset 的可选 Basispoints 明确不由 preset 自动推断；不误锁普通 Codex API。

运行时复用原 preparation 的错配拒绝，配置编译在发布前拒绝跨产品 adapter、固定 API 格式及产品不支持的 API 指纹。
未改 `preparation.rs`、认证、路由、计费、媒体执行器或 S06 Gemini lane。

## 2. 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p01`。

| 检查 | 实际结果 / 覆盖 |
| --- | --- |
| 前端首次红测 | 18 项失败，复现九类产品默认 none/指纹可写；见 frontend-red.log |
| Rust 首次红测 | 4 tests，3 fail/1 pass；失败为未知 special variant，legacy 无 profile fixture 通过；见 rust-red.log |
| 旧配置指纹红测 | 新增 preset-only/auto 测试先失败；见 legacy-fingerprint-red.log |
| Rust call_flow 定向回归 | **40 passed，0 failed/ignored**；包含九类真实 preset、跨产品拒绝、空/双账号编译和现有 send 路径 loopback 合同；见 rust-green.log |
| 前端受影响回归 | **6 files / 75 tests passed**；含产品、创建、原流程、指纹、Antigravity 和 pilot editor；见 frontend-green.log |
| TypeScript | `npm --prefix apps/desktop run typecheck` exit 0；见 typecheck.log |
| Rust 格式 | `cargo fmt --all -- --check` exit 0；见 format.log |
| 检查器测试 | **31 passed，0 failed/skipped**；见 checker-tests.log |
| 有效行 ratchet | 同一官方 checker 无 JSON 报告写入入口通过；不覆盖原有脏报告；见 ratchet.log |
| 全 feature-disabled 编译 | `cargo check --locked --lib --no-default-features --jobs 4` **exit 101**；错误来自本轮未修改的 Gemini speech/upload 等缺失符号；见 feature-disabled-check.log，未越界修改 S06 |
| 新产品 feature-disabled 编译 | `cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` **exit 0**；九类新增绑定对应的产品 feature 均关闭，仅保留 Gemini；不替代上一项全关失败；见 product-features-disabled-check.log |
| 编码/文档/diff | 13 个本批文件严格 UTF-8 无 BOM、54 个去重本地引用目标存在、代码围栏平衡；tracked 与单文件含未跟踪源码的空白检查通过；见 document-validation.json |

Rust 命令：`rtk proxy cargo test --locked --lib call_flow --jobs 4 --no-default-features --features runtime-defaults`。
40 是 test harness 的测试函数数量；函数内部九类/交叉矩阵不另造一个“测试总数”。4096 个不匹配过滤器的测试未运行。
首次红测 test profile 编译 7m42s，绿测重新编译 7m24s；这不是 devtest EXE 构建，不外推打包速度。

兼容证据边界：完全无 flow 的旧 payload 保留原路径；auto 的逐字 fixture 断言只覆盖未设置旧 API profile 的材料。
既有 preparation 对产品 auto flow 的 request-local profile 收敛行为未改；不宣称所有旧 effective payload 均逐字不变。
这些测试**不证明真实 OAuth 登录、刷新、额度、模型可调用性、媒体生成或无浏览器执行**。

## 3. 有效行与改动范围

| 文件 | 本批前 → 后有效行 |
| --- | --- |
| src/protocol/call_flow.rs | 116 → 136 |
| src/routing/config/tests.rs | 95 → 97 |
| apps/desktop/src/features/console/providerCatalogWorkflow.ts | 127 → 116 |
| apps/desktop/src/features/console/requestClientProfile.ts | 38 → 40 |
| src/protocol/call_flow/product_tests.rs | 新增 93 |
| src/routing/config/tests/product_call_flow.rs | 新增 74 |
| apps/desktop/src/features/console/providerSpecialAdapters.ts | 新增 32 |
| apps/desktop/src/features/console/providerProductCallFlow.test.ts | 新增 80 |

官方 lexer 测量见 effective-before.json / effective-after.json；新增/修改源码均低于 150 有效行。
未新增依赖、feature、数据库字段或存储版本；旧 preset/Provider ID 不迁移，不新增凭据。
原有有效行报告、balance/codex/site-health 调查产物未主动修改；Vitest 缓存原已脏，测试运行可能更新其生成缓存，不据此撤销它。

## 4. 明确未完成

- 尚未为这批渠道开放新的标准模板，没有登记任何新的蓝色验收标记。
- 账号专用多字段材料、OAuth grant/refresh、普通 Qwen/Express/Luma API 新 owner、Perplexity Web 等仍需实施。
- P01 只完成显式执行器/指纹接缝，不等于材料、安全提交和全链集成均完成；各 W 卡不标 offline_ready。
- 未搜索、读取或试用历史凭据；未登录、生成、刷新、改数据库、重启实例、制作 EXE 或 devtest 包。
- 未运行全仓回归、真实上游或正式 release gate；不把当前定向绿灯当总目标验收。

## 5. 下一步最小任务

1. 补齐按保存 preset 绑定的多字段 session 材料 → SecretPatch → 正常提交链，随后开放对应标准模板，而不是先加名称。
2. 已有可复用接缝：`chatgptWebMaterial.ts::applyChatgptWebMaterial` 返回脱敏 document + secretEdits；`useCredentialDialogEditor.ts` 正常合并材料；`CredentialDialog.tsx` 持有临时材料并在授权失效时清理。扩展要形成独立产品 owner，不能把其他产品硬塞进 ChatGPT schema。
3. 对 Accio utdid/token、Suno Cookie+Clerk bearer、Grok SSO、Qwen signin seed、Luma/Udio Cookie 分别验证必需字段和错池拒绝；不从通用字段 api_key 推断认证类型。
4. 然后继续普通 API 与 OAuth 生命周期工作包；优先使用本轮已记录的固定合同，不重新从零检索，不复制未确认许可的项目。
5. 保持总目标 active；集中录制与真实验收仍在 P50 之后，不要求用户现在逐平台提供账号。
