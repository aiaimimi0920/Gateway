# 分渠道扩展：P01 材料保存与标准模板检查点（第 2 批）

> 2026-11-02；总目标仍为[完成整份扩展计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)，不是只完成本批。
> main；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。延续 [P00/P01 第一批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p01.md)，未提交或推送。

## 1. 实际实现

### 1.1 七个独立的正常模板

| R | template / default provider ID | preset | 本批准确材料 |
| --- | --- | --- | --- |
| R03 | `grok-web` | `grok` | SSO 值 / 完整 Cookie；不复用 xAI API 或 Grok Build |
| R07 | `qwen-web` | `qwen-web-chat` | Web auth token + 可选 Cookie；signin seed 本批未导入 |
| R13 | `chataibot-web` | `chataibot` | token Cookie；图像渠道，不冒称通用聊天 |
| R14 | `lumalabs-web` | `lumalabs` | wos-session + 必需 realmId；不改指 Agents API |
| R15 | `suno-web` | `suno` | 含 __session 的完整 Cookie；不把短期 bearer 当长期材料 |
| R16 | `udio-web` | `udio` | SSR Cookie 值 / 完整分片 Cookie，分片不改写成 bearer |
| R06 | `freebuff-web` | `freebuff` | FreeBuff authToken + agentId；不冒称推理 API Key/OAuth |

目录正常创建 `explicit_credentials_only: true / credentials: []`；无默认账号、权益或模型目录证据。
保留独立 provider/credential 归属；同品牌 vendor 展示不合并凭据池、额度、认证或验收。
模板默认不测试、不补号/清理、不开启未匹配产品的健康探测；媒体仅提供合适的无文本标准答案提示词。
目录插入规则改为显式 anchor，Grok 紧邻 xAI API；其他新品牌不误插进 Gemini 区段，也不重复模板。
没有新增 `TEMPLATE_ADAPTATION_RECEIPTS` 或蓝色完成登记。

### 1.2 产品材料 → 普通 editor → SecretPatch

新增职责模块：

- `providerSessionProducts.ts`：固定产品身份、preset、受众、材料说明与目录元数据。
- `providerSessionMaterial.ts`：128 KiB UTF-8 上限、严格字段投影、产品绑定、Cookie/token 分离、有效期与各产品必需字段校验。
- `providerSessionTarget.ts`：暂存和提交共同使用的精确 HTTPS origin/path、执行器、realm/agent、认证结构及有效期保护。
- `providerSessionMaterialApply.ts`：普通文档仅写非秘密 metadata；token/Cookie/旧认证清理走现有 edit/patch owner。
- `ProviderSessionMaterialField.tsx`：粘贴与文件读取、授权门控、临时明文清理、generation 隔离晚到结果。
- `providerSessionCatalog.ts`：七模板、媒体测试提示词和明确目录 anchor。

`CredentialDialog`、`ConsoleGlobalDialogs`、`useCredentialDialogEditor` 已接生产入口；材料不经过通用 API Key 表单、不调用通用发现、不借用 OAuth 保存链。
编辑默认 keep 仍可只改 metadata；已有账号未授权时也能从对话框请求授权后替换。授权失效、切产品、关闭/切编辑会话均清理临时材料。
已有 Qwen 恢复 seed 的账号拒绝从此入口替换，避免旧恢复身份覆盖新 token；不悄悄删除旧 seed。
FreeBuff model-agent map 目前两层均拒绝，不把绑定的 agentId 被 map 覆盖当作成功；完整映射导入留作后续合同。

替换时清理旧 Cookie、Authorization、auth_token、refresh_token；有旧刷新 endpoint/client/lifetime 配置则拒绝混用，而不是套通用 OAuth。
每个秘密 edit 只附非秘密产品绑定，真正构建 SecretPatch 时再次检查；暂存后改目的地/产品/realm/agent/map/refresh/session_auth/expiry 都会拒绝。
`useConsoleRouteDraft` 的渲染预览剔除不安全暂存值、不抛错崩溃；真正 buildCommitRequest 对最终 document 严格复核，包括 secretPatchOverride 调用者。

共享 Cookie 提交接缝同时修复：后端 identity/path 大小写敏感，旧 `cookie/COOKIE` 各自 clear，再对规范 `Cookie` replace，不能只删除旧 keep。
相同保护用于 Authorization，保留无重复精确路径的补丁；没有修改后端秘密分类、schema 或数据库版本。
格式、字段、安全边界及明确未实现项详见 [Session 材料合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-session-material-import.md)。

## 2. 实测与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p02`。

| 检查 | 实际结果 |
| --- | --- |
| 首次材料红测 | 两个新纯模块尚缺；UI 5 项中 4 失败，复现通用表单/无专用入口；material-red.log |
| 模板红测 | 七模板和正确目录位置尚缺；catalog-red.log |
| 审查修补红测 | 13 项中 7 失败，复现 credential agent-map、旧 refresh、session_auth/expiry 重定向与 edit keep 无解锁入口；binding-review-red.log |
| 真正提交 owner 红测 | 1 项失败，复现错误目的地导致渲染 useMemo 抛错；commit-owner-red.log |
| 最终前端相关回归 | **22 files / 209 passed / 1 skipped**；frontend-regression-final.log。跳过项是已有显式私有 ChatGPT worker sample 测试，没有提供私有路径 |
| 前后端合成贯通 | 七种材料从真实 TS parser/apply/buildPatch 导出，再交真实 Rust resolver；4 tests **passed，0 failed/ignored**；rust-session-final.log |
| 扩大的 Console 文档合同 | **65 passed / 1 failed / 0 ignored**；rust-regression.log。失败详见下节，不宣称该套全绿 |
| TypeScript | `npm --prefix apps/desktop run typecheck` exit 0；typecheck-final.log |
| Web bundle | 使用生产 rsbuild config，仅将 output 指向 linshi 并移除发布/清理插件；exit 0，web-build-final.log。没有清理或发布现有 dist/web |
| Rust 格式 | `cargo fmt --all -- --check` exit 0；format-final.log |
| 检查器测试 | **31 passed，0 failed/skipped**；checker-tests-final.log |
| 有效行 ratchet | 同一官方 checker 的无 JSON 报告写入入口 exit 0；3888 files、14 immutable runtime artifacts、governed source >700=0；ratchet-final.log |
| 编码/文档/diff | 24 源码 + 6 文档严格 UTF-8 无 BOM，56 个去重本地引用存在，代码围栏平衡；tracked diff 与所有本批含未跟踪文件的 scoped whitespace 检查通过；document-validation.json / diff-check.log |

前后端贯通使用显式 `GATEWAY_SESSION_MATERIAL_FIXTURE_PATH`，文件仅含合成凭据。
覆盖旧大小写 Cookie/Authorization 清除、旧 refresh_token 清除、数组重排重绑定、realm/agent metadata、YAML 持久化读回、再次 redact+keep 读回。
这是进程内真实 parser/resolver/序列化合同，不冒称启动 EXE、数据库事务、revision 冲突、HTTP wire、原生 UI 或真实平台验收。
首次 Rust test-profile 编译 6m21s；增量合同编译 35.86s。未构建 devtest EXE，不外推日常包提速。

### 2.1 保留的失败，不靠缩小报告掩盖

`resource_limits::legacy_compiler_rejects_extreme_token_expiry_without_panicking` 期望旧编译器拒绝 `u64::MAX` lifetime，但当前 `refresh_state.rs` 将 lifetime 钳制，编译成功，断言失败。
实际错误：`legacy compiler must reject an unrepresentable expiry`。
`tests/console_document_contract/resource_limits.rs` 和 `src/routing/config/refresh_state.rs` 与当前 HEAD 均无 diff（检查 exit 0）；本批不修改它们，也不把该结果称为全套回归通过。
本批材料入口不接受 `token_expires_in_secs / refresh_endpoint / refresh_client_id`，不使用此旧 lifetime 配置；新的四个 Session 合同在最终代码/五类补丁版本下重新通过。
后续 OAuth 刷新工作包应统一该旧断言与现有 lifetime 契约，不能通过忽略或篡改报告关闭它。

## 3. 范围与有效行

现有 owner 的本批前 → 后有效行：

| owner | 前 → 后 |
| --- | --- |
| CredentialDialog.tsx | 404 → 421 |
| useCredentialDialogEditor.ts | 320 → 327 |
| ConsoleGlobalDialogs.tsx | 285 → 287 |
| credentialDocument.ts | 453 → 468 |
| useConsoleRouteDraft.ts | 338 → 341 |
| providerCatalog.ts | 429 → 429 |
| providerWebCatalog.ts | 27 → 29 |
| providerCatalogWorkflow.ts | 116 → 118（承接第一批 116，而不是 HEAD 的 127） |
| chatgptWebMaterial.test.ts | 67 → 70 |
| tests/console_document_contract.rs | 43 → 45 |

新增职责与测试模块均不超过 150 有效行，最大为后端 Session 合同 135；精确列表在 effective-before.json / effective-after.json。
本批源码没有新增依赖或 feature，不修改 S06/Gemini、既有执行器 wire、路由/计费/健康、运行数据库或旧 preset/ID。
最终格式、checker/ratchet、编码和 scoped diff 检查结果由同目录日志与 document-validation.json 记录；不覆盖原有脏的 JSON 有效行报告。

## 4. 当前状态与下一步

材料保存子链和七个模板已实施，不将各 W 卡整体升级为 offline_ready，更不将总计划标完成。
Accio/Kiro/Grok Build/Vertex OAuth 生命周期、Qwen 普通 API、Bedrock 多字段认证、Vertex Express、Luma Agents、新 Perplexity Web transport、各 Web 录制/刷新/完整媒体线仍需推进。
Qwen 原免费 OAuth 的停用约束继续追踪，未移出用户需求；Qwen seed 的受控导入/更新、FreeBuff CLI/fingerprint 和 map 模式继续独立开发。
下一步先完成普通 API/多字段认证的最小闭环，再推进已固定合同的 grant/refresh 和缺失 Web wire；复用本批安全提交接缝，不重起保存或第二套网关。
尚未读取/试用任何历史凭据、发起授权/刷新、调用实际上游、产生费用、重启在线 4200 或制作 EXE/包。集中录制和逐渠道真实验收仍在后半段。
没有提交、推送、重置、删除旧包或改写其他人的改动；Vitest 缓存原已脏，测试可能更新其生成结果，不据此撤销它。
