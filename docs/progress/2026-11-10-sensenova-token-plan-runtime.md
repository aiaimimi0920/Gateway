# SenseNova Token Plan 本地接入切片

状态：`partial_pass`。显式订阅 Key 的本地管理、目录、UI、三公共文本入口及函数工具切片已通过；真实账号、控制台 OAuth／积分池和独立 feature 构建仍未完成。范围属于持续 Magpie 原始 42 项＋红框 23 张卡＋聚合子渠道；不将本切片称为全目标或 SenseNova 全产品验收完成。

## 改了哪些内容

- 新增 Token Plan Key 的 preset、profile、feature 与控制台入口，固定官方 `token.sensenova.cn/v1`；不借用控制台 OAuth 身份，不与小浣熊账号合并。
- 显式 Key 创建／替换、metadata Keep、稳定账号 ID、grant 与 revision/CAS 复用既有凭据 owner。目录走独立管理接口，按官方 `output_modalities` 筛选文本输出，应用表单后正常保存；空池不默认模型、自动测试、余额或价格。
- 三个公共文本入口统一到 Chat 上游，覆盖 JSON、SSE 与函数工具双回合。最终 sender 绑定选定 Key、清除其他身份、禁跳转，排除 Native Codex 和 XML 工具旁路。
- 从 Qwen workspace 提取可复用的 Chat JSON／SSE admission；Qwen 保持原错误诊断与重试策略。SenseNova 对已收到生成回复后的失败使用固定 502 和 Abort，不将错误或截断记成成功，也不重放到第二候选。
- 交叉核验发现旧守卫会接受缺少工具身份的流，已增加每个工具的字段到齐标志及分片测试。该修复也约束 Qwen 的坏工具流；不缓存第二份参数文本，不增加后台任务。
- 原 buffered executor 已达 500 有效行，因此将“成功 HTTP 回复的解码选择”提取到 `buffered_response.rs`，保留其他产品原解码顺序，原文件降至 466 行。

## 证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sensenova-20261110`。所有临时数据库、日志和脚本在该目录内；`before/` 保存开始前的 dirty 文件，`scoped.diff` 只记录本轮增量。工作区继承大量未提交修改，不宣称整体来源或整体测试通过。

| 检查 | 结果 | 回执 |
| --- | --- | --- |
| 官方合同 | 匿名联网核实，原文与哈希留存 | `contracts/official-api.md` |
| Rust 首轮 | 38 passed；约 13m48s 编译、12.09s 运行，尚未包含后补的工具完整性修复 | `cargo-sensenova-r1.json/.log` |
| Rust 最终轮 | 40 passed / 0 failed；约 12m29s 编译、11.44s 运行，含完整工具身份、跨帧工具字段及双候选坏流不重放 | `cargo-sensenova-r2.json/.log` |
| Rust 相邻协议／配置 | 152 passed / 0 failed；复用最终编译，15.63s 运行，覆盖 Qwen 流水线、Agnes、MiniMax、公共调用计划及实现线 | `cargo-adjacent.json/.log` |
| UI 首轮 | 11 passed / 1 failed；新增测试使用了旧按钮名，未匹配实际 `-api` 后缀；已修复 | `ui-sensenova-r1.json/.log` |
| UI 修正及相邻入口 | 19 passed / 5 文件 | `ui-sensenova-r2.json/.log` |
| UI 其他相邻路径 | 18 passed / 5 文件；NVIDIA／OpenRouter 目录、通用发现、MiniMax／Qwen Key 编辑 | `ui-adjacent.json/.log` |
| TypeScript | 通过 | `ui-typecheck.json/.log` |
| Python | inventory 11、reference 14、manifest validator 5 通过 | `python-*.json/.log` |
| manifest／清单 | 52 条校验及生成通过，全部保持 metadata-only | `manifests.json`、`inventory.json`、`references.json` |
| 有效行 | checker 31 passed，ratchet 通过；当前受影响文件最大 500 有效行 | `line-checker-tests.json`、`line-ratchet-final.json` |

终态审计、取消和坏流使用非转发 loopback peer、合成 Key 与本地隔离存储。退款是 Gateway 本地预扣次数恢复，不代表上游退还已消耗额度。

Rust 两组最终检查有筛选重叠，不将测试数相加为独立用例总数。官方格式检查、scoped `git diff --check`、UTF-8 无 BOM 与最终源码无漂移核对均通过；源码以 `source-pre-r2.json` 对照 `source-final.json`，汇总为 `final-validation.json`。历史失败回执保留，结论使用修复后的实际结果。

## 未验边界

真实 Key、订阅模型权限、实际 SSE 帧布局、真实费用、上游额度退款、网页登录／续期、积分池读取、原生 Messages／Responses 完整 reasoning、媒体、原生 EXE、浏览器视觉、Server／Redis 与部署均未验。

独立 feature 裁剪构建本轮未执行；上一 Agnes／River 对照的 S06 Gemini 门控错误仍属于原 owner，不能以默认配置的测试成功声称独立裁剪已完成。本轮没有提交、推送、出包、重启或读取真实 `.ng`／凭据。

官方合同见 [SenseNova Token Plan](../providers/sensenova-token-plan-contract-2026-11-10.md)，完整范围见 [Magpie 看板](../providers/magpie-workboard-2026-11-08.md)。
