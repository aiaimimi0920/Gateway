# Bedrock 显式材料与最终发送认证合同

> 2026-11-02；属于 [分渠道扩展计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md) 的 W06/P10。
> 本文描述认证层、流式 text/tool/usage 双向切片、Console 材料及原生精确账号 probe；不代表完整 Bedrock 协议、模型目录、打包或上游验收已完成。

## 1. 认证模式不是三个额度池

显式模式保存于 provider `auth_mode` 或新增可选 credential `auth_mode`（reader 同时接受 `authMode`）；账号优先、缺省继承，不增加数据库版本或迁移旧实例：

| 模式 | 必需材料 | 明确拒绝 |
| --- | --- | --- |
| `bedrock_bearer` | `api_key` 原始 Bedrock Key；`extra_body.awsRegion` | IAM AK/SK/STS、已拼接 `Bearer ` 的值、独立 auth_token/session_auth |
| `aws_sigv4` | 空 api_key；AWS AK、SK、region | 混入 Bearer Key、半套 IAM、矛盾别名、独立 auth_token/session_auth |
| `aws_sigv4_temporary` | 空 api_key；AWS AK、SK、Session Token、region | 缺 Session Token；其他冲突同上 |

长期 IAM 不强制 Session Token；临时 IAM 必须有 Session Token。带 STS 材料也可由签名器处理，但不能据 Key 外形证明它是长期凭据。
两种认证类型不自动证明两份余额；最终按 IAM 主体、AWS account、region、model/runtime quota 识别，不合并未知额度。
新建 `bedrock-api` 的 UI/SecretPatch 必须显式选择模式，不能只加模板行或按材料“凑齐了没有”猜模式。

## 2. 现有 owner 与信任边界

- [bedrock_auth.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/bedrock_auth.rs)：模式、字段、别名、冲突和 adapter 绑定。
- [bedrock_target.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/bedrock_target.rs)：公共 runtime endpoint/region、最终 method/origin/path、model identity。
- [bedrock_runtime_helpers.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/bedrock_runtime_helpers.rs)：复用已有 AWS SigV4 实现；Bearer 由后端加前缀。
- [request_plan_part_1.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/request_plan_part_1.rs)：认证 metadata 不参与业务 JSON 合并。
- [streaming.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/streaming.rs)：`send_profiled_plan` 在最终 profile/header 处理之后统一认证，覆盖 buffered、stream 和适用 passthrough；Bedrock 请求禁止 redirect。
- [Converse planner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/bedrock_converse_official_api_common.rs)：显式模式不允许把请求 model path 改投另一模型。

IAM 使用 `extra_body.awsAccessKeyId/awsSecretAccessKey/awsSessionToken/awsRegion`。
字段识别和 JSON 排除均覆盖历史 reader 的大小写、下划线、连字符 aliases；同一字段出现矛盾值必须拒绝，不能靠 HashMap 遍历顺序选一个。
认证字段必须经现有秘密 owner 保存；它们位于 extra_body 不意味着可以公开显示或进入请求 JSON。第 6 批补齐无词边界 Access Key ID aliases 的秘密分类与 keep 受众／继承者保护；第 7 批接多字段保存链，并将保护扩展到账号有效模式。
显式模式在最终 adapter 被 call_flow 改为其他协议时拒绝，不把 IAM 材料作为 OpenAI/自定义 HTTP extra_body 转发。
旧的授权、x-amz-date/security-token/content-sha256、Host/Content-Length 不参与新签名；实际 host/port、序列化 body 和最终 headers 决定签名。

## 3. 受众、region 和模型路径

显式模式只接受已核实的公共 `bedrock-runtime.{region}.amazonaws.com` 和对应六个 FIPS 地域；必须是 HTTPS、根 base URL、无 userinfo/query/fragment/非默认端口。
地域表来自 AWS General Reference 的 **runtime** 段，不从 control-plane 段推断支持。未列入表的地域、China、PrivateLink/VPC、自定义代理、dual-stack 域名不在本批显式模式支持范围内，不自动试连。
这不是“这些产品永久不可用”的断言；未来新增应补官方证据、精确受众许可和定向测试。
最终请求必须仍是相同 origin/region 的 POST `/model/{encoded-label}/converse[-stream]`，不得携 query 或改写模型 path。

`modelId` 保留原始 identity，长度 1–2048。正常模型 ID/inference profile ID 和 ARN 可以使用；路径逃逸、query/fragment、反斜线、预编码 `%xx` 输入拒绝。
按 AWS 官方 SDK 的 non-greedy URI label 行为编码一次：ARN 的 `:`、`/` 编码为 `%3A`、`%2F`；`-._~`、字母数字保持原样。SigV4 canonical URI 的再次编码属于签名步骤，不是预先修改模型 identity。

## 4. 存量兼容与有意收紧

未声明新模式的旧 payload，以及历史通用 `auth_mode=bearer`，不自动迁移到新显式模式。
旧自定义认证 header、无 IAM 的原 headers 行为保留；旧完整 IAM 仍可使用原有 region 推断及 operator 配置的自定义 endpoint。该兼容分支**不具备新模式的官方目标 allowlist 保证**。
旧 IAM 字符串前后空白仍按历史 reader trim；字段缺项、非法材料或相互矛盾 aliases 现在 fail closed，不再静默保留另一种 Authorization。这是有意安全收紧，不声称所有错误配置行为完全不变。
不改旧 `bedrock-converse` preset 的序列化默认值，不改存量 Provider ID、秘密、运行数据库或在线实例。

## 5. 离线证明与剩余项

定向测试使用合成材料及随机 loopback 端口，不连接 AWS，不使用实际凭据：

- 两条实际执行路径捕获请求，检查 canonical/snake_case/大小写连字符 IAM 字段不进入业务 JSON。
- 独立 HMAC 计算核对实际捕获的最终 headers/body；覆盖简单模型和 encoded ARN。固定空 query 的 fixture，不外推所有 profile/transport。
- 新模式材料矩阵、region/host/path/adapter 错配、旧配置行为分别保护。
- 默认允许 redirect 的测试 client 接收 307，检查第二随机 listener 未收到携密请求。

实测数量、红绿记录和失败边界见 [第 3 批回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p03.md)。测试源码存在不代替运行证据。

第 7 批已实现正常空池模板、多字段材料、账号模式、暂存后目标复核及配置编译材料校验；UI→SecretPatch/redaction/YAML/keep/clear 的合成运行证据见 [P07](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p07.md)。
第 8 批已接精确账号原生 probe：生产 dispatcher、32-token原生上限、原生 CRC/stop/metadata/EOF decoder、validated usage、精确审计和健康归属；没有请求选项／保存策略时仍 unsupported，不隐式生成。已修模型/stream HTTP labels 不应进入业务JSON的旧打包问题。证据及取消 audit 未闭合边界见 [P08](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p08.md)。
第 9 批实现按有效 Bedrock 产品校验的长模型身份：编辑/重开/编译/映射/手动及自动测试与路由准入；普通模型编辑/测试仍 256 字节，case/plan/scope ID 不随之放宽，见 [P09](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p09.md)。
后续 [P22](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p22.md) 已补四类基础生成参数→inferenceConfig桥接与planner/defaults校验；完整协议、取消记录收尾及集成打包继续，不能把基础参数子集当全部参数支持。
第 6 批的**后端安全前置**继续保留：AWS aliases 经同一秘密 owner；keep 不能更换目的地、模式、有效 region、refresh/keepalive 受众或无授权新增秘密继承者。判断使用全部补丁完成后的局部 resolved 文档，replace/clear 仍走 Grant/revision/CAS；第 7 批已同步账号有效模式。历史证据见 [P06](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p06.md)。
第 4 批已新增独立 Bedrock 入站 owner：完整 Converse JSON 和真实 header-tagged AWS EventStream 均不再委托 Accio；验证两种 CRC、事件身份、text/tool 状态、post-stop usage、完整 EOF、异常及截断。冻结源码对应的 20 项入站、3 项真实 stage_send 回环以及既有公开合同回归见 [第 4 批回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p04.md)。合成 loopback 不是 AWS 账号验收。
第 5 批已修 OpenAI normalized SSE→Bedrock 原生出站：三 string headers、未包装 payload、双 CRC、独立 text/tool block、完整 EOF/usage 验证；按 service 限制分片，而不错误收紧 P04 client decoder。原生编码在唯一 TrackedStream 之内，真实 route 不二次编码；metadata 与 finalizer 使用同一份 validated usage，见 [第 5 批回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p05.md)。
[原生请求历史子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-signed-history.md)已接未改 canonical 投影时的原始 messages/system 有序保留、畸形 reasoning 拒绝，以及策略/extra 冲突在网络前明确失败。signature 可省略，完整 opaque JSON 值不 trim/重编码。
最新[非流式响应子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-buffered-reasoning.md)接原生 JSON/真实 EventStream 聚合、有界 native-index 快照、三公共 JSON 显式扩展/投影核对及原生 Converse 内容恢复；redacted 流片按解码 bytes 组合后一次编码，不拼 Base64 文本。正常路由补现有 planner 的 Messages/Responses 准入，JSON tool stop 无工具拒绝；新增18项定向通过，最终检查以回执为准。
最新[实时流子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-reasoning-stream.md)接 reasoning/signature/redacted 原事件、三公共 SSE 显式扩展及原生 EventStream 往返；保留 native block index/逐块 close，成功等待完整 CRC/state/DONE/真实 EOF。普通 text/tool input 用同帧引用避免重复扩张；原生过大片按 service 限制拆分，不收紧 client decoder 为 24 MiB。最终Rust去重478 passed/1 ignored（15新增），正常/代表性关闭生产lib、格式、checker31与ratchet4810通过，来源/失败边界以子批终态为准。
最新[公共续接子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-public-history.md)新增响应 optional continuation 与顶层显式 history，绑定 source、实际 resolved model 和最终原生 messages/system 摘要；三公共 JSON/SSE 三轮及跨模式、policy/defaults 冲突、网络前零发送和健康不变已有定向与相邻终态证据：Rust按harness与完整名去重631 passed/0 failed/2 ignored（历史21新增），正常/代表性关闭生产lib、25Rustfmt、checker31/31和ratchet4816通过。13终态阶段各4241输入与终态一致，25Rust最大497有效行；来源/范围与根报告直接恢复见回执，不报全仓绿。ordinal 按原生 assistant turn；Responses split/并行结果稳定合并，字符串 ToolResult 保留 text，不发非法 name。旧 turn 仍可读但不能直接回放；history 8MiB/64项，单 turn 4MiB/4096 blocks。witness 不是 MAC、凭据、权限或 AWS signature 验证，不复制内部 system/prefix，也不证明账户额度共用。
最新[异常子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-exceptions.md)保留五类合法 discriminator 的 typed 语义状态/错误分类；不把 HTTP 200 transport 改说成错误状态，不保存或回显原始 message/originalMessage。optional/未知成员按具体 shape 校验；未知/unmodeled安全502。响应开始后全部无重试/切候选许可；原生 exception frame 在唯一 tracker 已确认失败/退款后编码，公共三协议 SSE 保持中断/失败，无成功 terminal。普通Local Key、真实随机回环公共TCP的原生/buffered/SSE异常与一次发送、失败audit/退款/permit释放已有终态定向/相邻证据：Rust按harness与完整名去重236 passed/0 failed/0 ignored（9新增），正常/代表性关闭生产lib、15Rustfmt、checker31/31及ratchet4820通过。10终态阶段各4245输入一致，15Rust最大456有效行；一次性feature选择脚本失败/补跑及写前来源、范围/编码/根报告恢复见回执，不报全仓绿。typed rate-limit/model-circuit分类恢复，既有shared availability/health policy不在本批重写。
最新[公共TCP子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-public-tcp.md)用生产AppState/Local SQLite、正常管理HTTP创建普通次数Key、Gateway与合成AWS两个随机回环server，已验三公共协议与原生入口JSON/流式三轮成功、工具/签名历史及第三轮跨模式；真实socket关闭覆盖头前、body前、残帧、文本、reasoning、未完工具参数和完整AWS序列后EOF悬挂七边界。精确audit/usage/余额/permit/上游Drop/无重放与健康有断言；native成功终态仍等真实EOF，不为测试提前发送。24成功+56取消是80次one-send场景，最终Rust去重22 passed/0 failed/0 ignored（16新integration+6既有lifecycle），7Rustfmt/checker31/ratchet4827通过，5终态阶段各4252输入一致，7Rust最大216有效行。生产源码未改；正常/代表性关闭生产lib编译按旧4245输入同hash复用，失败分类/来源/范围/编码与根报告原bytes恢复见回执。
普通Local次数Key的上述公共TCP成功/工具/历史/七边界取消缺口已退役；兼容loopback/Bearer合成配置不代替显式官方AWS认证，buffered有意聚合/converse-stream，本批未新增/converse上游JSON传输验收。完整参数/模态、其他适用余额/权限/存储/运行模式联合、Server/PG/Hosted、独立进程/原生包仍缺，不能以这些子链宣称全部Bedrock协议或所有计费/权限场景均已支持。
W06 尚未达到整体 offline_ready；IAM 身份权限、模型开通、真实额度、刷新/到期、集中验收均未证明。

## 6. 固定公开依据

- [Bedrock API Key](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-use.html)：原始 Key 由 Bearer header 发送；不证明权限或第二份额度。
- [短期 Key 生命周期](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-how.html)：短期 Key 具有地域/期限约束，不能跨区 fallback。
- [IAM 临时材料](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_temp_use-resources.html)：临时凭据调用必须包含 Session Token。
- [Runtime endpoints](https://docs.aws.amazon.com/general/latest/gr/bedrock.html)、[VPC/FIPS](https://docs.aws.amazon.com/bedrock/latest/userguide/vpc-interface-endpoints.html)：公共 runtime 与企业 endpoint 的支持边界。
- [Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html)、[ConverseStream](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ConverseStream.html)：操作、modelId、inference profile/ARN 和权限合同。
- AWS 官方 Rust SDK `44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7`：[Converse serializer](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/bedrockruntime/src/operation/converse.rs#L238-L249)、[Stream serializer](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/bedrockruntime/src/operation/converse_stream.rs#L255-L266)、[label](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/aws-smithy-http/src/label.rs#L14-L33)、[encoding](https://github.com/awslabs/aws-sdk-rust/blob/44d6380d22c14bd5c89c59ceafcda7bc4da5b6f7/sdk/aws-smithy-http/src/urlencode.rs#L8-L16)。仅行为参考，自有代码未复制 SDK 文件、未新增依赖或分发 SDK。
- [AWS EventStream 规范](https://smithy.io/2.0/aws/amazon-eventstream.html)、[ContentBlockDeltaEvent](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ContentBlockDeltaEvent.html)：upstream frame header 与 payload 不能混同 wrapper JSON。

## 7. Console 材料入口与配置编译边界

`bedrock-api` 仍是正常空池模板，不创建账号、默认模型、路由或自动探测。账号可选择 Bearer/IAM/temporary；不同 AWS 主体或产品确有独立额度时可创建多个独立池，不把材料模式数量当作额度数量。
新材料只在受控表单与 `CredentialSecretEdit` 内暂存；公开 JSON 只写 mode、受控 endpoint 和 region。权限撤销/关闭/Provider 切换清材料；模式/region/FIPS 变化必须重新录入全套材料，不读取旧秘密凑齐新模式。
提交前按稳定 ID 复核非秘密受众；后续 JSON 修改认证、region、flow、refresh/路径等会拒绝旧暂存值。旧 AWS 和 Authorization/Cookie aliases 的每个 exact path 都有明确 clear；普通 Grant/revision/CAS 不旁路。
runtime 的 `extra_body` 是 exact-key merge；账号改区时只在账号层覆盖 provider 中每个 region alias 的同名键，再写 canonical `awsRegion`，不修改 provider 的共享元数据，也不假装 runtime 已采用 family-level merge。
旧账号只改模型等元数据时 keep，不自动转换认证；复制只带公开 mode/region，必须提供新材料。专用入口不接受 provider 级共享秘密或其他 session/discovery/keepalive 配置，使用独立空池处理。
每个实际 credential（包括 disabled）和真正 provider default account 都在最终 discovery/flow 后复用发送 reader 校验完整显式材料；空池/继承模板 base 不要求材料齐全。配置阶段没有实际 wire，最终请求的 method/path/model 仍在发送时验证。
缺省账号模式不序列化新字段；Hosted 两层 aliases 各自规整，同层双 alias 仍拒绝。Hosted 历史 `blank=继承/null=移除` 与 typed `None=继承` 不冒称相同；新表单始终提交显式字符串。
本入口不自动取得或刷新 STS，不证明临时材料尚未过期或具备模型权限。UI/manual/model-selection/test-policy 的长身份由 P09 按最终产品限定至 2048 UTF-8 字节；其他产品和非模型 ID 不机械放宽，真实 AWS 权限与额度仍待集中验收。独立清空全部必需材料不能形成可执行账号，需完整替换或删除账号。
