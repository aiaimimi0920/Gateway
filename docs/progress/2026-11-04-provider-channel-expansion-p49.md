# P49：Kiro 历史 folder 材料归一化与固定设备身份

日期：2026-11-04（Asia/Shanghai）。状态：**本子批定向离线验证通过**。P48 已完成的 typed context、持久刷新和秘密保存合同保持有效；R02/W02 与总目标继续 active，不增加模板、整卡 `offline_ready` 或真实验收标记。分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有并行脏改，未提交或推送。

## 1. 本批范围

- 在既有 `normalize_import_payload` 的 `kiro-compatible` 实际分支接入专用纯材料转换，不新建文件监听、数据库写入或刷新系统。原文件容量限制、路径约束、source hash 和 snapshot/CAS 更新顺序不变。
- 根对象与 `extraBody/extra_body` 中受支持的历史名称先统一查找，再判同义字段冲突；同值重复可折叠，不同值和错误类型拒绝，不按 HashMap 顺序选择。两个 extra 对象也必须完全相同，不能静默拼装。
- 完整注册材料转换为 credential-owned `kiro_oauth`；必须显式具有认证方式、directory Region/Start URL、client ID/secret、注册绝对 expiry 和 scopes，不从 API Region、access expiry 或历史参考自动补缺。只接受现有 `builder_id/identity_center` 枚举，不把 Social 套成 AWS OIDC。
- 已有 canonical context 继续严格解析并比较其并存 legacy aliases；缺 secret 的 redacted metadata 不自动恢复秘密。只有 bearer 的旧材料可以保留为 bearer，不制造 OAuth 注册或通用 refresh endpoint。无 access 时必须有完整含 secret 的 context 和 RT，且不能借 provider 的非空 access 替代。
- 首次转换固定原实际 headers owner 可观察到的 machine ID：显式 machine 规范化，provider 层原显式 machine 优先于旧 extra RT 的 hash；只有根级 RT、原来没有额外 machine 输入时仍固定旧全零值，而不是突然换成新 hash。后续 RT 轮换和重新导入不重算已固定值。provider 各种 machine aliases 在 credential 中统一投影相同规范值，不继承 provider RT/注册 secret；旧 UUID/hex 和无效外层字段均已通过实际 merge→headers 验证。
- 认证副本从业务 extra 中移除；根 `rawSource` 只作旧归档丢弃，不作为补缺或新授权来源。业务 extra 中嵌套的 `rawSource/kiro_oauth/grok_oauth/accio_sgcookie` 明确拒绝；相同 key 保护同时用于 typed Kiro 的正常配置准入，防止嵌套注册/归档秘密副本进入业务材料。
- 精确核对选中 account 的 adapter/family/profile/service 与 direct execution；使用真实 DB merge、payload deserialize 和 call-flow prepare 后复用 P48 的最终业务目的地检查。不同 base、认证头覆盖、external keepalive、继承认证副本及相反 refresh endpoint/client 拒绝。
- 输出匹配 account 既有 DB key spelling，避免 camel/snake 跨层合并产生重复 serde 字段；没有修改全局 merge 或给 route credential schema 添加新 aliases。folder JSON 不因此被声明为任意 route YAML 的通用替代。
- access expiry 只保留明确绝对 RFC3339 时间，和 Session expiry 冲突时拒绝；缺省保持 unknown，不由注册 expiry 或任意 lifetime 补齐。过期材料可以作为证据读取，不代表可发送或真实权益。

## 2. 红测与实现来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p49`。

`baseline-files.json` / `before/` 保存本批原字节。`red-folder.log` 实际 **0 passed / 3 failed**，暴露实际 folder dispatch 没有转 typed 注册、没有固定旧派生 machine，且接受相互冲突的标准/legacy RT；不是编译失败。`red-source-verification.json` 证明红测运行期间 dispatch、协议 mod 和 business 三项生产输入仍与 before 原字节相同。

新增实现按 alias 解析、注册材料转换和最终 DB 目标投影拆分；只读子代理不修改源码。预审发现的 extra 嵌套秘密副本已补对应 folder 与正常 typed-config 负例。首轮 14 项通过后，静态复核确认 provider 同名显式 machine 会被 credential 默认值覆盖；主线程补保留原显式值，第二轮 16 项通过。随后扩展为两种 provider aliases × 64hex/历史 UUID，统一投影各别名；最终由 v5 扩大回归验证，不把前两轮通过当作新输入已经验证。

最终窄复核再发现 account fallback 复用了 raw importer 的宽外层 aliases，能把原 runtime 不消费的 `extra-body/ExtraBody/EXTRA_BODY` 升级为新 machine 来源。已只保留精确 `extra_body/extraBody`，并补“忽略属性不改变最终 User-Agent machine”的实际 merge→headers 负例。v3 验证尚在首个 Cargo 编译时被主线程因该确认缺陷取消：先核对唯一 P49 wrapper 的 PID/启动时间/命令，再仅停止它和已记录的后代；`cancelled-v3-processes.json`、`cancelled-v3-receipt.json` 和 `regression-folder-v3-cancelled.log` 保留，不计入通过，不停止其他项目编译。不是因观察超时而重启。

v4 主配置四组已完成，但代表性关闭在实际编译时暴露 `E0425`：新增 importer 使用的 `normalize_machine_id` 不在 `kiro_disabled` 导出。旧关闭分支的 machine 派生和 metadata lookup 也直接返回 `None`，不能满足离线材料保真。已将原纯 lookup/设备算法外提为两个构建共同使用的 `kiro/material_metadata.rs`，保留原开启模式的规则、显式值优先级和 hash；关闭模式只恢复纯转换，不开放 packing 或 OAuth 网络。新增三项 exact hash/UUID/lookup 测试。原失败与四组结果保留为 `*-v4.log/json`，最终输入更新为 v5 并重新执行受影响回归，不把 v4 来源冒充最终来源。

## 3. 最终验证与来源

`green-folder.log` 为 14 passed；`green-folder-v2.log` 为 16 passed。v4 主配置四组通过后，feature-off 编译失败；上述历史结果不冒充最终来源。最终 `source-green-input-v5.json` 冻结共享纯材料模块修正后的输入，`verify-v5.ps1` 全部八步 exit 0：

| 配置 / 过滤器或目标 | passed | ignored | 日志 |
| --- | --- | --- | --- |
| runtime-defaults / `--lib provider_credential_folder_sync` | 158 | 14 | `regression-folder.log` |
| runtime-defaults / `--lib kiro` | 114 | 0 | `regression-kiro.log` |
| runtime-defaults / `--lib token_refresh::tests` | 122 | 4 | `regression-refresh.log` |
| runtime-defaults / `--test console_document_contract` | 70 | 1 | `regression-context-console.log` |
| runtime-defaults / `--test kiro_oauth_context_contract` | 15 | 0 | 同上 |
| 仅去掉 Kiro line / folder 新材料套件 | 17 | 0 | `feature-disabled-folder.log` |
| 同一关闭组合 / `protocol::kiro_oauth` | 10 | 0 | `feature-disabled-protocol.log` |
| 同一关闭组合 / `token_refresh::tests::kiro` | 5 | 0 | `feature-disabled-refresh.log` |
| 同一关闭组合 / `protocol::kiro::material_metadata` | 3 | 0 | `feature-disabled-metadata.log` |

所有 Cargo 命令共同使用 `test --locked --jobs 4 --no-default-features` 与 `-- --test-threads=1`；关闭组合展开 runtime-defaults 后只移除 `line-kiro-official-vendor-api`，精确列表在 `feature-off-list.txt`，没有暗中重新启用 runtime-defaults。`RUST_MIN_STACK=16777216` 仅作用于本次测试子进程；TEMP/TMP 指向本批 linshi 子目录。没有启用默认 management-ui 的工作区发布/清理 hook。

`verification-summary.json` 按完整测试名去重：**主配置 449 个不同 Rust 测试通过，21 个本批新增全部有主配置通过记录；另 19 ignored 不计通过。** 关闭组合 35 项通过，仅 `compiled_out_line_rejects_registration_without_network` 一个测试名不在主配置，因此不把重复项相加成 484 个独立测试。它是代表性关闭验证，不是任意 feature 组合或全仓回归通过。

checker 31 项通过（`checker.log`），checker 本身未变，不重复执行。`static-v5-exits.json` 的 Rustfmt、ratchet、Git diff 均 exit 0；ratchet 使用同一官方 checker 的无报告写入入口，保护已有脏报告。`audit-v5-final.log` / `source-frozen-final.json` 核对 UTF-8 无 BOM、空白、有效行与本批 before 字节差异；**13 项 Rust 输入与 v5 冻结哈希一致**。新增文件最大 244 有效行；共享纯材料模块117，原 Kiro owner377→320，disabled69→59，未新增超限或例外。`scope.diff` 保存本批范围差异，未撤销并行改动。

本批是 folder ingress 的转换与保存前准入。生产导入仍沿原 DB update/create；当前离线测试不证明真实 PostgreSQL 写入、持续文件监听、旧 source hash 未变化文件自动重扫或 hosted DB 已获得 P48 route-document scheduler 的全部能力，不能把这些不同 owner 的缺口藏在“已导入”里。

## 4. 后续仍需完成

1. 旧本地 route document 的显式转换/冲突处理，以及已有未变 source hash 的历史 DB 材料如何在正常生命周期内升级，仍需独立兼容方案；本批不重写现有数据库或用户文件。
2. 下一条功能链仍是普通 Console device 登录、receipt/poll/cancel、SecretPatch/CAS 保存及 UI；复用既有有界会话/保存任务生命周期和原 scheduler，不另建无限轮询或持久化系统。profile/subscription、最小 scopes、当前 Builder ID verification URL 样本及真实产品/client/额度仍未证。
3. Accio/Grok 余项、Qwen/Vertex 未决认证、其他 API/Web/媒体及 P50/P60/P70 不变。按用户要求先无凭据开发，最后集中录制与逐渠道验收，不因本子批删减原范围。

本轮另完成公开资料增量核实：[Kiro 合同 §7](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-kiro-oauth-contract.md)记录当前 ACP/device-profile 治理、legacy/new 域名，以及原生 CLI headless Key。Key 的存在与同用户订阅 credits 扣费已由官方证实，两个固定许可的第三方 direct HTTP 实现只作后续采用线索；尚未新建 Key 模板、扩大 allowlist、运行 CLI 或宣称完整官方 HTTP 合同。模板数量仍为12。

没有读取 `.ng`/历史凭据、请求真实账号或模型、创建/删除 Redis 容器、修改在线实例、出包、提交或推送；未撤销并行改动，未接管 S06 Rust/Gemini lane。
