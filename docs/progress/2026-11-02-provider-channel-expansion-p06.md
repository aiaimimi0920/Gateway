# 服务商分渠道扩展：第 6 批，Bedrock 材料保存的安全前置

> 2026-11-02；W06/P10 与 P01 的无凭据切片，**不代表 Bedrock 表单、W06 或总目标完成**。
> 分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留前批与其他既有脏改，未提交、推送、打包或重启实例。
> 本批 15 项定向合同及 83 项相关回归已通过，详见第 4 节；P03–P05 的冻结证据不覆盖。

## 1. 本批为何先做后端保护

接续 [P05](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p05.md)，准备接 Bedrock 多字段表单时，实际读取了普通 Console SecretPatch owner 和配置合并点，发现两个可复现缺口：

1. runtime 接受 `awsaccesskeyid / AWSACCESSKEYID` 等 IAM aliases，但秘密分类只识别 canonical camelCase 等形式；无词边界的 Access Key ID 可以留在公开文档，并绕过普通秘密补丁入口。
2. 普通 `keep` 只绑定 Provider/credential ID；同 ID 的模式、region、endpoint 或继承者改变后，旧材料仍可被复制到新配置。`keep` 本身不要求 Secret Grant，不能依靠 UI disabled 或另一个发送阶段的错误代替此安全边界。

本批只使用合成材料和隔离测试状态，不读取现有凭据，不连接 AWS、不调用付费上游、不修改在线 `.ng` 或数据库。
所有测试的 `TEMP/TMP/TMPDIR` 显式放在本批 `runtime-temp`；测试 fixture 的递归清理只在该临时根下发生。

R10 仍对应 `bedrock-api / bedrock-converse`。Bearer API Key、IAM、temporary IAM 是材料模式，不自动当成三份额度；仍按实际 AWS 主体、account、region、model/runtime quota 追踪。

## 2. 实现与信任边界

### 2.1 同一秘密 owner 覆盖 runtime 接受的 IAM Access Key aliases

[classification.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/classification.rs) 新增精确 compact `awsaccesskeyid` 分类。
既有分词、大小写和分隔符归一化继续复用；redaction、普通 draft 的秘密拒绝、SecretPatch 目标许可继续使用同一分类，不另造 IAM 专用明文通道。
`awsRegion / region` 仍是公开元数据，不错误变成秘密字段；它们参与后面的身份绑定。
本批不重写已有持久数据、不升级存储版本、不改旧序列化缺省。

### 2.2 keep 的后端受众与消费者绑定

- [secrets.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets.rs)：沿现有 pointer schema、稳定身份映射、预算及 keep/replace/clear 链处理；只暂存有实际值的相关 keep 路径。
- [bedrock.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/bedrock.rs)：active 或 draft 任一侧是显式 Bedrock 模式时启用，不以可修改的 Provider ID 后缀识别产品。
- [bedrock_context.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/bedrock_context.rs)：只投影非秘密认证目的地；排除 label、账号名称、模型目录、模型列表及测试策略等不改变本项受众的元数据。

绑定覆盖有效 endpoint、模式、region aliases、preset/adapter、flow/指纹/协议和路径、execution/session/runtime state、discovery API base/protocol，以及适用的 refresh endpoint/client 和 keepalive service/path。
进入、离开显式模式或切换 Bearer/IAM/temporary 时，不能靠 `keep` 沿用旧认证材料；必须明确 replace/clear，再走原有 Secret Grant 和 revision/CAS。
有 Grant 也不会把 `keep` 自动解释成 replace。

provider 级秘密还检查 draft 中实际继承该字段的 credential：必须是 active 中相同稳定 ID 的既有继承者，且有效绑定不变。
新 ID 即使目标相同也不能无授权借用旧 provider 材料；disabled 账号同样检查，不能先添加后启用绕过。
credential 原来有自己覆盖、现在改为继承 provider 的情况同样拒绝。
完整 exact-key replace 后不再继承的 credential 不算旧秘密消费者；aliases 按真实 merge 的精确键规则处理，不擅自折叠秘密路径。

provider 自身的公开认证配置也保守绑定，包括空池的未来使用语义；即使当前所有账号覆盖 endpoint，也不以 `keep` 偷换 provider 基线。
active/draft 两侧都不是显式模式的旧配置保留历史行为；这不是所有旧产品已有通用受众保护的声明。

### 2.3 顺序、资源与 keepalive 的独立秘密

继承判断使用**所有补丁已应用的局部 resolved 结果**，而非缺少秘密的 redacted draft；否则新账号的完整 replace 会被误判为继承，且结果取决于补丁顺序。
校验失败发生在返回候选给配置编译／持久化之前，不能产生部分提交。

其他 provider 不额外克隆第二套 pending 路径或生成 Bedrock context；本批没有新任务、线程、网络动作、锁或依赖。
同一稳定 owner pair 的 context 比较每次 resolver 最多一次；逐字段消费者继承检查仍在 cache 外，不因缓存跳过新继承者。
pending 路径沿既有补丁数量／字节预算有界；不复制实际材料到 context 或报错中。

只读复审又发现独立 `keepalive.authToken` 边界：即使所有 AWS 材料都 replace，旧 service token 的 keep 也不能转移到新 service URL/path。
已补专用红测并纳入 guard；keepalive 使用真实的**整对象覆盖**规则，`credential.keepalive` 存在但缺 token 不解释为继承 provider token。
这不实现 AWS 自动刷新，也不证明第三方 keepalive 服务受信任。

## 3. 明确尚未完成的功能

- 本批没有新增 `bedrock-api` 空池模板或多字段认证 UI，不能把后端安全前置称为可添加闭环。
- 当前 `ProviderCredentialYaml` **没有独立 `auth_mode` 字段**；现有模式由 provider 继承。下一批必须先决定并验证账号级显式模式的加法式 schema／合并，或明确的池级模式交互，不能直接把未知字段写进账号 JSON 后假定后端保留。
- IAM 字段的值类型、半套材料、Bearer/IAM 冲突和 endpoint 完整性主要仍由 runtime 认证 owner 校验；本批 resolver 成功不是材料有效／权限有效证明。配置编译的完整材料前置校验仍待补。
- Web/OAuth/API 的真实额度关系不由本批推断。其他原始 R01–R16 项均继续保留，不把总目标缩成 Bedrock。
- Bedrock reasoning/signature/redactedContent、模型策略、精确账号测试、devtest、集中录制与上游验收仍未完成。
- P05 提到的可选 nested cache-usage null 兼容疑点未在本批扩改；也未修既有极端 token expiry 测试失败。

## 4. 验证记录

证据目录：[P06](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p06)。测试命令均先设置三个临时目录环境变量，再使用 `rtk proxy`；Cargo 保持 `--locked --jobs 4`，没有同时运行两个 Gateway Cargo 构建。

- 第一组红测 `console-secret-red.log`：7 项，2 过、5 失败；直接证实 alias 脱敏、模式／目标／region keep、新继承者等缺口。
- 初轮绿测 `console-secret-green-v1.log`：扩展到 10 项，10 过。
- keepalive 专用红测 `keepalive-red.log`：3 项，1 过、2 失败；失败是仅旧 service token keep 仍可换 service/path，而非缺少凭据或连接上游。
- 最终三组合同 `bedrock-console-final-v3.log`：secret 10、keepalive 3、真实 management 2，**15 项全过，0 fail/ignore**；管理拒绝沿用既有 HTTP 400 + `secret_keep_destination_changed`，没有另造错误协议。
- 中间尝试保留：`bedrock-console-final-v1.log` 是测试误用 crate-private `snapshot.document()` 的编译错误，已改读公开 compiled runtime；v2 是测试误期待 422，实际既有映射为 400，修正测试而非改变生产状态码。它们不是成功证明，也不是上游失败。
- 共享 Console 回归 `console-regression-v1.log`：`console_document_contract` 64 过、1 ignored、1 filtered；`console_management_contract` 12 过；`console_management_sensitive_contract` 2 过；`site_health_secrets_contract` 5 过。合计 **83 过，0 fail**，不冒称全仓回归通过。
- 上述 ignored 是需显式导出前端合成 fixture 的 opt-in Session 回环；filtered 是前批已知未修的 `legacy_compiler_rejects_extreme_token_expiry_without_panicking`，命令明确 `--skip`，没有把它报成通过。
- `cargo fmt --all -- --check` 通过，`fmt-final.log`；checker 自测 **31/31**，`checker-tests.log`；官方同一 ratchet 的无报告写入入口通过，`ratchet-final.log`：3925 files、14 immutable runtime artifacts、governed source >700=0。未覆盖原有脏的 `scripts/artifacts/effective-code-lines.json`。
- 7 个源码 UTF-8 无 BOM 并冻结 SHA256：`source-hashes-frozen-v1.json`；`source-check-final.log` 确认无漂移。P05 的既有 v3 冻结来源仍匹配，`p05-source-still-matches.log`；本批未重跑其全部 wire 测试，不用来源匹配代替新运行结果。
- `cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` 通过，`bedrock-feature-disabled.log`。Bedrock feature 关闭仍可编译 Console guard；保留 Gemini 是沿用前批绕开既有全关组合缺符号的验证范围，不宣称全 feature 关闭通过，也未修改 Gemini lane。
- 4 份本批相关文档的 UTF-8 无 BOM、围栏和 75 个去重本地引用检查通过，`docs-check-final.log`；全工作树 `git diff --check` 与本批 6 个新文件的独立 no-index whitespace check 通过，`diff-check.log`。最终 source hash 再检无漂移，未覆盖任何前批冻结快照。

主要合同入口：

- [secret](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_console_secret_contract.rs)：aliases、raw draft 拒绝、有效绑定、跨 owner 继承、完整 replace、YAML/redaction 回环、legacy 兼容。
- [keepalive](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_console_keepalive_contract.rs)：仅 service token keep 的换目标拒绝和整对象覆盖。
- [management](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_console_management_contract.rs)：真实 router→管理认证→SecretPatch→runtime commit；精确 Grant、错 origin、无 Grant、stale revision、失败不改 active。

本批没有 TS/TSX 生产改动，不伪造前端类型检查或表单验收；没有出包、启动 EXE 或真实上游验证。
既有 unused/dead-code warnings 保留，不运行 `cargo fix` 接管其他 lane。

有效代码行按官方 lexer 实测，未新增例外、修改 baseline 或规避大小限制：

| 文件 | 本批前 → 后 |
| --- | ---: |
| `src/console/secrets.rs` | 354 → 367 |
| `src/console/secrets/classification.rs` | 135 → 136 |
| `src/console/secrets/bedrock.rs` | 新增 143 |
| `src/console/secrets/bedrock_context.rs` | 新增 69 |
| `tests/bedrock_console_secret_contract.rs` | 新增 274 |
| `tests/bedrock_console_keepalive_contract.rs` | 新增 79 |
| `tests/bedrock_console_management_contract.rs` | 新增 176 |

274 行测试文件只有真实 resolver 的材料绑定矩阵这一职责，处于允许的 251–500 区间；新生产模块均低于 150 行。原入口只留少量 wiring，没有接管无关旧债。

## 5. 接力入口

下一步继续 Bedrock 正常添加链：先亲读并测 `ProviderCredentialYaml`／payload merge／Console draft 和 SecretPatch 字段类型，确定模式归属；再以专用材料模块接控件、keep/replace/clear 和提交后受众重绑定。
不要复用 Web Session schema 承载 IAM，也不要仅加 catalog 行；Secret Grant/revision/CAS 和新后端 guard 均须保留。
后续一旦增加 credential-level 模式，必须同步扩展本批 stage/context 的**有效模式**计算与继承测试，不能仍只检查 provider.auth_mode。
W06、W05 和全计划保持未完成；最终集中录制仍在无凭据功能实现后执行。
