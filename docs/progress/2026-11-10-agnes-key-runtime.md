# Agnes AI 国际站 Key 接入进展

本轮承接会话 `01a1bfd4-a07b-7eb2-9b0f-14a8be6ac4ea` 最后被限流中断的 Agnes 切片。
原三图 42 项、红框 23 张卡及聚合子渠道仍按完整看板跟踪；本条记录不代表整体接入完成。
默认功能配置下的本地切片和定向回归已通过；单独启用 Agnes 的裁剪构建仍受 Gemini feature 门控错误阻碍，
本轮最终状态为 `partial_pass`，不将独立构建标绿。

## 实现范围

- 控制台创建固定国际站空账号池，新增/编辑显式 API Key 或订阅 Key，无猜测模型、价格或套餐权益。
- 凭据与目标验证覆盖 provider 声明、账号覆盖、编译和最终发送；沿用 secret grant、稳定账号 ID、
  revision/CAS、secret keep 目标绑定与本地持久化机制，没有新增数据库版本或自动刷新流程。
- 目录读取按明确账号或新 Key 请求，只产生元数据；表单应用后走普通保存，目录错误保留表单和既有状态。
- 公共 Chat、Messages、Responses 的文本与函数工具由共享协议 owner 处理；Messages 桥接到 Chat，
  Chat/Responses 使用对应原生上游 wire。覆盖 JSON、SSE、两回合工具、账号选择、审计、预付扣次、取消退款与源释放。
- manifest、sample 和 canonical inventory 已登记 Agnes，实现线元数据总数为 50；未提供真实账号证据，
  inventory 仍为 `metadata_only`，目录声明不等于推理成功。

## 验证发现并修复的缺陷

1. 自动保存完成前修改备注，`keep` 操作会清掉 Agnes 尚未提交的 Key replacement。
   前端回归先得到 1 pass / 1 fail，修复现有 editor 分支后通过；相邻 Qwen/Kiro/Vertex Express/Luma 编辑测试通过。
2. Messages 的上游 body 已转换为 Chat，但 `RequestPlan.response_kind` 仍为 Messages，触发最终产品校验拒绝。
   现在仅对 Agnes Messages 修正为实际 Chat wire，共享通用计划的既有语义保持。
3. 不透明的 Agnes 模型名触发通用 XML 工具注入，移除了原生 function schema。
   现在明确保留 Agnes 原生函数工具与两回合返回绑定；不靠模型名推断或伪装已验证能力。
4. Responses 被通用快捷分支识别成 Native Codex，绕过 Agnes 的最终身份头校验和测试目标映射。
   已在 Native Codex 分发条件中排除 Agnes，并加入 JSON/SSE 分类回归。测试构建还会在错误进入
   `send_native_codex` 时、创建请求之前立即失败，专门防止该类测试隔离回归。

Rust 首轮为 9 passed / 4 failed，第二轮为 10 passed / 4 failed。原失败日志和断言均保留。
第二轮 Responses 请求使用合成无效 Key `cpk-synthetic-agnes` 实际抵达真实 Agnes 服务并返回 401；
没有读取真实凭据，但这轮测试未完全隔离，不能将其描述为纯 loopback 验证或真实账号成功验收。

5. 官方 Theta 文档要求兼容两套用量名称，既有 Responses buffered 解析只识别 `input_tokens/output_tokens`。
   已在现有 parser 补 `prompt_tokens/completion_tokens` 和缓存详情别名，原字段优先、必要计数缺失不填零，
   总数仍使用检查溢出的相加；增加 parser 与公共 JSON/SSE 到审计记录的回归。
6. 通用 JSON 重组无法保留 `incomplete_details/error`。Agnes Responses 现在复用原生 JSON 正文返回路径，
   仍经过 Agnes 最终 sender，未完成响应保留原始详情。失败/取消、非终态、非空 error 或缺少 output 数组
   则返回固定 `502 / agnes_response_failed`，不进入成功结算、退还本地预扣且禁止自动重放，不泄露上游错误原文。

第三轮 16 项通过，已证明原四项失败修复；上述用量和响应合同补齐后，第四轮最终 20 项全部通过。
相邻 OpenAI、Responses、Nous、OpenAI-compatible builder 和 Native Codex 定向回归 173 项全部通过。

## 当前验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-agnes-20261110`。

- `cargo-agnes-r4`：20 passed / 0 failed，覆盖管理、目录、三公共入口、两回合工具、审计、预扣及退款、
  Responses 字段兼容和错误响应不重放、Native Codex 错分测试隔离。编译 13m 09s，测试执行 5.10s。
- `cargo-adjacent-r1`：173 passed / 0 failed，复用测试二进制；执行 18.37s。与 Agnes 筛选有两项重复，
  不将分批数字直接相加当作独立测试总数。Rust lib test 构建仍报告 76 条 warnings。
- `cargo-line-check-r1`：`cargo check --locked --no-default-features --features line-agnes-official-api --lib`
  失败，15 个 Gemini 编译错误。`cargo-line-control-r1` 改为单独 River feature、不启用 Agnes，得到相同 15 个错误。
  对照摘要见 `standalone-feature-blocker.json`。协议层的 `src/protocol/gemini/mod.rs:35` 在关闭 feature 时选择
  `web_reverse_disabled.rs`，上游 `src/upstream/gemini/web_reverse/mod.rs` 仍无条件加载 execution/speech/uploads，
  因而引用了 disabled 模块没有的媒体符号。该范围由 AGENTS.md 与并行看板保留给 S06，本轮未接管修改。
- `ui-agnes-r1`、`ui-agnes-r2`、`ui-editor-green`、`ui-editor-r2`：前端分批去重 9 文件 / 79 项通过，
  其中 Agnes 专属 12 项、相邻 67 项；覆盖新建/替换 Key、暂存 keep、目录失败及晚到结果隔离。
- `typecheck-r3`：TypeScript 类型检查通过；先前测试夹具 `modelCatalog: null` 已按类型改为 `undefined`。
- `python-inventory-r1`、`python-references-r1`、`python-line-list-r1`：分别 11、14、2 项通过。
- `manifests-r2`：50 份 manifest 通过；首次错误是新增 manifest 引用了不存在的 Rust 文件名，已修正。
- `checker-r1`：行数检查器 31 项通过；`ratchet-r5` 最终源码门禁通过，governed source 超过 700 行为 0。
  本切片范围最大手写文件为 494 有效行，无新增超限例外；分类的 immutable runtime artifacts 单独报告。
- `source-r4.json` 是最终响应合同补齐后的 Rust 构建前范围源码摘要；`source-final.json` 与
  `final-validation.json` 记录最终 UTF-8 无 BOM、范围哈希和检查 receipt。不是整个继承 dirty tree 的完整审计。
  `rustfmt-final` 和 `diff-check-final` 为最终范围检查；最终回执同时保留独立 feature 构建失败与对照失败。

## 仍开放的验收边界

2026-11-10 已匿名联网复核公开推理与订阅 Key 文档，见
[合同与资料边界](../providers/agnes-contract-2026-11-10.md)。官方文档没有直接给出 HTTP Models API 成功合同；
匿名 `/v1/models` 的 401 只证明鉴权响应，不能代替真实 Key 目录与推理验收。

真实账号、订阅额度/费用、国内站、网页登录、Agnes Code、媒体、原生包和浏览器视觉、Server/Redis 与跨应用验收仍开放。
本轮未读取真实凭据或访问真实 `.ng`，未进行付费推理，未修改运行实例，未提交、推送、出包或部署。
