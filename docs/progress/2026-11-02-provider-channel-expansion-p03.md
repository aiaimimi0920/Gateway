# 分渠道扩展：Bedrock 认证底座与 API 合同（第 3 批）

> 2026-11-02；main，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交或推送。
> [总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md) 仍在执行；本批不是 W06 整体完成，也不是 P50 或真实验收。

## 1. 实际实现

承接 [第 2 批材料链](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p02.md)，本批进入 W06/P10 后端认证前置修补。
新增 `bedrock_auth / bedrock_target`，复用既有 AWS SigV4，不新起 transport、存储或第二套网关。

- AWS AK/SK/STS/region 不再从 payload.extra_body 合并到业务 JSON；读取和排除统一覆盖历史大小写、下划线、连字符 aliases。
- buffered 与 streaming 统一在最终 profile/header 处理后认证；移除旧 buffered 独立签名点，防漏签、重复签或先签后改。
- 新 `bedrock_bearer` 接原始 Key、后端加 Bearer；`aws_sigv4 / aws_sigv4_temporary` 区分材料，临时模式强制 STS；缺项、混合、矛盾 aliases 拒绝。
- 新模式绑定公共 runtime endpoint/region、POST/canonical model path，禁止 redirect；typed preparation 在快捷入口前拒绝材料改投其他 adapter，最终 sender 再校验。
- modelId/ARN 按官方 SDK non-greedy label 编码；拒绝 traversal/query/fragment/预编码输入，显式模式不能 override 到另一模型 path。
- 旧 preset/ID/默认序列化、legacy 自定义认证和完整 IAM region 推断保留；半套/非法/矛盾 IAM 不再静默降级，这是有意安全收紧。

具体 owner、材料、受众、兼容和来源见 [Bedrock 认证合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-bedrock-auth.md)。
没有开放 `bedrock-api` Console 模板或多字段 UI/SecretPatch；不把 helper 支持当正常添加账号已完成。

## 2. 调查补齐，不冒称实现

[API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-api-contracts.md) 记录 Qwen 普通按量 API 六地域 workspace、Plan 隔离、native 分页目录与现有接缝。
官方 2026-10-27 List models 使用 `GET /api/v1/models` 和 `output.models[].model`，不是通用 `data[].id`；US shared 文档矛盾和能力/目录 source 未决边界保留。
四次公开无凭据 GET 均 401，只证明鉴权边界，没有成功 Key 目录或推理。Qwen 普通 API preset/UI/parser/管理路由尚未实现，不混 Coding Plan，Web 继续独立保留。
AWS 公开认证、FIPS/runtime、modelId/ARN 合同已补齐；官方 SDK 固定 commit 仅行为参考，无 SDK 文件复制、依赖安装或升级。
现 Bedrock accumulate 委托 Accio，完整 JSON 有合成覆盖，但 upstream EventStream 跳过 headers、未验 CRC。wrapper 和反向 OpenAI→Bedrock 测试不证明真实 AWS upstream，必须继续补输入 owner。

## 3. 测试与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p03`。

| 检查 | 实际结果 |
| --- | --- |
| 初次红测 | 20 tests：16 passed / 4 failed；raw Authorization、缺项降级、恶意 final target、IAM 进 JSON；bedrock-red.log |
| 首次修补 | 25 passed / 0 failed；bedrock-green.log，不代表后续 alias/redirect/ARN/prepare 最终版本 |
| 最终 Bedrock 定向 | **29 passed / 0 failed/ignored**；bedrock-final-v2.log，源码冻结后重跑，包含 alias/redirect/ARN/prepare 修补 |
| 相邻 call_flow/header/profile | 分别 **40 / 39 / 5 passed**，均 0 failed/ignored；call-flow-regression.log / headers-regression.log / profile-regression.log |
| 两个 Bedrock public contract targets | Converse **5 passed**，EventStream **13 passed**，均 0 failed/ignored；bedrock-public-contracts.log。既有方向不代表 AWS upstream |
| Bedrock feature-disabled 编译 | `cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` exit 0；bedrock-feature-disabled.log。不重复已知全关 Gemini 缺失符号失败，也不宣称所有 feature 组合均通过 |
| 格式/checker/ratchet | `cargo fmt --all -- --check` exit 0；checker **31 passed / 0 failed/skipped**；ratchet exit 0，3894 files / 14 immutable / governed source >700=0；final-check-exits.json / checker-tests-final.log / ratchet-final.log |
| UTF-8/scoped diff/有效行 | 15 源码 + 5 文档严格 UTF-8 无 BOM；68 个本地引用目标存在，围栏平衡；scoped whitespace 和 tracked diff 检查通过；document-validation.json |

两条 loopback 执行路径用三套 aliases 和普通 ID/ARN 捕获最终请求，检查 JSON 秘密排除并独立重算 actual headers/body 的 HMAC。
307 负测保留测试 client 默认 redirect，再检查第二随机 listener；测试不为方便放宽生产 AWS allowlist。
这些仅是合成材料/随机端口证据，不外推实际 AWS 权限、所有 region/profile/transport 或 W06 全链。
前批 Console 整套的已知失败 `legacy_compiler_rejects_extreme_token_expiry_without_panicking` 未修；本批不改 refresh lifetime，不称该整套全绿。
最终检查期间仍有编译 warning；没有运行 `cargo fix`、升级依赖或为消除无关 warning 扩大修改范围。

## 4. 有效行与审查

| 修改 owner | 本批前 → 后有效行 |
| --- | --- |
| bedrock_runtime_helpers.rs | 283 → 260 |
| client.rs | 442 → 444 |
| client/request_plan_part_1.rs | 139 → 145 |
| client/request_plan_part_2.rs | 433 → 429 |
| client/streaming.rs | 341 → 348 |
| bedrock_converse_official_api_common.rs | 147 → 155 |
| protocol/bedrock_converse.rs | 81 → 93 |
| protocol/call_flow/preparation.rs | 118 → 133 |
| upstream/mod.rs | 184 → 185 |

新增六个源码/测试模块分别为 150、122、126、180、170、59 有效行，最大 180；精确路径和值见 effective-after.json。
最大修改源码 444，有清晰职责；没有新增 501–700 例外或修改 baseline/策略来绕门禁。
只读独立审查指出 legacy 推断被新 allowlist 连带收窄、redirect 负测客户端自身禁跳转等问题，均已修补；主代理同时补齐历史 aliases 归一化、显式 adapter/path 和 preparation guard。
审查结论不是执行绿灯，最终仍以源码冻结后的测试日志为准；source-hashes-frozen.json 与回执记录对应范围。

最终定向及相邻回归均使用以下相同编译配置，分别以 `bedrock_`、`call_flow`、`upstream::headers::tests`、`upstream::request_profile` 为过滤器；数量包含彼此可能重叠的测试，不相加宣传为去重覆盖总数：

```powershell
rtk proxy cargo test --locked --lib --no-default-features --features runtime-defaults --jobs 4 bedrock_
rtk proxy cargo test --locked --no-default-features --features runtime-defaults --jobs 4 --test bedrock_converse_contract --test bedrock_eventstream_contract
```

checker 使用官方 npm 测试入口；ratchet 使用同一官方 checker 的 `--mode ratchet` 无 JSON 报告写入入口，避免覆盖本轮开始前已脏的 `scripts/artifacts/effective-code-lines.json`。没有修改 checker、策略、baseline 或例外文件。
源码 freeze hash 在最终测试后再次比对一致；格式、checker、ratchet、编码和 diff 的实际 exit code 保存于 final-check-exits.json。

## 5. 写入边界与下一步

本批只改 Bedrock、共享最终 sender 的最小接线、typed preparation 的 Bedrock guard 与文档。不接管 S06/Gemini，不改在线实例、存量账号、密钥、数据库或旧包；前两批与原有脏改保留。
未搜索/读取/试用历史凭据，无实际授权/刷新/推理/生成、付费调用、EXE/包、重启、提交或推送。
下一步补 Bedrock 正常材料/secret 回环、配置编译/候选及真实 AWS EventStream 输入 owner；Qwen 普通 API 按 native/地域合同实施。
其他 OAuth、Vertex Express、Luma Agents、Perplexity Web、Web 生命周期/完整媒体线、集中录制和逐渠道真实验收继续追踪。总目标仍 active。
