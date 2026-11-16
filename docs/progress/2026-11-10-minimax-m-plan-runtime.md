# MiniMax M Plan Key 本地管理与推理切片

状态：`partial_pass`。M Plan 本地核心路径已通过；扩大 UI 回归仍有六项已证实的既有失败，独立 feature 和真实账号验收未完成。用户继续完整 Magpie 接入任务后推进的独立子批；不把此项等同于 MiniMax Code OAuth 或全部 42 项＋23 张卡完成。

## 改了哪些内容

- 新增国际 `minimax-m-plan`、国内 `minimax-m-plan-cn` preset，以及一个 `line-minimax-m-plan` feature／`minimax_m_plan` profile。固定当前官方 `.io`、`.cn` 目标；控制台默认 provider ID 分别为 `minimax-m-plan-api`、`minimax-m-plan-cn-api`，遵循现有命名约定。
- 复用普通 Key 表单、secret grant、独立 secret patch 和 CAS；新建／替换保留稳定账号 ID，备注编辑保留待提交 Key，keep 不允许跨区域重绑。空池不添加默认模型、自动测试、价格、额度或真实验收标记。
- 三个公共文本入口复用 Anthropic Messages 协议。Messages 保留原生 thinking/signature、完整工具内容；Chat/Responses 是文本与函数工具桥接，不承诺完整 reasoning 往返。
- 发送前校验精确目标、Key、请求方法和协议；只保留当前账号 Bearer，关闭默认头和重定向，排除 Native Claude／XML 工具旁路。官方只支持 auto 工具策略，本切片明确拒绝其他策略。
- 修复本产品会遇到的两条错误结算路径：非流请求跳过共享强制 SSE 累积器，改为有界 JSON 读取及结构校验；流式请求在转换前验证原始 Messages 终态，结束帧保留至合法 EOF。读取、解码、错误帧、截断和非法尾部均不能转换为空成功或触发再次推理。

## 证据与结果

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-minimax-20261110`。临时数据库、调查脚本、日志均在该目录下。本轮保留开始前 30 个已有文件快照，后续新增一个共享接线文件快照；只评审 `scope-files.json` 范围的增量，不把工作区既有数千项改动声称为本轮成果。

| 检查 | 结果 | 回执 |
| --- | --- | --- |
| Rust 第一轮 | 11 passed / 2 failed；暴露 auto wire 形状与强制流绕过 | `cargo-minimax-r1.json/.log` |
| Rust 第二轮 | 14 passed / 1 failed；新增坏流测试未释放自身 response body，等待资源收尾超时 | `cargo-minimax-r2.json/.log` |
| Rust 最终轮 | 15 passed；含三入口 JSON/SSE、两回合工具、Native thinking、grant/CAS/keep、取消退款、六种坏流与双候选非法 JSON | `cargo-minimax-r3.json/.log` |
| Rust 相邻协议／配置 | 160 passed；复用最终编译，没有再次修改 Rust 源码 | `cargo-adjacent.json/.log` |
| 新增及 Agnes 相邻 UI | 11 passed / 4 文件 | `ui-minimax-final.json/.log` |
| 扩大 UI 回归 | 130 passed / 6 failed；失败不属于 MiniMax | `ui-adjacent-r2.json/.log` |
| 修改前目录隔离对照 | 相同 6 项失败；使用 before/providerCatalog.ts，未覆盖工作区文件 | `ui-inherited-baseline.json/.log`、`inherited-ui-comparison.json` |
| TypeScript | 通过 | `ui-typecheck-final.json/.log` |
| Python | inventory 11、reference 14、manifest validator 5 通过 | `python-*.json/.log` |
| manifest／清单 | 51 条校验和生成通过，全部保持 metadata-only | `manifests.json`、`inventory.json`、`references.json` |
| 有效行 checker | 31 passed；ratchet 通过 | `line-checker-tests.json`、`line-ratchet-final.json` |

UI 对照的六项失败具体为：Kiro OAuth 通用创建夹具缺 Profile ARN 导致两项失败，Cloudflare 默认名缺通道后缀一项，DIM 两区域及 Nous 的显示名称与通用测试预期不一致三项。本轮未改变这些产品合同或弱化相关断言。MiniMax 自身命名已按既有测试要求修正。

第一轮和第二轮都是在本地非转发 peer 上运行，合成 Key 没有进入真实推理。第一轮前端启动命令的 Vitest root/config 路径曾错误，已修正；后续测试真实运行。历史失败日志保留，最终结论只使用修复后回执。

格式检查、scoped `git diff --check`、UTF-8 无 BOM 检查及最终 Rust/TS 源码无漂移通过。最终汇总为 `final-validation.json`，变更范围为 `scope-files.json`，增量审阅为 `scoped.diff`。本轮所有受影响源码均不超过 500 有效行；主要产品边界 255 行、SSE 守卫 155 行。

## 未验边界与停止条件

真实订阅 Key、套餐权限、实际消费／退款、原生 EXE、浏览器视觉、Server／Redis、正式包、部署均未验。这里的退款指 Gateway 本地预扣次数恢复，不代表上游会退还已消费订阅额度。没有提交、推送、出包、重启或读取真实 `.ng`／凭据。

官方模型目录虽公开，但没有承诺按订阅权益过滤；remains 成功响应 schema 未公开。本切片保留手工模型，不伪造目录能力、余额或 entitlement。官方 Codex Responses wire 没有作为本实现的独立原生 wire 接入。

MiniMax Code 原生 `mcode-public` device OAuth、Agent mavis、免费 credits 与订阅 Key 分开，仍待实现。当前文档和域名变化见 [官方合同](../providers/minimax-m-plan-contract-2026-11-10.md)。原始完整接入清单继续保留在 [Magpie 看板](../providers/magpie-workboard-2026-11-08.md)。

独立 `--no-default-features` 裁剪构建本轮没有重跑。上一 Agnes 子批已通过 Agnes 与 River 两个对照定位 S06 Gemini Web 的相同 15 个 feature 门控错误；该 lane 保持原 owner，本轮没有修改，不能声明 MiniMax 单独 feature 编译已完成。
