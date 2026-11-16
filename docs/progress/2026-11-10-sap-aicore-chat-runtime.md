# SAP AI Core 显式 Chat 部署本地接入

状态：`partial_pass`。SAP 自有 Service Key、显式 Azure OpenAI Chat 部署的本地管理／UI／推理切片已完成最终运行验证；Rust 联合轮 207 passed / 0 failed，其中 SAP 相关 19 项。真实账号、部署发现及其他 SAP 模型协议未验。持续范围仍为 Magpie 原始 42 项＋红框 23 张卡＋聚合子渠道，本切片不代表全目标完成。

## 改了哪些内容

- 新增 `sap-aicore-chat` preset／profile／feature，控制台入口为 `sap-aicore-chat-api`。每个池明确填写部署 URL、资源组和 API 版本；账号显示只读部署 ID，不进行模型猜测、自动发现或付费探测。
- 完整 Service Key JSON 复用既有 secret slot、grant、SecretPatch、CAS 与稳定账号 ID。支持创建、替换及 metadata Keep；Keep 绑定产品、部署、资源组和版本，不让旧秘密改变目标。
- 增加 client_credentials token owner：有界缓存、同条目合并获取、密钥修订隔离、双时钟过期、统一 15 秒交换预算，取消时无遗留后台任务。OAuth 与推理禁跳转和 ambient headers。
- Chat／Messages／Responses 文本及函数工具桥接到明确 Chat wire，移除 Gateway 部署别名 `model`，将显式版本放到 query。完整 reasoning、远端状态、媒体、hosted/custom tools 等在发出凭据前拒绝，避免静默丢失语义。
- 复用共享 Chat JSON／SSE 完成校验和既有结算；坏响应、错误尾部、取消不能完成计费或重放。普通 Bearer sender 明确拒绝 SAP，防止把 Service Key JSON 当 token 发出。
- 最终调查补齐管理旁路：stored-account discovery、后台 discovery 配置、通用目录 URL、通用连通性／prompt 探测均拒绝 SAP；公共 header builder 不构造 Service Key Bearer。专属控制台测试尚未实现，公共推理入口继续由 SAP sender 执行。
- `UpstreamClient` 原文件已有 500 有效行，新增缓存字段时将浏览器远端 timeout 计算移回既有 browser executor 模块；父文件降至 493 行，原 20 秒 grace 语义保留。

## 证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sap-aicore-20261110`。临时存储、日志和调查脚本均在该目录；`before/` 保存任务开始前的受影响 dirty 文件，`scoped.diff` 记录增量。大量 inherited edits 保留，`wholeWorkingTreeProvenance=false`，不声明整个工作树已验证。

| 检查 | 实际结果 | 回执 |
| --- | --- | --- |
| 官方合同 | 在线核实，Service Key／OAuth／Chat／SDK／部署原文、UTC、SHA-256 已保存 | `contracts/sources.json` |
| Rust r1 | 编译失败：两个导入路径错误、误用未引入的 chrono；已修复并保留日志 | `cargo-sap-r1.json/.log` |
| Rust r2 | 15 passed / 0 failed；编译 10m04s、测试 7.04s | `cargo-sap-r2.json/.log` |
| Rust 相邻中间轮 | 167 passed / 0 failed；尚未包含最终管理旁路修复 | `cargo-adjacent.json/.log` |
| Browser timeout 中间轮 | 1 passed；仍为 20 秒 grace | `cargo-browser-timeout.json/.log` |
| Rust 最终轮前的取消记录 | 为修正 discovery UUID 和探测错误断言，主动停止本轮编译进程；不计为测试通过 | `cargo-final-cancelled.json`、`cargo-final.json/.log` |
| Rust 最终联合轮 r4 | 207 passed / 0 failed / 0 ignored；含 SAP 19 项，编译 11m50s、测试 31.01s | `cargo-final-r4.json/.log` |
| SAP UI | 最终 8 passed / 2 文件；名称合同独立筛选 1 passed | `ui-sap-final.json/.log`、`ui-sap-r2.json/.log` |
| 相邻 UI | 6 文件的 28 项通过；完整首轮为 79 passed / 6 failed，含已修复 SAP 命名失败 | `ui-adjacent.json/.log` |
| 名称合同余项 | 最终 52 passed / 5 failed；修改前 catalog 源码对照 51 passed / 相同 5 failed | `ui-names-final.json/.log`、`ui-inherited-baseline.json/.log` |
| TypeScript | 通过 | `ui-typecheck-final.json/.log` |
| Python | inventory 11、reference 14、manifest validator 5 全通过 | `python-*.json/.log` |
| 清单 | 53 条 manifest／inventory／reference 校验通过；全部保持 metadata-only | `manifests.json`、`inventory-validate.json`、`reference-validate.json` |
| 有效行与格式 | checker 31 passed、最终 ratchet／定向 rustfmt 检查通过；当前范围最大 497 有效行 | `line-checker-tests.json`、`line-ratchet-final.json`、`rustfmt-r4.json` |
| 增量与来源 | scoped diff／UTF-8 无 BOM 检查通过；受影响源码及配置与最终编译前 hash 一致 | `scoped-review-final.json`、`source-at-r4.json`、`source-final.json`、`final-validation.json` |

相邻名称合同的 5 项分别是 Kiro draft 缺 profile ARN、Cloudflare 默认 ID 后缀、DIM 两站及 Nous label 约定；本轮对照只替换任务开始前的 catalog 源码，其余测试环境相同。SAP 默认 ID 已遵守现有 `-api` 规则，测试补充其必需的部署字段；未通过降低产品校验使测试变绿。

公共入口测试使用不转发外网的 loopback OAuth/Chat peer、合成 Service Key 与实际 LocalRuntime。覆盖文本 JSON/SSE、函数工具双回合、token 单次获取／过期／替换、管理 grant／CAS、错误不重放、取消释放连接与本地预扣恢复。未请求真实 SAP 账号；本地预扣恢复不代表上游额度退款。

最终联合轮包含四项新增管理边界回归：通用 header／后台 discovery job、stored-account discovery、默认及显式 prompt 探测、通用目录 URL。后台 job 使用有效 UUID 并断言 SAP 专属错误，避免先被通用 UUID 校验拒绝而假阳性；通用探测也要求 SAP 专属拒绝信息。相邻 188 项覆盖 Qwen、Agnes、MiniMax、SenseNova、call flow、preset、implementation lines、普通 API header、目录边界及 browser timeout，均包含在同一 207 项计数中。

## 未验边界

真实 SAP 账号、订阅／部署权限、真实 token 生命周期和费用、实际 SSE 帧、自动部署目录、专属控制台账号测试、Azure/GCP endpoint 模式、x.509、Orchestration、Bedrock、原生 Responses、完整 reasoning、媒体、桌面 EXE、浏览器视觉、Server／Redis 及部署均未验。

独立 feature 裁剪构建本轮未执行；此前 Agnes／River 对照中的 S06 Gemini 门控错误保留原 owner，默认 feature 测试不代表裁剪构建通过。本轮没有提交、推送、出包、重启或读取真实 `.ng`／凭据。

合同见 [SAP AI Core Chat](../providers/sap-aicore-chat-contract-2026-11-10.md)，完整范围见 [Magpie 看板](../providers/magpie-workboard-2026-11-08.md)。
