# 服务商分渠道扩展：第 9 批，Bedrock 长模型身份

> 2026-11-02；W06/P10 的长模型身份切片，定向离线验证通过；不是 W06 或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或变更运行实例。

## 1. 不变的范围和产品规则

API Key、OAuth、Web 按实际产品、认证与额度归属决定拆分。Web 不是仅在前两者不存在时才采用；额度关系未知时保持隔离。
本批仍为 R10 的 `bedrock-api / bedrock-converse`，不因长 ARN 或 AWS 材料模式额外制造服务商或配额桶；R01–R16 均继续保留。
总入口见 [扩展计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。

## 2. 实施 owner 与边界

- [模型身份校验](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_model_id.rs)：结构读取最大 2048 UTF-8 字节；配置／显式账号测试许可依赖实际 payload，普通产品的这些模型字段仍为 256。长 Bedrock 身份复用既有 native transport 语法，不按 Provider ID 后缀、字符串前缀或秘密草稿判定产品。
- [测试策略](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/credential_test_policy.rs)：默认 `validate()` 保持旧 256 语义；新增仅供受控 owner 使用的结构／最终 payload 校验。case、plan、account/subpool scope ID 不随模型放宽。
- [编译最终校验](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/model_identity_validation.rs)：在 discovery 与 call flow 合并后验证精确账号、pool/default/subpool/named 策略及长模型声明／映射；覆盖启动和正常 Console commit，不新增存储版本或迁移。
- [配置编译](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/provider_compilation.rs)：两处 selection 使用一致的已校验模型结果；聚合不重新调用旧 256 字节 reader。映射反序列化只负责有界结构，最终产品门在编译器。
- [单次请求解析](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/console_probe_request.rs)、[映射前后 probe gate](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/console_model_probe.rs)、[矩阵执行](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/test_execution.rs)：parse 不拥有产品上下文；目标解析后与发送前仍校验。All 不再静默丢弃合法长身份；Selected 也必须通过实际账号产品校验。
- 自动 inherited 与 named 调度按每个 target 的最终 payload 检查，不只看 provider preset；disabled/network/revision/128 次与 300 秒预算不变。
- [Circuit](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/model_circuit.rs) 和 [capacity](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/model_circuit/capacity.rs)：仅模型键的存储上限由 1024 对齐到 2048，provider/group 仍 1024；容量条目总数和既有退避／取消 owner 不变。这是有界存储能力，不是非 Bedrock 产品许可。
- [前端模型上下文](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/providerModelId.ts) 贯通 selection 保存与重开、CredentialDialog/正常 editor、映射输入与写入、测试候选、测试范围、命名计划与列表运行。实际 adapter/discovery 与 `call_flow/callFlow` 覆盖优先于 preset；混合范围采用各账号均可接受的限制。所有长度使用 UTF-8 字节，不以 JS 字符数代替。

本批不新增 Bedrock 官方模型目录、不把 fetched/manual 标记升级成调用证明，不修改 ARN 的大小写、字符或精确身份；有效原串只做 URI label 编码一次。
既有短声明的宽容性不借本批全面重写；新增或扩展的长身份必须通过产品门。存量 credential、secret keep/replace/clear、路由 disabled 和字段缺省保持原 owner。

## 3. 验证记录

证据目录：[P09](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p09)。
所有测试使用合成材料、随机 loopback；`TEMP/TMP/TMPDIR` 指向本批 `runtime-temp`。没有读取历史凭据、访问 AWS、付费调用或修改真实 `.ng` / 数据库。
Cargo 使用 `--locked --jobs 4`，由主代理串行协调；其他项目的 native/Cargo 工作不停止、不冒领。

- `red-ui.log` 是执行路径错误（根目录没有对应 Vitest module），不算产品红测；修到实际桌面 npm 入口后，`red-ui-v2.log` 真实复现保存／命名计划 2 项失败、1 项通过。
- `red-rust.log`、`red-rust-v2.log` 是测试误用 crate-private snapshot API 的编译错误，不算生产缺口；修正夹具后，`red-rust-v3.log` 为实际 5 fail / 1 pass：selection、映射、策略与两条真实调用在旧边界失败。
- `green-rust-v1.log` 生产代码与夹具编译通过，5 个配置测试通过，但 Console commit + generation 测试 future 在 Windows 默认测试线程发生 stack overflow；不记作整批通过。
- `green-rust-v2.log` 改用显式 16 MiB 线程后 7/8 通过，真实 pipeline 长模型已通过；剩余失败来自夹具把“执行保存策略”的空请求体误写成无 prompt JSON，HTTP 400 正确拒绝。保留 parse 规则，修测试入口，不为夹具放宽生产输入。
- `green-rust-v3.log`：7 个集成目标合计 **30/30**；新合同8、既有原生probe7、named计划3、NVIDIA5、预算3、策略1、模型映射3。
- 补强后的 `rust-final.log` 为 **7/8**：混合池断言误读了普通路由候选此时固定为 `None` 的 `provider_credential_id`，真实选号保存在 `payload.credential_id`。只修测试断言，不改生产选号或扩大回退；失败日志保留。最终 `rust-final-v2.log` 七目标 **30/30**，反复选择长模型均命中 native `one`；7 次精确 probe 的 SQLite 审计全部 `completed`，完整 2048 字节模型及 provider 逐字读回，健康状态仍绑定同一模型／credential ref。
- `lib-circuit-v1.log`：**34/34**，包含 2048 模型 storage 接纳、2049 拒绝，以及 provider/group 1024 边界仍拒绝；`lib-policy-v1.log`：**1/1**；`lib-schedules-v1.log`：**2/2**。这些过滤器外的4119/4152/4151项不是本批全量测试通过证据。
- `feature-check-v1.log`：`cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` 通过；保留既有 Gemini 组合，不代表全部 feature 关闭。
- `ui-regression-v2.log`：8 文件、47/47；`ui-test-owners-v1.log`：另8文件、28/28。最终收敛的 `ui-final.log` 为 **16文件、75/75**，`typecheck-final-v2.log` 通过。scope 产品判断直接读取所选账号的元数据，不为每个 scope 重建所有模型目录；没有新 effect、网络 waterfall 或自动提交行为。
- 新合同入口：[长模型合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_long_model_contract.rs)、[配置矩阵](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_long_model_contract/configuration.rs)、[真实 wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_long_model_contract/fixture.rs)、[普通产品／非模型 ID 负测](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_long_model_contract/negative.rs)、[前端合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/bedrockLongModels.test.tsx)。

Rust 定向合计 **67/67**（30 个集成 + 34 circuit + 1 policy + 2 schedule），不重复累计复跑次数；前端 **75/75**。这不是全仓回归、真实 AWS 验收、打包或运行实例验证。

前置来源保留 `source-before.json`；后加入的 circuit 单测和 negative fixture 另有 `source-before-additional-v1.json`，不覆盖原证据。最终来源使用独占创建的 `source-frozen-v1.json`，不改写旧批 frozen 证据。

### 3.1 有效行与来源检查

来源 scope 共 37 个文件，本批变化 35 个；最大 441 有效行，没有新增超过 500 行的文件、例外或 baseline 变更。完整前后值见证据目录的 `code-size-delta.json`，重点为：

| Owner | 本批前 → 后有效行 |
| --- | --- |
| CredentialDialog / editor hook | 439 → 441 / 334 → 335 |
| test policy / matrix execution | 91 → 103 / 250 → 254 |
| console probe / provider compilation | 351 → 356 / 170 → 171 |
| circuit / capacity / circuit tests | 355 → 360 / 119 → 122 / 212 → 236 |
| 新 Rust model identity / compiler gate | 新增 14 / 105 |
| 新 TS model identity / UI contract | 新增 18 / 87 |
| 新集成主文件 / configuration / wire fixture / negative | 新增 186 / 133 / 78 / 64 |

251–500 范围内的旧 owner 仅接线其原责任；matrix execution 仍只拥有单轮测试矩阵与预算，不新增后台循环或协议实现。新产品 gate 与测试 fixtures 按责任独立拆分。

- `checker-final-v2.log`：checker 自测 **31/31**；`ratchet-final-v2.log`：3946 文件、14 immutable runtime artifacts，`governed source >700=0`。使用无报告写入的 ratchet 入口，未覆盖原有脏报告。
- `fmt-final-v2.log`：`cargo fmt --all -- --check` exit 0；`source-verify-final.log`：37 个 scope 文件与 frozen 指纹一致。
- `document-final-v2.log`：5 份文档严格 UTF-8 无 BOM、围栏和 111 个本地引用检查通过；scope 与文档共 42 文件的 tracked/untracked whitespace 检查通过。本批 scoped diff 已复查，未撤销前批或其他并行改动。
- 测试 Cargo session 已 exit 0；`process-final.json` 保留结束时进程快照，不把其他任务仍在编译的 Cargo/rustc 算成本批任务或停止它们。既有 Gateway 进程未重启。

## 4. 后续仍须完成

普通 OpenAI 等入站生成参数至 Bedrock `inferenceConfig` 的桥接、reasoning/signature/redactedContent、原生 modeled exception 输出、取消 audit 收尾，以及 P50 devtest 仍是独立剩余项。P08 的 socket 关闭不证明 durable audit 已终结。
其余 API/OAuth/Web/媒体平台继续按各 W 卡推进；不以本批或一个成功 ARN 代替 Bedrock 完整验收，更不代替总计划。
先完成全部公开可证实的实现和离线闭环，再集中录制与逐渠道真实验收；当前不要求用户提前提供材料。

本批有意修改 P07/P08 的部分 owner，旧 frozen JSON 继续保留历史，不覆写旧 hash 或冒称其仍代表当前来源。
默认管理 UI 的 Cargo build script 会因前端源码变化重新嵌入 UI；接力时尽量先收敛 UI 改动，再开始串行 Rust 构建，避免同一批反复重编译。观察超时不等于测试已停止，不启动第二个排队 Cargo，也不停止其他项目的进程。
