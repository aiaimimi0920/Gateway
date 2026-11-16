# 服务商分渠道扩展：第 8 批，Bedrock 原生精确账号测试

> 2026-11-02；W06/P10 无凭据开发切片，本批定向回归已通过，总计划仍未完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。未提交、推送、出包、联网调用 AWS 或修改运行实例。
> **不把一个 probe 子链替代 W06，更不把它替代 R01–R16 总计划。**

## 1. 本批完成的代码边界

R10 仍为 `bedrock-api / bedrock-converse`，不按材料模式额外制造额度池。API/OAuth/Web 的产品与额度拆分规则不变。
先完成 P07 最终来源冻结、fixture 等价及复用测试 EXE 的跨语言回环；历史回执见 [P07](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p07.md)。

- [bedrock_model_probe.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/bedrock_model_probe.rs)：精确 target + mapped model，使用 native `normalize_converse`，`inferenceConfig.maxTokens=32`；通过生产 `execute_stream_with_wire` 发送，不另建绕过签名的请求。
- [console_model_probe.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/console_model_probe.rs)：只为显式生成请求／已有测试策略开放 Bedrock；保留原模型限制、精确账号映射、禁用和 network gate。无选项旧连通测试仍 unsupported，不悄悄发送付费生成或 `/models`。
- [response.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/upstream/response.rs)：仅将既有 buffered 原生 decoder 提供给 crate 内复用；仍验证 header、双 CRC、内容块、stop、post-stop usage 和完整 EOF，不引入 SSE/Accio parser。
- [model_probe_recording.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/model_probe_recording.rs)：复用现有 audit/health owner，记录 `bedrock_converse / converse_stream / stream=true`、精确 provider/credential、mapped model 与同一 validated usage。

探测完整读取真实 source 至 EOF，最多 256 KiB、60 秒；仅正常 stop、非空纯文本、无工具、有效 usage 才成功，回复超过8192字符拒绝，不通过截断假装完整。
HTTP错误只返回分类／状态，不输出原始上游 body；成功回复先按长值优先清理 Key、auth/header 和敏感 AWS extra aliases，再统一脱敏，避免重叠 STS/SK 残留。无新线程、worker、重试或后台 cleanup 任务。

## 2. 实测发现的请求打包缺口

联网核实 [AWS ConverseStream 合同](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html)：model 在 URL label 中选择、stream 由操作路径选择，JSON 中生成上限使用 `inferenceConfig.maxTokens`。
旧 pack 却将 `modelId` 和 `stream` 也放进业务 JSON；这不是账号不足或上游错误，而是本仓 wire 问题。

[packing.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/bedrock_converse/packing.rs) 已移除两项 HTTP labels，并阻止 request extra 重加；[最终 extra merge](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/request_plan_part_1.rs) 同样过滤两项，防止 provider extra 补回。
调用 API 参数签名保持兼容；不改变旧认证模式，不借测试放宽显式 AWS endpoint/region allowlist。
原生 probe 的显式 `inferenceConfig` 不被 provider 中 `maxTokens=9999` 覆盖，有真实请求捕获断言。
**普通跨协议参数桥接另有未完成项**：OpenAI normalizer 将 `max_tokens/temperature/top_p` 等保留在 extra，现 pack 不会自动转换成 Bedrock inferenceConfig；本批 native probe 不证明该普通请求桥接已完成。

## 3. 实测与证据

证据目录：[P08](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p08)。全部为合成材料、随机 loopback，无真实凭据读取，无上游费用。
执行前设置 `TEMP/TMP/TMPDIR` 到本批 `runtime-temp`；Cargo 串行协调、`--locked --jobs 4`，没有另开排队 Cargo。

- `probe-red.log`：Converse 6项中4过2失败，真实复现 HTTP labels 错误。
- `probe-red-console.log`：原生 probe 的正常调用及错误 wire 两项均被 unsupported 截断；admission 通过。矩阵另因 fixture 没有 LocalRuntime、结果保存尝试不可用 Redis 返回500，不把该500作为生产 probe 的红测证据。
- `probe-green-v1.log`：Converse 6/6；probe 仍有 fixture 错误。旧 preset 的自定义 Authorization 本来保留 raw token，fixture 误认为 Bearer；已修 fixture，不改变生产旧认证。矩阵补隔离 LocalRuntime。
- `probe-green-v2.log`：测试代码把 provider 的 String base_url 误写成 Option，E0308；修测试类型，未改生产 schema。
- `probe-green-v3.log`：Converse **6/6**、新 Bedrock probe **7/7** 通过。包括19种负向 credential 场景的单测试矩阵；逐字 encoded ARN path、精确材料、无模型目录、32-token原生上限、同一 usage audit、IAM重叠秘密、取消释放真实连接和 metadata 后传输错误。
- `bedrock-lib-regression-v1.log`：30/30，包括材料/目标、实际 buffered/streaming 最终签名和 redirect；未执行的4122项为过滤器外测试，不冒称全仓绿灯。
- `affected-regression-v1.log`：9个集成目标 **58/58**；mode6、旧EventStream13、真实pipeline3、原生入站20、Anthropic1、普通credential6、NVIDIA5、测试预算3、测试策略1。加本批13项及lib30项，实际执行合计 **101/101**。
- `bedrock-disabled-v1.log`：`cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` 通过。沿用保留Gemini的已知组合，不声称全部feature关闭或修改Gemini lane。
- checker **31/31**；`ratchet-final.log` 扫描3938文件、14个immutable runtime artifacts，governed source >700=0；使用无报告写入入口，没有覆盖原脏报告。`fmt-final.log` 通过。
- 13个本批源码/测试已冻结 `source-frozen-v1.json`，最终UTF-8无BOM/hash复核、文档链接与whitespace检查见本批final日志；没有借调早批hash冒充当前来源。

测试入口：[console_bedrock_model_probe_contract.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/console_bedrock_model_probe_contract.rs)、[fixture](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/console_bedrock_model_probe_contract/fixture.rs)、[lifetime](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/console_bedrock_model_probe_contract/lifetime.rs)、[Converse contract](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_converse_contract.rs)。
合成 wire 使用既有 legacy 自定义目标及 raw Authorization／SigV4；**不证明显式 Bearer/IAM/temporary 可向本地回环发送**，不放宽新模式的官方受众。显式模式材料/签名的历史独立证据仍见 P03/P07，不冒称本批已在 AWS 实测。

有效行：`provider_runtime`76→77、Console346→351、recording148→161、Bedrock public100→101、packing167→166、upstream模块24→25、response149→152、最终planner145→146、Converse测试177→191；新probe190、主测试225、fixture198、lifetime110。无新例外／baseline修改，没有本批文件超过500有效行。
审查：未改变Secret Grant、配置revision/CAS或凭据选择；没有新增存储字段、外部依赖、worker或后台进程。输入与回复有界，所有异步发送受deadline，错误不携原文，上游不能决定probe的执行目标；完整EOF后才提交成功与health更新。
本批无UI改动，不重复运行P07浏览器或假构建；未打devtest/正式包，未执行真实账号、权限、额度或长期生命周期验收。

## 4. 仍未闭合的取消记录边界

取消 probe future 会 drop 实际 HTTP source，本批有 socket关闭测试；health begin 只保存快照、无租约，无假成功或新增资源泄漏。
但共享 recording owner 在 begin 后若整个 future 被取消，可能留下未终结 audit。**连接释放不等于 durable audit 已完成，也不能因此标完整取消合同通过。**
这属于 P50 前必补的跨 probe 记录切片：取消保持健康中性、限定异步清理预算、只对仍进行中的 audit 做 CAS，避免覆盖已完成结果，并分别保护 SQLite/服务器存储和 shutdown。
本批没有用新后台任务或无条件 finalize 掩盖竞态；也不宣称重启／崩溃下的 audit 收尾已实现。

## 5. 下一批与全计划剩余

1. 长模型 ARN 必须贯通模型编辑／draft重开／typed配置编译／测试策略／单次请求parse／执行前gate；不能只改输入框。
   `exact_model()` 还被 case/plan/scope ID 共用，不能整体放宽到2048。使用实际 Bedrock产品上下文和其model语法，保留普通产品及其他ID的256字节边界；无上下文parse需有界结构校验＋目标后校验。
2. 普通协议生成参数桥接、reasoning/signature/redactedContent、modeled exception frame、取消 audit 收尾、集成 devtest 等仍缺；W06不标整体 offline_ready。
3. 继续全部 R01–R16：Kiro/Accio/Grok Build/Vertex OAuth、普通 Qwen API、其余 Web/媒体、P50、集中录制与逐渠道真实验收。总目标保持 active，不提前要求用户逐个录制。

P08 有意修改早批原生响应/请求 owner；P03–P07 冻结证据保留历史，不覆盖旧 JSON，也不用旧 hash 冒充当前来源全部匹配。

### 5.1 长 ARN 接力 owner 清单（只读核对，尚未改代码）

| 层 | 必须一致处理的 owner |
| --- | --- |
| 模型选择、提交与读回 | [credentialModelSelection.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/credentialModelSelection.ts)、[providerCredentialDraft.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/providerCredentialDraft.ts)、[useCredentialDialogEditor.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useCredentialDialogEditor.ts)、CredentialDialog 的错误文案 |
| 模型测试表单及映射 | [credentialTestPolicyDocument.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/credentialTestPolicyDocument.ts)、[CredentialTestPlanFields.tsx](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/CredentialTestPlanFields.tsx)、[ProviderModelMappingDialog.tsx](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/ProviderModelMappingDialog.tsx) |
| typed 模型列表及两条编译路径 | [model_selection.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/model_selection.rs)、[provider_compilation.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/provider_compilation.rs) |
| 模型／非模型 ID 分离 | [credential_test_policy.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/credential_test_policy.rs)、[credential_test_plan.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/credential_test_plan.rs)；case、plan、scope ID 不随模型放宽 |
| 保存校验、手动parse、自动／手动执行 | [test_policies.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/document/validation/test_policies.rs)、[test_policy.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/test_policy.rs)、[console_probe_request.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/console_probe_request.rs)、[test_execution.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/test_execution.rs) |

同长度普通模型拒绝、Bedrock合法长ARN通过、2049字节拒绝、错误ARN语法／目标拒绝、保存重开、all/selected、单次/保存/自动计划、case/plan/scope ID仍受256限制，均应有真实定向测试。
验证 actual encoded path 解码后逐字等于模型；不能只检查“发出了一次 HTTP”。fetched 目录无当前 Bedrock来源，勿借手填ARN扩张成未实现的官方目录能力。
