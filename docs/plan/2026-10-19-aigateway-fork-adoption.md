# AIGateway fork 增量审计与 Gateway 采纳建议

日期：2026-10-19。状态：**只读源码审计和方案完成；候选功能尚未实施，不是发行或业务验收。**

## 1. 范围与真实基线

本次回答“本地 fork 相对原始 CPA 增加了什么，是否值得以 Neuro Gateway 的框架重新实现”，而不是再做一轮 CPA 最新版本的全量追平。

- `F/`：`C:/Users/Public/nas_home/AI/GameEditor/AIGateway/repos/CLIProxyAPI/`，真正的 Go 源码子仓。
- `G/`：当前独立 Neuro Gateway 仓库；下文未带前缀的 Gateway 路径均相对此仓。
- `B:`：官方固定 Git 对象 `555662940411a07460e9d24d14477a5f50dffdb5:`，不是工作树行号。
- fork HEAD：`ca5dc3a6e440411812cb74734e35b8957adcbdf0`。
- fork 当前分支：`migrate/local-final-netdiff-v7.2.116`。
- Gateway HEAD：`08634912809fea48b45fd888936cd688356fdfac`。

本地 fork 的实际 `origin` 指向官方 `router-for-me/CLIProxyAPI`，不存在文档所写的 `upstream` remote；`merge-base HEAD origin/main` 恰为上述 B。没有 fetch、改 remote 或整理分支。B 是可复现的本地官方祖先，**不表示今天的官方最新版本**；迁移分支的名称也不能替代 Git 对象基线。

在线核实了 [官方基线提交](https://github.com/router-for-me/CLIProxyAPI/commit/555662940411a07460e9d24d14477a5f50dffdb5)。旧文档中的 Arron196 仓库现重定向至 [BenjaminAaron196/CLIProxyAPI](https://github.com/BenjaminAaron196/CLIProxyAPI)，页面显示已于 2026-06-01 归档；不因此自动修改本地配置。

### 审计覆盖与归属限制

分别对比 B→HEAD、B→index、B→当前工作树，并将 untracked 源码加入清单：

| 集合 | 路径数 | 含义 |
| --- | ---: | --- |
| B→HEAD | 122 | 已提交净差，不等于当前有效源码 |
| B→当前已跟踪工作树 | 166 | 包含 staged additions 和后续 unstaged 覆盖 |
| 未跟踪源码、测试和交付脚本 | 53 | 排除了 output 与 .playwright-cli 产物路径 |
| 当前有效净差并集 | 219 | 166 + 53；不是 219 项新功能 |
| 仅历史已提交差异、当前已还原或删除 | 12 | 防止把撤回内容继续列为能力 |
| 上述去重总清单 | 231 | 111 个生产/交付模块、98 个测试或验证脚本、22 个合同/元数据文件 |

逐路径清单在 [2026-10-19-aigateway-fork-file-inventory.csv](2026-10-19-aigateway-fork-file-inventory.csv)。每项均有功能组 ID、Git 层次和当前净差标记；多功能模块允许多个 ID。清单表示路径归类和关键生产调用链核验，**不表示每一行都做过形式化审计或全部测试通过**。

当前基线上有 9 个作者记录为 `aiaimimi0920` 的提交。第一个是历史净树定制的整体迁入，不能把其中每一行都归为个人原创；有些是二阶 fork 继承、旧上游实现、测试重排或对官方新逻辑的回退。未提交源码也没有可靠的 Git 作者归属。因此下文准确称“本地 fork 定制增量”，并单列已证实的官方/插件上游功能。

## 2. 已提交的九个变更

| 提交 | 主题 | 当前语义或限制 |
| --- | --- | --- |
| `7a11af9a` | sync v7.2.116 customizations and WorkBuddy compatibility | 整体迁入；WorkBuddy 部分当前已撤回，不能整包认作有效增强 |
| `fdbaf728` | honor account-wide usage reset cooldown | 特定 FreeModel 402 限额提升为账号级冷却，防止换模型绕过 |
| `7f5a74f1` | preserve DeepSeek reasoning across Responses tool calls | reasoning carrier 和工具往返定制，见 D01/D02 |
| `27f13671` | retry transient failures in place | 同路由两波恢复策略，见 A01 |
| `ecf48f08` | bound transient upstream attempts | 限制每次 bootstrap 尝试，不等于整个请求有统一总上限 |
| `4994b548` | persist single-route transient recovery | 单路由持续恢复直至取消或错误改变，见 A02 |
| `1db70afa` | retry model capacity responses | 将特定容量错误识别为瞬态，见 A03 |
| `00a29649` | retry codex capacity stream failures | 同类容量语义进入 Codex 首包失败路径 |
| `ca5dc3a6` | release websocket session before retry | 调整 unlock 和 invalidation 顺序，是特定资源生命周期修复 |

## 3. 逐项能力对照和采纳决定

“建议”是工程取舍，不表示已经移植。“未找到”等价实现限于本轮检查范围，不能升级为全仓绝对不存在。

### A. 路由、重试、OAuth 和流生命周期

| ID / fork 增量 | 当前源码事实 | Gateway 对照 | 决定 |
| --- | --- | --- | --- |
| A01 同路由两波瞬态恢复 | `F/sdk/cliproxy/auth/conductor_transient_retry.go:16-25,47-58,125-196`：默认每波 6 次、1 分钟，1→10 秒退避，两波间等 3 分钟；带管理配置和热重载 | `src/retry.rs:31-55,94-109` 已有指数退避、jitter 和分类；`src/pipeline/stage_rate_limit/attempt_budget.rs:48-65` 共享尝试预算，每次重新 admission | **已有基础能力，不照搬多分钟默认值。** 应坚持一个请求预算 owner，不叠加独立重试层 |
| A02 单路由/严格 OAuth 模式持续恢复 | `F/sdk/cliproxy/auth/local_compat.go:64-99`、`conductor_transient_retry.go:224-240`、`conductor_execution.go:156-165`：符合条件时持续新恢复周期，独立于普通故障转移上限 | `src/pipeline/request_budget.rs:33-64,89-109,139-169` 限制工具/创建请求 replay，deadline 从原始开始计算 | **不采纳无限恢复。** 可取消不等于有界；保留总 deadline 和副作用发送上限 |
| A03 特定模型容量错误识别 | `F/.../conductor_transient_retry.go:242-313` 识别明确容量短语，即使状态为 400/429，也与普通无效请求区分 | `src/retry.rs:94-105` 对 400/401/403/404 有硬 veto，本轮未找到这些容量短语的 provider 专项分类 | **优先候选。** 仅在明确 provider 错误分类层识别；不开放所有 400，不绕过总预算/Retry-After/replay 限制 |
| A04 OAuth-first，禁止临时失败后偷跑 API key | **untracked** `F/sdk/cliproxy/auth/oauth_first.go:29-68`：开启且非 Home、无 pin 时，只要有合格且未耗尽额度的 OAuth，就强制 OAuth；不是普通权重偏好 | `src/routing/credential_routing.rs:129-134` 按凭据归属分优先级，并非 OAuth/API-key 硬筛选；未找到等价开关 | **有收益，建议做显式账号池策略。** 区分 strict 与 prefer/fallback；尊重 pin、授权范围和手动禁用，不能全局默认牺牲可用性 |
| A05 bootstrap 超时、撤销检查和成功流生命周期分离 | **untracked** `execution_recovery.go:11-58,65-95` 限制首包等待，空流为错误，处理 timer 回调竞争；`oauth_first.go:71-87` 重试前核验当前状态，但仍返回旧 auth，不是 token 刷新证明 | `src/pipeline/stage_send/stream_preflight.rs:32-38,74-108` 已有部分首包检查；`request_budget.rs:172-178` 对已交付流保留总 deadline | **采用为验收要求，不立刻新增第二套生命周期。** 补核所有 adapter 的首包/取消/撤销窗口；不得删除全请求 deadline |
| A06 上游 bootstrap 期间的 Responses heartbeat | **untracked** `F/sdk/api/handlers/openai/responses_bootstrap_keepalive.go:14-41` 在 conductor 尚未返回时即可写 SSE 注释；handler 独占写端 | `src/http/sse.rs:21-49,82-95` 已有流中 5 秒 heartbeat；`stage_send/streaming.rs:39-68` 先等待上游，不能据此证明 bootstrap 阶段等价 | **有条件候选，非立即默认开启。** 首次 flush 承诺 HTTP 200，后续错误必须合法转 SSE；需单独设计 header/status 承诺边界 |
| A07 FreeModel 402 账号级限额与 reset | `conductor_cooldown.go:813-815,965-972,1738-1760`、`selector.go:836-841`：特定 provider 的 usage-limit 影响整个账号，保留正 RetryAfter；WorkBuddy reset 文本解析已撤下 | `src/provider_health_state.rs:85-92,127-140` 有 quota/rate-limit/transient；上游等待提示当前只覆盖显式 RateLimited | **按真实 provider 合同补齐，不做全局 402 规则。** 不把旧文案解析当标准 Retry-After |

### B. API-key 保护和外部健康源

此组主要为 **staged/working tree 新增**，B→HEAD 对 routeguard/protection 范围为空，尚不能说已提交或部署。

| ID / fork 增量 | 当前源码事实 | Gateway 对照 | 决定 |
| --- | --- | --- | --- |
| B01 同账户组共享错误预算和发送 reservation | `F/internal/routeguard/engine.go:218-244,294-412`：默认 1 小时 10 次失败；pending 占预算，跨 alias/key request ledger；auth 保护层限制重试所有者和发送次数 | `src/provider_runtime/recording.rs:65-124` 已有 credential breaker；`stage_rate_limit/attempt_budget.rs:13-24` 已分 admission、send-start、observation；精确跨 key 账户组 reservation 不等价 | **值得分阶段设计。** 在现有预算/health owner 中增加显式共享组，不能再外挂一套互相打架的 breaker；Local/Server 都要并发与崩溃合同 |
| B02 崩溃保守记账和单次真实请求放行 | `engine.go:146-216,449-466`：恢复时 pending 算失败；状态故障 fail-closed；管理 AllowProbe 不主动调用供应商、不清历史 | `recording.rs:166-205`、桌面 CredentialTestManager 已有显式测试/探测，但不是“只放行下一个业务请求” | **有条件采用单次 half-open 准入。** 不能假装现有测试器缺失；写盘不能在全局请求锁中反复序列化全量状态 |
| B03 外部健康源适配器 | `F/internal/config/protection.go:33-50,70-120`、`routeguard/sources.go:34-65,95-215`：JSON、New API summary、channel-health、model-health-slot、model-status embed/all，带模型映射 | `src/pipeline/stage_route_health.rs:16-130` 主要来自本地 breaker/quota/runtime；未找到这些外部模板源 | **值得可选加入，但先做 advisory。** source 必须保鲜、有失败状态和范围绑定；默认不能因监控站宕机隐藏合法模型或拒绝路由 |
| B04 pricing-groups 与健康模型取交集 | `F/internal/routeguard/pricing_groups.go:12-62` 只读 model_name、enable_groups、auto_groups，**不读价格**；空样本/重复模型/多 group partial failure 采用保守口径 | `src/db/operator/pricing.rs`、`pricing_editor.rs` 已有明确价格 owner；外站分组不是 tariff 或授权 | **随 B03 作为源作用域适配，不独立做计费功能。** group 名 free 不等于零价，不推断 paid/free/suspect |
| B05 独立保护管理页、源预览/识别和绑定 | `F/internal/api/handlers/management/protection*.go`、`protection.ts:58-155`；数据接口在 management 鉴权组，静态页独立，CSP/no-store | Gateway 有自己的 console 组件和账户测试/运营入口，没有对应外部 source 配置页 | **采用功能语义，不采用压缩 JS/DOM 补丁。** 预览需防 SSRF/跨源认证泄露、限制源数/并发和响应大小，秘密 headers 不回显 |

特别注意 B03 的当前 fork 语义：`sources.go:67-93` 对 unconfigured/stale/缺模型/低成功率直接拒绝；`engine.go:294-303` 甚至在 `enabled=false` 之前检查 source。它不是 advisory。采纳适配器不等于采纳这一默认硬阻断边界。

### C. 模型证据与 usage 观测

| ID / fork 增量 | 当前源码事实 | Gateway 对照 | 决定 |
| --- | --- | --- | --- |
| C01 requested / upstream / declared model 证据链和最近 100 条观测 | `F/sdk/cliproxy/usage/manager.go:47-63` 六字段增量，经 `pluginhost/adapters_usage_translation.go`、`sdk/pluginapi/types.go` 透传；`redisqueue/plugin.go:22-68` 最近快照不消费原队列；管理 usage 与面板展示 | `src/routing/candidate.rs:351`、`stage_send.rs:272-278` 已有确定的发送模型；usage/report 没有同等 declared-model mismatch/evidence 观测 | **高收益优先候选。** 先记录明确模型 ID、来源和 alias 关系，bounded/no-body、流与非流一致；未知保留 unknown，不触发扣费、授权或健康惩罚 |
| C02 结构 fingerprint 学习与候选推断 | **untracked** `F/.../helps/response_fingerprint.go:14-50,142-203`：用上游自报 model 训练结构相似性，至少 3 样本、score≥0.35、领先≥0.08；不是校准概率 | 未找到相同推断记录；现有确定性路由元数据不等于模型真实性证明 | **暂不做自动鉴真或 suspect gate。** 训练标签不可信；profiles/feature map 未见容量淘汰，遍历需深度/字节限制。最多后续做显式“未验证候选”观测实验 |

官方 B 已有 Redis usage 队列、pluginhost、pluginapi、ResponseModel/ResponseServiceTier；本地净增是证据字段、推断和展示，**不是整个插件或 usage 架构**。

### D. 协议、提示词和供应商细节

| ID / fork 增量或对照发现 | 当前源码事实 | Gateway 对照 | 决定 |
| --- | --- | --- | --- |
| D01 DeepSeek reasoning/tool 历史往返 | `F/.../responses/reasoning_carrier.go:10-44`、request `:466-483`、response `:863-874`：自有前缀 Base64 carrier 写入 encrypted_content，只解码自家标记 | `src/protocol/responses/normalize.rs:98,107-169` 未处理同等 reasoning item 往返 | **优先补协议保真，但重新设计 carrier。** Base64 不是加密，不能伪装 OpenAI 的真实 opaque encrypted reasoning；必须限制体积、避免日志泄露和跨账号复用 |
| D02 tool namespace/custom input/ID 与输出邻接 | namespace/additional_tools/碰撞治理大部分 **B 已有**；当前 fork `responses_tool_index.go:103-116` 的 terminal 特例是增量，同时删了官方 NormalizeResponsesToolCallOutputs/AlignOpenAIToolCallMessages 等 | `normalize.rs:123-140,217-245` custom 仅取 arguments，未完整保留 input/namespace；`src/protocol/openai/pack.rs:95-106,243-258` 有更窄工具映射 | **建议以官方合同补齐 Gateway 跨协议缺口。** 不能归为 fork 全部原创，不能硬编码 terminal 例外；必须覆盖 orphan/重复 ID/交错工具输出与 allowed_tools 子集 |
| D03 text.format/structured output 和 incomplete | text.format→response_format、incomplete **B 已有**；本地部分 untracked 文件为提取重实现 | `normalize.rs:78-82` extra 只带 temperature/max_output_tokens/top_p；未见跨 Chat 的 text.format 转换；terminal/stream_support 已有 incomplete/完成证明 | **补结构化输出字段保真，不重复开发流终止机制。** 覆盖 text/json_object/json_schema、strict=false、raw schema；保留既有严格完成证明 |
| D04 不规范 SSE 和缺 DONE 放宽 | `F/internal/runtime/executor/openai_compat_executor.go:537-548,578-600`：无空行也拆 data 帧；见 finish_reason 后可在 EOF 补 DONE | Gateway 已有 SSE/BOM/Chat→Responses 完成证明（上一批已提交），不是没有流兼容层 | **不默认采用。** 仅对明确供应商兼容档位提供有界容错，不能把截断流一律变成功或破坏 usage/终态顺序 |
| D05 可配置 prompt 字符串替换 | **untracked** `helps/openai_compat_prompt.go:13-132`，已由 executor `:128-132,350-354` 调用；默认 system/developer/instructions，scope=all 修改所有 JSON 字符串值 | Gateway 规范化 instructions，但本轮未找到同语义字符串替换器 | **低优先、只做显式 provider 兼容策略。** all 会改 user 文本、工具参数、ID、URL，不是无损修复，不应默认启用或承担系统安全策略 |
| D06 Codex turn-state ticket 缓存/预热 | staged `codex_turn_ticket.go:16-169`，仅 OAuth+gpt-6-astra；全 repo 生产 Go 搜索 prepare/start/cached 符号仅有定义，executor 只初始化 map | 未找到同形缓存，但 fork 自身也未接线 | **不采纳为当前优势。** 独立 background ping 会真实调用/耗额度，脱离原请求；缓存未见容量淘汰，长度/前缀不是票据有效性证明 |
| D07 Kimi 嵌套 refresh token 兼容读取 | `metadata_helpers.go:6-43` 支持 metadata/token/Token 和 refresh_token/refreshToken；`kimi_executor.go:952` 实际使用 | Gateway 有原生 credential normalization/import 与 refresh owner；未逐一证明所有历史 Kimi 导入形态等价 | **作为导入 fixture 候选，不建平行读取体系。** 先确认实际受支持格式和现有 owner；保留 ID/秘密/来源 |
| D08 Codex account/client ID、模型抓取辅助 | `auth/codex/jwt_parser.go:106-113` 暴露首个 aud；`cmd/fetch_codex_models/main.go` 增加元数据容错；executor 测试与模型注册有相关调整 | Gateway 有 `protocol/chatgpt/codex_client.rs` 和原生 catalog/request-plan owner | **不把 JWT aud 自动视为已验证 client 身份。** 只按 provider 合同补 fixture；模型抓取不是新增 Gateway catalog 子系统 |
| D09 signature 旧符号兼容、replay cache eviction 小修 | signature_validation 净增 6 行别名；cache 净差是 eviction 排序保留 deleted tombstone、同 timestamp 按 key 稳定，不是整套 replay cache 新增 | Gateway Gemini/Rust 责任域由既有 S06 owner 保留；不能凭另一个 Go cache 名称制造新职责 | **不重复移植整套 cache。** 将 generation fencing/失效标记保留作为针对现有 owner 的验收参考，不接管 S06 |
| D10 避免 payload 与 original 重复转换 | 新 `request_translation_helpers.go:11-35` 有 equal-bytes 后复用与隔离拷贝；全 repo 生产 Go 搜索只有定义 | Gateway 使用自有 normalize/pack owner；未测得同类重复转换热点 | **当前未接线的性能意图，不宣称已收益。** 有 profile/调用证据后再优化，不引入闲置 helper |

### E. 凭据管理、目录、模型目录和交付工程

| ID / fork 增量 | 当前源码事实 | Gateway 对照 | 决定 |
| --- | --- | --- | --- |
| E01 Codex 删除先归档、批量筛选 401 | `auth_files_crud.go:243-330,501-555`、`internal/misc/archive.go:23-44`：Codex 删除用回收站/归档；批量端点按状态和字符串筛选 | `credential_pool_automation/archive.rs:15-24,69-88` 已有带 schema/provider/credential/reason 的归档；普通 server delete `internal_provider_credentials/mutations.rs:167-206` 仍是删除路径，不等价于可恢复管理 UX | **复用既有归档 owner 补管理 UX 有收益。** 提供显式预览/选择/部分失败回执；不用 Contains("401") 自动销毁凭据，不假装现有归档完全缺失。fork 的移动和记录清理也不是单一 crash-atomic 事务 |
| E02 批量解除 quota/cooldown | `management/quota.go:17-50` 遍历 managed auth，ResetQuota 后返回 ok/partial；不是向供应商充值额度 | Gateway 有健康状态、测试/probe 和 quota refresh，未核实有完全同形的全量 reset 接口 | **有条件采用定向恢复动作。** 按 credential/model/失败原因选择，不默认一键清空所有历史或绕过硬封禁；优先单次恢复许可 B02 |
| E03 递归 JSON 目录扫描和非 auth JSON 跳过 | `watcher/synthesizer/file.go:35-64` 改 WalkDir；store 对空 type 跳过；插件源身份按宿主重新解析 | `provider_credential_folder_sync/filesystem.rs:85-146` 已递归且限制目录深度、entries/files、链接边界；classification 和 normalization 已有 owner | **已有，维持 Gateway 更明确的有界扫描合同。** 不重复实现或放宽失败扫描后 missing reconciliation |
| E04 远端模型目录分 section fallback | `registry/model_updater.go` 在更新前 merge 旧 section，incoming len==0 同样当缺失；service_models 还包含删除官方元数据的净差 | `provider_discovery.rs:98-127` 绑定 origin/credential、限制目录并区分 verified；还有原生 manifest/catalog owners | **采纳“失败更新不毁好快照”原则，不照搬空列表永回退。** 缺字段/抓取失败/合法清空必须区分；不能因 fork 减少能力 metadata 而倒退 Gateway |
| E05 EXE 邻接 runtime dir 与 config 模板 bootstrap | `internal/startup/local.go:16-60` 有 helper 和测试，但全 repo Go 搜索只见定义/测试，未见启动链调用 | `docs/local-data-storage.md:3-15`、`crates/gateway-local-data` 已共享 .ng owner：显式 override→既有 EXE 邻接 .ng→home，不按 cwd、不隐式 portable | **不采用未接线 helper，更不能改变现有持久数据 owner。** Gateway 已有更严格的 portable/升级合同 |
| E06 Windows 打包、周期重启、计划任务升级、GHCR | `scripts/build_windows_package.ps1:35-48,126-151` 构建并拷配置，生成 3 小时重启 supervisor；`deploy_windows_oauth_first.ps1:47-99` 有任务备份/回滚；workflow 改 GHCR beta/stable/tag | Gateway 自有 EXE/desktop/package/runtime gates 与 .ng 外置状态；本轮没做交付链缺口审计 | **不移植固定端口计划任务或定期重启 workaround。** 复用其 ownership/hash/回滚思想，但不把带 live config 的 release 包当标准产物 |
| E07 日志 IP 口径、auth-index 重排、测试/SDK 文档 | `logging/client_ip.go:12-16` 仅复用 Gin ClientIP；auth-index 多为移动/重排，真正 prompt 字段另计 D05；benchmark/测试修改不等于生产能力 | Gateway 自有 management identity、日志与测试；upstream-sync-status.json 仍是旧 v6.9.5 记录 | **不是独立待移植产品特性。** Gin ClientIP 复用不等于可信代理验证，旧同步文档不证明当前状态；新增测试未执行 |
| E08 Codex Free 池面板补丁 | `management_panel_patch.go:19-52,420-453` 使用 codex_pool_entry 或 free+codex_warm_ready；当前生产 Go 中除该补丁外未找到这些字段的生产者 | Gateway 有显式凭据 pool/stock/quota/test 状态，不需要按压缩 React 字符串和中文 DOM 文案打补丁 | **只视作 UI 意图，当前端到端未证明接线。** 不据模型名/套餐猜路由授权；可用池应从原生可信状态投影 |

### P. Basispoints 插件：独立上游与本地修订分开

**P01：**`plugins-src/oai-basispoints/**` 为 untracked vendoring。`FORK.md:3-17` 写明上游 [JaxsonWang/cpa-plugin-oai-basispoints 固定提交](https://github.com/JaxsonWang/cpa-plugin-oai-basispoints/commit/708082da2f851569984de395d25405e61c2bbc34)，本轮在线核实该提交存在；不是把插件全部归为个人原创。

插件主体提供 Excel/Basispoints 请求画像、模型映射、turn/iteration、run_officejs 工具和附件传输。当前 Rust 源码中未找到相同 provider owner，普通 ChatGPT/Codex 接入不能自动等同此能力。本地明确修订是：

1. source-auth marker：宿主重新建立可信 native 源身份、派生身份仍为 virtual；`sdk/cliproxy/auth/plugin_source.go:5-25`、`sdk/auth/filestore.go:238-244,272-289`。不能据此说插件能独立刷新 OAuth。
2. stream=false 却返回 SSE 时聚合最终 JSON：插件 `upstream.go:107-125,194-232` 要求真正 completed；Gateway `stage_send/responses_bridge.rs:80-123` 已有同类聚合基础。插件本来的流也是先缓冲再回放，不是更低 TTFT 的证明。

**决定：暂不纳入通用优化。** 若确有 Excel/Basispoints 使用需求，再按 Gateway provider/credential/entitlement/附件/工具合同单独设计；不引入 Go plugin ABI，不进行请求外预热或未经同意的真实探测。

### H. 回退、撤回和未接线项不能当成果

**H01：**当前 scheduler.go、types.go、auto_refresh_loop.go、conductor_refresh.go 对 B 净差为零；index 的大规模变动主要恢复官方实现。当前 selector 净增主要是 FreeModel 账号级限额，不是千行 scheduler 新优化。12 个历史差异路径已在 CSV 单列。

此外，Responses 当前净差移除了官方的 orphan tool output/重复 ID/工具消息对齐部分保护；codex_executor_stream.go 也删了 NormalizeCodexToolSchemas、EnsureSessionContext 和部分 response.done 规范化。这里只确认净差，未运行反例测试，**不能进一步声称真实请求已经故障**。采纳 D01/D02 时要保留 Gateway 既有防线，而不是把 fork 整文件覆盖过来。

## 4. 建议实施顺序及验收合同

此处是下一阶段建议，不是已批准的新增承诺，也没有因本报告自动提交或上线。

### 第一批：高收益、保持明确边界

1. **C01 模型证据观测**：规范 requested/resolved-upstream/declared-response 三层，alias 导致的正常差异单独标记；缺字段保留 unknown，流与非流均覆盖；只记 bounded 元数据，不保存响应文本，不做“真假模型”断言，不改变授权/扣费。
2. **A03 精准容量错误分类**：provider+typed code/明确短语+status 联合判定；包含普通 400/401/403、content error 的反例；重试不重置全请求预算，工具/创建请求不增加发送。
3. **D01/D02/D03 跨协议保真**：以 normalize/typed representation/pack owners 扩展，而非旁路替换；覆盖 reasoning/tool round trip、custom input、namespace 碰撞、重复/孤立输出、structured output；不削弱上一批流终止证明。这组有一部分来自官方 CPA，而非 fork 原创。

### 第二批：涉及路由策略或持久状态，先确定合同

4. **A04 OAuth-first 策略**：明确严格禁止 API-key 回退还是优先后允许回退；按账号池显式 opt-in，不能跨租户或绕过 pin/allowed accounts。“指定官方 Pro 账号优先”的目标应绑定明确账号与可信 provider，不能仅凭 OAuth 类型推断官方身份或价格；禁用/撤销/quota 改变在每次发送前可见；不引入 A02 无限恢复。
5. **B01/B02 共享错误预算与单次恢复许可**：扩展已有 attempt/health owner；明确 group 身份来源、Local/Server 原子 reservation、并发与崩溃回收、历史保留；真实流交付后不再透明重试。
6. **B03/B04/B05 外部健康源**：先 advisory，显式绑定来源/窗口/模型/账户分组；SSRF、认证 header 脱敏、bounded poll/response/age、错误/空样本/partial failure 可见；硬 gate 必须另有用户明确策略，不能当授权证据。
7. **E01/E02 凭据维护 UX**：复用既有归档和健康 owner，明确 preview/选择/回执与恢复合同；失败凭据不能凭一句包含 401 的文本自动永久删除。

暂不排入：C02 模型指纹鉴真、D06 请求外 ticket ping、A02 无限恢复、WorkBuddy 身份伪装、全 JSON 默认替换、DOM 注入式面板补丁、周期重启 workaround、未有使用需求的 Basispoints provider。

## 5. 本轮实际变更与验证边界

- 只新增本报告和 CSV 清单；没有改 Gateway 运行源码或接管 S06。
- AIGateway 源码、HEAD、分支、既有 staged index、未提交源码保持；没有 fetch/reset/stash/rebase/checkout/commit/push。
- 未读 live config、auths、AIRead 或 secret 实值；只读示例配置、源码、测试、非敏感文档及 Git 对象。
- 没有执行 Go/Rust/前端测试或构建，没有调用真实供应商、没有执行 verify/deploy 脚本，没有触碰 8317 服务、计划任务或部署目录。
- 读取测试只用于理解意图，历史 closeout 的测试数不作为本轮结果；本轮仅文档 UTF-8、diff、清单完整性和现场保全检查。
- 现场证据放在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-fork-audit-20261019/`：fork-source-before.json、fork-status-before.txt、fork-net-paths.txt、fork-committed-only-paths.txt、fork-file-inventory.csv、inventory-summary.json 和 preservation-check.json。
- 当前真实部署是否启用 OAuth-first/protection/Basispoints，dirty 组合是否可编译，均未在本轮验证。

结论：**有值得吸收的增量，但应按上述收益和信任边界逐项重新实现；不能整体搬运 fork，也不能把上游已有、已撤回或未接线代码当作新增能力。**
