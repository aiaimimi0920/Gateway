# Kiro OAuth：显式 profile 的 runtime 兼容调用与正常模板

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
继承 `main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981` 的大量未提交改动；HEAD 不是本子批源码交付凭据。

## 改了哪些内容

原 OAuth sender 使用旧 `/generateAssistantResponse`，而固定公开参考使用 `runtime.<region>.kiro.dev/`、AWS target 和显式 profile。
本批不在旧路径上直接加一个同名模板，而是增加可区分的 `auth_mode: kiro_oauth_runtime`，仍复用 `kiro` preset、三适配器、正常 dispatcher 和原有生命周期。

1. 新增正常 `kiro-oauth` 空池模板。输入 Profile ARN 后仅据其 Region 确定业务地址；不默认模型、scope、订阅或凭据，不请求 Secret Grant 来创建空池。
2. OAuth runtime 与 CLI Key 的所有权显式分离。业务请求为固定 runtime-root POST、原生 EventStream；携带精确 profile/model，移除仅属 Key 的 `TokenType`，晚到的 header/body/目标变更被最终发送边界拒绝。
3. optional `runtime_target` 进入 credential-owned 注册 context；无该字段的旧 context 保持原序列化。目录 Region、推理 Region、profile 分别处理；普通设备授权保存通过已有 SecretPatch/CAS 绑定实际 consumer，含同 Region 的 credential profile override。
4. 沿用原 scheduler、durable grant、publication/rebuild 和 cache repair。typed context、业务 base/mode/profile 均参与受众匹配；不能以同一 OIDC 注册向另一 profile 发布旧刷新结果。
5. 精确模型测试沿正常 sender、recording/health owner；默认检查和 generic models discovery 不发生成请求，也不允许 `modelsPath` 把 OAuth Bearer 导向任意 URL。Key 旧路径保留。
6. 共享纯请求形状校验在 Kiro feature 关闭时仍可编译；关线不启用网络或 packing。

OAuth 的 HTTP/解码失败不返回供应商正文、不自动重试或换账户。buffered/stream 复用既有有界 EventStream reader；不把共享 Key decoder 的估算 token 当 subscription credits。

## 来源与验证

本批参考固定公开 commit，而不是宣称已经获得当前平台的官方完整推理合同。
精确来源及采用边界见 [REFERENCE.md](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_oauth/REFERENCE.md)。
没有运行或安装参考项目，没有读取 `.ng`、历史凭据、真实 Kiro 数据库或调用真实 OIDC/模型。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-oauth-runtime-20261107`。
保留最初的运行红测（OAuth 被误判为 CLI Key）、关线 `E0425`、错误 Vitest root 的失败以及中间来源漂移记录，不把早期编译或部分绿测当最终验收。
最终结果以本目录的阶段回执及 `audit.json` 为准。

### 最终定向验证

| 检查 | 实际结果 | 回执 |
| --- | --- | --- |
| 正常 `runtime-defaults` 的 Kiro 库测试 | 203 passed / 1 ignored | `kiro-final-v2.json` |
| 同次五组 integration 的 `kiro` 名称筛选 | 30 passed | 同上 |
| context integration 中不含 `kiro` 的三个顶层测试 | 3 passed；只补遗漏，不重复前 12 项 | `context-unfiltered-v1.json` |
| 仅关闭 Kiro 实现线的生产库编译 | passed | `disabled-final-v1.json` |
| 正常桌面组件定向回归 | 5 文件、35/35 passed | `ui-final-v2.json` |
| TypeScript 类型检查 | passed | `typecheck-final-v1.json` |
| 官方 Rustfmt | passed | `rustfmt-final-v2.json` |
| checker 自测 | 31/31 passed | `checker-final-v1.json` |
| 官方有效行 ratchet | passed；4784 文件，14 immutable runtime artifacts，governed source >700=0 | `ratchet-final-v2.json` |

Rust 按完整测试名去重为 **236 passed / 1 ignored**，其中本批正常配置新增 15 项；另有 3 项新增 UI 测试包含在 35 项内。ignored 是既有 CLI Key 浏览器 fixture host，未显式运行，不计通过。没有运行全仓测试、完整 feature 矩阵、Server/真实 Redis/PG 或原生包验证。

最初 OAuth 被误判成 Key 的运行红测、关 Kiro 的 `E0425` 均已修复并有最终绿测。中途库测试唯一失败是新 probe 测试把缺 prompt 的非空 options 当作默认请求：生产 parser 正确返回 400，修正测试预期并另走真实 empty-body 默认入口，未放宽生产语义。首轮 Vitest 的 `document/localStorage is not defined` 来自错误 root，正确 desktop root 下 35 项通过。早期源码漂移和既有编译 warnings 原样保留，不报全仓零 warning。

45 项审阅源码经官方 lexer 复算与逐项记录一致，最大 496 有效行；`providerCatalog.ts` 484→488、`request_plan_part_2.rs` 494→496、`streaming.rs` 466→467、`headers.rs` 455→461。未改 baseline/exception。五个终态后端/静态阶段各 5097 项所选输入在阶段内无漂移，且最终 hash 一致；前端、类型和 checker 按各自消费的源码子集核对，早期扫描未包含 integration tests，不追认全仓冻结。

最终 UTF-8/无 BOM、本地链接、六份固定公开原文 hash、范围 diff、`git diff --check` 与来源审计见 `audit.json`。ratchet 的本批报告单独归档，再从本批开始前的直接 preimage 恢复共享 `artifacts/effective-code-lines.json`，保留继承的 dirty 报告。

## 接力与未完成

正常模板与本批限定 runtime 子链不等于 R02/W02 整卡 `offline_ready`，也不证明实际订阅、当前最小 scope、profile 发现/选择、Social 登录或第三方客户端使用资格。
旧 route/未变 source hash 的 DB 自动兼容、runtime 模板 UI→真实管理 TCP→公共调用/结算的联合验收、完整取消 audit、Server/真实 Redis/PG、跨应用 receipt 与原生包仍须继续；不借先前 CLI Key 的这些证据替代 OAuth 验收。

不静默迁移旧 OAuth/Key 账号，不改凭据或数据库版本，不替换运行实例、不出包、不提交或推送。
P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道真实验收继续。
历史 P34 Redis 授权已使用并清理；本次问卷回放不触发新容器，也不复用授权。
