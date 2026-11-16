# 服务商分渠道扩展：第 7 批，Bedrock 正常材料入口

> 2026-11-02；W06/P10 和 P01 的无凭据材料保存切片；最终运行记录见第 4 节。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留前批及无关脏改；未提交、推送、出包、重启或修改运行实例。
> **只闭合材料保存子链；W06 和总目标均未完成。**

## 1. 身份与本批范围

R10 对应 `bedrock-api / bedrock-converse`。API Key、IAM、temporary IAM 是三种材料模式，不自动表示三个产品或额度池；不增加假 OAuth，也不取消同品牌其他有效渠道的调查。
按实际 account/principal/region/model quota 决定多个独立 provider，关系不明则隔离；同一空池支持多种模式的合成测试不是“真实额度可合并”的证据。
没有读取真实凭据、连接 AWS、调用付费上游或触碰真实 `.ng`/数据库。全部测试先将 `TEMP/TMP/TMPDIR` 指向本批 `runtime-temp`。

## 2. 实现 owner 与边界

### 2.1 账号模式是实际 schema，不是丢失的未知 JSON

- [schema.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/schema.rs)：新增可选 credential `auth_mode`，接受 `authMode`；缺省不序列化，保留旧缺字段文档。
- [provider_compilation.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/provider_compilation.rs)：两条 preset/非 preset 路径统一在 credential 合并后采用账号 override；discovery/flow 后校验实际 payload。
- [Hosted merge](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/db/provider_credentials/payload.rs)：两层各自规整 alias；同层重复不清洗。历史 blank/null 行为不被本批改写，也不冒称与 typed None 相同。
- [认证 reader](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/bedrock_auth.rs)：compile 与最终发送复用 `resolve_material`；配置阶段无实际 request plan，不签名、不联网。
- [keep guard](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/bedrock.rs) 及 [context](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/bedrock_context.rs)：使用 credential 优先有效模式；provider 本身不显式但账号显式时，共享秘密消费者同样保护。

空显式池/credential 继承用 base 不要求全套认证；真正 provider default 和所有实际 credential（含 disabled）要求完整显式材料。最终 wire 的 method/path/model 仍由发送 owner 检查，不能称所有请求配置已经在保存时验尽。
folder/archive/Redis 仍沿 typed credential owner，不新增数据库列或存储版本；本批没有真实存储迁移或外部 Redis 验收。

### 2.2 正常模板、表单、编辑和提交

- [bedrockMaterial.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/bedrockMaterial.ts)：精确 AWS alias taxonomy、公共 runtime/FIPS region、显式材料校验、只读公开 metadata。
- [BedrockMaterialField.tsx](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/BedrockMaterialField.tsx)：受控 password 字段；模式/区域/FIPS 改动清空全部材料。复用现有 Neuro 控件，无独立异步任务或旧材料 reveal。
- [bedrockMaterialApply.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/bedrockMaterialApply.ts)：公开文档只写 mode/base/region；新秘密进入普通 edits。账号级覆盖每个 inherited region exact key，不改变 provider 元数据。
- [bedrockMaterialBinding.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/bedrockMaterialBinding.ts)：暂存与实际 patch 构建均复核稳定身份及认证目标；逐 exact-key 清旧 AWS aliases。JSON 后改模式/region/endpoint/flow/refresh 等会拒绝旧材料。
- 正常 Catalog 空池、全局对话框、`useCredentialDialogEditor`、`credentialDocument` 已接线；不自动发现、不调用 `/models`、不补号、不造模型路由。

新账号必须全套显式材料；编辑 keep 不读取旧秘密，也不自动升级旧认证。duplicate 只复制公开 metadata。metadata-only keep 不丢此前暂存的 edits，最终仍校验 binding。
权限撤销、关闭、Provider 切换清对话框材料。专用入口拒绝 provider 级共享秘密及其他 session/discovery/keepalive 配置；不能用新账号继承旧共享秘密绕过 Grant。
clear 用于模式切换及旧 alias 移除；独立清空全部必需材料不能保存成实际可执行账号，用户应完整替换或删除该账号。不自动取得/刷新 STS，真实到期和权限尚未证明。

## 3. 实测发现并修复的额外边界

只读复审指出 provider `region`、账号 canonical `awsRegion` 在改区时可能冲突。已加红测，再按真实 exact-key merge 修复：账号覆盖 provider 所有 region 同名键；没有把前端改为 runtime 不存在的 alias-family override。
另补两项真实红测：JS taxonomy 原先可命中对象原型属性；Bedrock UI 原先仍可被外部 API classifier 暴露通用发现按钮。现使用精确 switch/ASCII 归一，并显式屏蔽 Bedrock 的通用发现入口。
这些是本批自己开发/复审的修正，不把红测失败归因于缺凭据或真实 AWS。

## 4. 验证记录

证据目录：[P07](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p07)。所有材料都是合成值；冻结来源只覆盖本批源文件，不覆盖所有既有工作树内容。

- `credential-mode-red.log`：6 项，1 过、5 fail；schema丢字段、Hosted跨层alias、材料编译及keep有效模式缺口均真实复现。
- `credential-mode-green-v1.log`：6/6 过。
- `bedrock-ui-red.log`：5 项，1 过、4 fail；不是组件已存在的假红测。
- `bedrock-ui-review-red.log`：16 项，13 过、3 fail；region exact-key、taxonomy、误露通用发现按钮。
- `bedrock-ui-final-v4.log`：18/18 过；后续含相同源码的相关回归见下。
- `frontend-regression-v1.log`：13 文件、133/133 过；对话框、model selection、普通 secret patches、Session 既有链、Catalog/flow、Bedrock 新链均有实际执行。
- `console-regression-v1.log`：104 过，0 fail；包含新 mode 6、Bedrock secrets/keepalive/management 15、其他 Console 83。
- 上述 Console document 保留 1 ignored（前批 opt-in Session fixture）、1 filtered（已知未修 extreme token expiry，命令明确 `--skip legacy_compiler_rejects_extreme_token_expiry_without_panicking`）；不冒称全仓绿灯。
- `bedrock-runtime-regression-v1.log`：30/30 过；含认证/目标矩阵、真实 buffered/streamed 最终签名/redirect 合同。未重跑全部 P04/P05 协议集成测试，不复用其旧冻结 hash 冒充当前来源一致。
- `typecheck-v2.log` 通过；`build-web-isolated-v2.log` 编译实际 main 和实际 CredentialDialog 合成 harness，通过。
- build 使用已安装 Rsbuild/React、`cleanDistPath=false`、输出全在本批 `web-build`；没有调用会 prune 仓库原 dist 的正式发布脚本。它不是标准打包或 EXE 验收。
- 浏览器验证：实际组件/真实 CSS，默认宽屏和 390×844；缺 IAM/STS 材料显示错误、通用拉取按钮 disabled；窄屏 documentWidth=390、dialogWidth=dialogScrollWidth=341，无横向溢出。截图 `ui-wide-iam.jpg / ui-narrow-validation.jpg`。仅合成 harness，不代表真实管理页/后端原生验收；临时 loopback 服务已正常关闭，viewport 已恢复。
- checker 31/31、官方 ratchet 无报告写入入口通过：3934 files、14 immutable artifacts、governed source >700=0。未覆盖原脏 `scripts/artifacts/effective-code-lines.json`。
- `material-roundtrip-v3.log`：显式执行 opt-in Rust 合同，1/1 过，内部覆盖三种前端真实 editor 导出材料；经过 resolver→YAML reload→redaction/keep→最终 account payload/模型/原生 adapter，未连接 AWS。
- `bedrock-disabled-v1.log`：`cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` 通过。保留 Gemini 是沿用前批的已知验证组合，不宣称全 feature 关闭通过，不接管 Gemini lane。
- 最终 `typecheck-final.log`、`ratchet-final.log` 通过；25 个源码/样式/测试文件已冻结为 `source-frozen-v1.json`，`source-verify-final.log` 全部匹配，UTF-8 无 BOM。fmt、文档和 whitespace 检查见相应 final 日志。
- `bedrock-material-synthetic-final.json` 与原 fixture SHA-256 相同（`fixture-equivalence-final.json`）；复用 v3 已编译的测试 EXE 显式执行最终 fixture，`material-roundtrip-final-reused-exe.log` 为 1/1 过，不冒称重新编译或出包。

中间非业务失败保留：`bedrock-ui-final-v2.log` 是测试误期待 route 不含现有 enabled/priority，改断言而未改生产路由；`build-web-isolated-v1.log` 是隔离 harness 的 React resolver 路径错误；`material-roundtrip-v2.log` 是测试借用临时 JSON 值的 E0716 编译错误。
`material-roundtrip-v1.log` 只排队等 Cargo 锁，发现后按精确身份停止该排队客户端，未执行测试、未中断正在运行的 lib 回归。后续 Cargo 等前一项 terminal 后串行；其他项目构建未触碰。

## 5. 大小与源码审查

本批涉及 25 个源码/样式/测试文件。有效行由官方 lexer 统计，未改 baseline/例外；新文件最大 211 行，没有新软上限例外。

| owner | 前 → 后有效行 |
| --- | ---: |
| CredentialDialog / credentialDocument | 421 → 439 / 468 → 476 |
| useCredentialDialogEditor / providerCredentialDraft | 327 → 334 / 142 → 145 |
| Catalog / GlobalDialogs / CSS | 429 → 436 / 287 → 289 / 24 → 26 |
| schema / provider_compilation / Hosted merge | 318 → 320 / 165 → 170 / 165 → 166 |
| bedrock_auth / target | 150 → 163 / 122 → 123 |
| keep guard / context | 143 → 166 / 69 → 70 |
| 3 个 struct literals 所在测试 | 325 → 327 / 489 → 490 |
| 新前端材料/绑定/apply/field | 54 / 51 / 28 / 41 |
| 新前端测试三文件 | 84 / 76 / 49 |
| 新 Rust mode / roundtrip 测试 | 211 / 97 |

审查：秘密只进既有 owner，后端继续校验 Grant/revision/CAS；目标严格受控、无额外 network/redirect/worker。输入为最多32768字节ASCII材料，region/mode枚举受限；没有新线程、进程、队列、轮询或凭据读取。React 使用受控状态和既有 Radix 生命周期/Neuro样式，没有新增主题/依赖；旧样式仅补自然高度说明文字，避免复用固定高度 status 造成窄屏重叠。
前批 P06 源码在本批开始前仍匹配原冻结清单；本批有意修改 P03/P06 owning 文件，原冻结证据保留为历史，不覆盖或伪称当前全匹配。

## 6. 下一步，不缩减全计划

1. Bedrock 精确账号测试当前在 `console_model_probe.rs` allowlist 被 Unsupported 截断；新增专用 probe，使用实际 dispatcher + Bedrock 原生 CRC/stop/metadata/EOF decoder，不能套 Anthropic SSE 或 OpenAI `/models`。保护精确账号和审计。
2. 长 ARN：前端 `credentialModelSelection`、后端 `ModelSelection`、`credential_test_policy::exact_model` 目前均有256字节上限，而 transport 支持2048；必须同步真实 owner 和测试，不只改文案。
3. reasoning/signature/redactedContent 等协议边界、集成打包仍缺；W06 不标整体 offline_ready。
4. 继续 R01–R16：OAuth（Kiro/Accio/Grok Build/Vertex）、普通 Qwen API、其余 Web/媒体、P50 devtest、最终集中录制及逐渠道真实验收。总目标继续 active；不提前要求真实凭据，不以 Bedrock 一个切片替代全部计划。
