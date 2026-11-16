# 服务商分渠道扩展：第 13 批，普通 Qwen 模板、目标与 Key 编辑

> 2026-11-02；R09 / W05 / P01 / P10 的无凭据切片，定向检查通过；第 3.1 节保留实际构建副作用及授权遗漏，W05 与总目标未完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包、修改运行实例、真实凭据或 `.ng`。

## 1. 渠道与范围

普通 API 使用 `templateId/providerId=qwen-api`（实例 ID 可改）、`preset=qwen-official-api`、OpenAI Chat wire 和同地域/workspace Bearer Key。
内部 `qwen_workspace_api` marker 只保留产品身份，不是 OAuth 或第四种认证；普通 API、Coding Plan、Qwen Web 不混材料/额度。
同平台有 Key/OAuth 也继续评估 Web；未知额度关系仍隔离，本批不推断 Web/API 共享权益。

沿 [P10](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p10.md) 的六区 descriptor、[P11](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p11.md) 的 fail-closed sender 和 [P12](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p12.md) 的 keep 受众保护实施；没有重新发明 transport、认证或配额框架。
公开地域/产品事实继续引用 [API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-api-contracts.md) 的联网来源；本批没有新的第三方代码复制、依赖变更或真实联网调用。

## 2. 实现与边界

### 2.1 Preset 与最终配置

- `src/preset/qwen.rs` 注册完整普通 preset，沿 `line-qwen-official-api` gate；profile 为 `qwen_official_api`，无默认模型、地域、Responses/session。退役 `qwen` / `qwen-dashscope-openai` 不复活。
- `src/routing/config/qwen_workspace.rs` 按**原始 preset**进入检查，不能先用合并后的 payload ownership 决定是否校验。目标/auth 同时改成 Plan 或外站也不能失去身份而被放行。
- `uses_provider_default_account()` 区分真正默认账号和非执行 parent。仅普通 preset 的非执行 parent 可暂留空 base；配置声明仍检查，最终每个 credential（含 disabled）都必须有合法有效目标并正常 prepare/runtime validate。
- parent configure 只延迟最后的 prepare，不跳过字段和 Basispoints 配置；真实默认账号不豁免。Console 文档的空 URL 特例限定同一条件，不推广给其他产品。
- 拒绝其他产品 profile/family/adapter/auth、非 Chat paths、session/keepalive、危险认证/Host headers、浏览器模式、Basispoints；raw credential 的 discovery/job 与四类 OAuth refresh 字段在建立后台状态前拒绝。
- 软件 API fingerprint 与产品身份分开：Codex/Claude/CPA 等现有 API 身份不被一概强制 Auto；Session/Antigravity 仍不适用。实际 wire 保持普通 Chat，不恢复 native Responses bypass。
- 空/clear Key 可作为正常无凭据状态保存；不伪造 Key，不放松 P11 `validate_key`，发送前仍拒绝空/非法材料。native `model_catalog` 必须绑定最终账号目标，不能换区后复用。

### 2.2 正常创建与账号目标

- 新建 `qwen-api` 空池：显式六区/workspace、派生只读 endpoint，无默认地域、模型、账号、探测或路由。纯创建 owner 再校验目标，不只靠组件按钮。
- `QwenEndpointFields` 与 `QwenCredentialEndpoint` 使用相同 public grammar；账号继承值不展平为 override。空的 partial override 仍是未完成输入，不能误当继承。
- 有效目标相同的继承/override 表示变化允许 keep；有效目标变化则必须明确 replace/clear。旧目标非法/未知时 keep 仍拒绝，但有效新目标加显式 replace/clear 可以修复。
- 目标或 Key 变化清旧 fetched、目录和通用协议证据，保留 manual，不把旧 fetched 洗成 manual；duplicate 仅复制公开目标和手动选择，不复制 Key/目录归属。打开新增账号也不把其他账号的池级 fetched 并集预填成 manual。
- 当前 Qwen native 目录管理/UI 尚未实现，故禁 generic discovery 与借用其他产品的目录入口；不以自动 generic job 冒充 Qwen native 目录。

### 2.3 普通 Key 暂存与最终提交

`qwenMaterialBinding.ts` 只绑定公开 `product + effective base + effective auth`，identity 沿现有稳定 provider/credential ID；不包含 Key，不依赖数组 slot 或 ID 后缀。
replace 从本次最终账号文档计算 binding；后续 metadata-only keep 保留同一暂存值与 binding，**不能重算 binding 给旧 Key 授权新目标**。clear 覆盖原 replace，不残留旧值。
`credentialDocument` 在输出 SecretPatch 前验证；有 binding 的 edit 即使当前已改 preset 也不能逃逸，当前 Qwen api_key replace 缺 binding 同样拒绝。
预览只省略不安全 staged edit，严格提交报错；现有 `useConsoleRouteDraft.buildCommitRequest` 对最终 document 再检查，即使调用方显式提供 secretPatchOverride 也不能绕过已有 staged edit 的验证。
这些是正常 CredentialDialog 暂存链的保护，不宣称将所有手工 raw SecretPatch 输入升级为新授权系统；明确手工 replace 仍沿原 Secret Grant/CAS。

本批没有新增后台循环、锁、进程或存储版本。目标 parser 有界，workspace 最多 63 字符；模型/secret 继续沿现有 schema 和预算。
React 定向审阅使用派生目标、受控输入和显式事件，不用 effect 同步派生 endpoint；六区固定小列表，不引入网络 waterfall、全局可变状态或额外组件依赖。

## 3. 实际验证

证据目录：[P13](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p13)。所有命令将 TEMP/TMP/TMPDIR 放入本批 `runtime-temp`；仅合成材料，无真实账号/费用。

- `config-red-v1.log` 是测试误调用 API 的编译失败，不当行为红测。修正后未注册 preset 的 `config-red-v2.log` 实际 1 pass / 5 fail；不据此声称每项具体保护已复现。
- 先只注册 preset/profile 后，`config-red-v3-preset-only.log` **3 pass / 3 fail**：确实复现空 parent 提前失败和同时改 Plan base/auth 后产品身份丢失的接受。随后实现配置 guard。
- `config-green-v1.log` 五组 **32/32**；这是扩充最终集前的结果，不重复累加到最终独立数量。
- `ui-red.log` 两项实际失败：缺 `qwen-api` 模板及 `Qwen 目标来源` 控件。`ui-green-v1.log` 两项通过。扩充后 `ui-expanded-v1.log` 为 **29 pass / 1 fail**，暴露 duplicate 将旧 fetched 转为 manual；修复后 `ui-expanded-v2.log` 为 45/45。
- 只读独立复审发现 previous 非法目标阻止显式 replace/clear 修复；追加 `repair-red-v1.log` **25 pass / 2 fail** 后修正，after 仍严格验证，未知 previous 视为受众变化。
- `ui-regression-v1.log`：15 文件 / 139 pass，包含当时 5 个 Qwen 文件共 47 项。命令中的 `providerCredentialDraft.test.ts` 不存在，没有独立执行；复制行为由新的 `qwenEndpoint.test.ts` 覆盖，不用未匹配 filter 冒充通过。
- 再审查新增账号路径发现旧 generic fallback 会将池级模型并集预填为新 Qwen 账号的 manual；`new-account-red-v1.log` 实际 **1 fail / 5 filtered skips**，不是 6 项都执行。限定普通 Qwen 不走该预填后，`ui-regression-v2.log` 最终 **15 文件 / 140 pass**，含 Qwen **48 项**；不改变其他产品的原预填。
- `ui-shared-final.log`：**5 文件 / 21 pass / 1 skipped**；skip 是需显式私有 worker sample 的旧 opt-in 测试，未读取该材料。最终前端合计 **161 pass / 1 skipped**，不把早期重复执行再次累加，也不是全前端回归。
- `typecheck-final-v2.log` exit 0。`web-build-v2.log` 使用原 RSBuild production web entry/defines/transforms，在 `linshi` 输出独立 bundle，14 文件；隔离命令本身关闭 live publish/prune 和 clean，`web-build-receipt-v2.json` 记录 hashes。这是前端编译，不是 EXE/打包/浏览器人工验收；但**整个 Cargo 验证过程确实触发过默认发布 hook**，不能用这个隔离命令的边界掩盖第 3.1 节的副作用。
- `default-final-v1.log`：**124 pass / 0 fail / 1 ignored / 1 filtered**。其中 Qwen config 9、runtime 6、catalog binding 4、secret/consumer/management 20，Bedrock 四组 21，共享 Console document 64；ignored 是合成 frontend fixture opt-in，filtered 明确 skip 前批已知未修 `legacy_compiler_rejects_extreme_token_expiry_without_panicking`，不算通过。编译/链接 8m16s，不作为性能承诺。
- `call-flow-regression-v1.log`：共享 lib `call_flow` **40/40**；`qwen-wire-regression-v1.log`：精确 Qwen native shortcut/compact/实际 sender Bearer+Chat+拒绝重定向 **3/3**。它们分别有 4127/4164 项按名称正常 filtered，不是全 lib 回归；默认组合最终定向共 **167 pass**，早期重复执行不再次累加。
- `feature-disabled-final.log`：`--no-default-features --features line-gemini-web-reverse` 下 **38/38**，含新 preset gate/无 generic fallback 2、runtime/API-off Coding Plan 拒绝 5、Qwen pure secret/consumer 18、Bedrock pure secret/keepalive 13；编译/链接 7m39s。Qwen/Bedrock API、common families 和 management UI 关闭，Gemini Web 是沿用的最小编译底座；不宣称全 feature 关闭或全部组合矩阵。默认 167 加关闭组合重复执行 38，共 205 次 Rust 定向通过执行，不是 205 个独立用例。
- 原始 preset 锚定、parent 延迟范围、最终 disabled credential、raw后台字段和 Console 空 URL 特例经只读独立复审，未发现新的绕过；这是源码审阅，不代替运行结果。
- `checker-v1.log` **31/31**；`ratchet-final.log` exit 0，3973 文件、14 immutable、governed source >700=0。使用同一 checker 的无报告入口，不覆盖原有脏的有效行报告。
- `source-before.json` 与两个不覆盖的 addendum 记录 28 个源码 scope 的原状态；`source-frozen-v1.json` 是修新增账号预填前的中间状态，最终 `source-frozen-v2.json` 不覆盖旧文件。P11/P12 各自 frozen source 重查通过，不用 hash 匹配替代其全部旧测试重跑。
- `format-final.log` 的 `cargo fmt --all -- --check`、`diff-final.log` 的全工作树 `git diff --check`、`source-verify-final.log` 的最终 v2 来源复核均通过；Rustfmt check 不执行发布 build script。没有配置新的 JS formatter 或为此改写其他前端文件。
- 最终 5 份文档及 28 个源码的 UTF-8 无 BOM、围栏、本地引用和 tracked/untracked whitespace 使用 `check-documents.mjs --v2` 检查；本批 Cargo session 已实际结束，不把观察超时当作完成或另排重复构建。

### 3.1 必须保留的构建副作用更正

本批后续检查真实 `build.rs:122-175` 才发现：默认 features 包含 `management-ui`，而默认 Cargo 测试会执行 `npm run build:web`，显式 `GATEWAY_WEB_PRUNE_LIVE=1`。
`rsbuild.config.ts` 的发布 hook 会改写工作区 `apps/desktop/dist/web`、ready marker 并按现有 owner 清理旧产物。`target/debug/build/gateway-be91ca49e2982853/output` 已实际记录本次发布，ready marker 最后写入为 2026-11-02 18:32:06（Asia/Shanghai）。
没有预先目录快照，**不能确认具体清理了哪些旧文件，也不能声称整个验证未触碰工作区静态目录**。这项间接副作用未在执行前按用户要求解释并取得“同意”，是本批遗漏；已明确告知用户，不用 green test 掩盖它，也不擅自清理/恢复目录来消除痕迹。
未启动/重启 Gateway，未读写真实 `.ng`、凭据、数据库或发布 EXE；静态构建目录变化不等于运行实例已更新，但本批也不外推所有潜在静态资源消费者均未受影响。
发现后不再新增会触发该 hook 的命令；后续后端定向测试优先 `--no-default-features --features runtime-defaults`，关闭 management UI 构建副作用。确需重新发布/清理该目录时必须先解释并等待用户明确“同意”；不能仅依赖一个可能过期的 prebuilt 开关声称安全。只读保存的 [构建 hook 观察记录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p13/build-hook-observation.json) 与相邻 stdout/marker 副本不伪造缺失的 before inventory。
正在串行收尾的 feature-disabled 组合本身不含 `management-ui`，不执行此 hook。本记录不作为后续删除/发布的授权。

## 4. 有效行与来源

按官方 lexer 测量；本批所有源码均 ≤500，无 baseline/例外/immutable policy 或 lockfile 变更。完整逐文件 hash/计数在 frozen evidence；主要接线前后如下：

| 文件 | 前 → 后 |
| --- | ---: |
| `src/preset/qwen.rs` / `preset.rs` / `preset/registry.rs` | 121→146 / 231→232 / 179→180 |
| `src/protocol/registry/profiles.rs` / `routing/config.rs` | 231→232 / 208→209 |
| `src/routing/config/call_flow.rs` / `provider_compilation.rs` | 44→45 / 171→176 |
| `src/console/document/validation.rs` | 404→409 |
| `providerCatalog.ts` / `ProviderCatalogDialog.tsx` / `ProviderCatalogForm.tsx` | 436→445 / 104→106 / 60→66 |
| `CredentialDialog.tsx` / `ConsoleGlobalDialogs.tsx` | 441→456 / 291→293 |
| `useCredentialDialogEditor.ts` / `credentialDocument.ts` / `providerCredentialDraft.ts` | 335→346 / 476→482 / 146→150 |
| 新 `qwen_workspace.rs` / config 合同 | 131 / 213 |
| 新 Endpoint / EndpointFields / CredentialEndpoint / CredentialDraft / MaterialBinding | 33 / 20 / 25 / 37 / 20 |
| 新 Endpoint / MaterialBinding / editor / CredentialDialog / CatalogDialog 测试 | 86 / 77 / 99 / 83 / 39 |

表中的前端文件均在 `apps/desktop/src/features/console/`；没有 >500 新文件或必须拆分的增长。

## 5. 下一个最小切片与仍未完成的内容

W05 仍 `in_progress`，不标 `offline_ready`，不登记真实验收/蓝色。下一批应继续：

1. 已有 provider 父级目标的正常编辑入口，保留精确子账号 override 与 SecretPatch 授权，不把高级 JSON 编辑称为正常功能完成。
2. Qwen native 动态目录 management：精确 credential 或明确新材料、Secret Grant、slot、revision/CAS、六区 descriptor；前端同步 `sourceUrl + accountBaseUrl` schema、request fence 和 stale result 拒绝。新目录不能被本批清旧证据的规则误删。
3. native 目录 source/文本模型策略→手动选择→路由投影，以及精确账号 probe；目录不是 entitlement 或生成成功。
4. 当前普通 preset 的生产 pipeline buffered/stream/响应/取消闭环及管理/UI 合成回环；本批两侧单测不能代替这条完整接线。

明确的接力风险：当前 `officialModelCatalog.ts` 仍只接受旧 fixed-source enum，`providerCredentialDraft.ts` 因此不能完整读取通过高级 JSON 写入的 Qwen native `model_catalog`。没有明确 `model_selection` 的这种人工草稿，目录模型存在被旧 reader 视作 manual、元数据保存时丢目录的缺口；本批没有为它伪造通用 schema 兼容结论。下一批必须先补 native schema/ownership 读取与保存保护，再开放目录入口或声称普通 Qwen 可完整编辑已有 native 目录；这也是 W05 不能标 offline_ready 的原因。

Bedrock 普通参数/reasoning/signature/取消 audit，其他 R01–R16 的 OAuth/Web/API/媒体，P50 devtest 与集中录制/真实逐渠道验收继续保留。
没有使用历史凭据，没有完成上游登录/刷新，没有交付新 EXE，也没有全仓回归结论。总目标保持 active。
