# 2026-10-20 上游客户端参数套件

## 本轮范围

按用户要求，以 Gateway 的协议、路由配置及执行框架重新实现可选择的上游客户端参数，
参考 CPA 最新主分支、本地 CPA fork 及 sub2api；没有直接拷入其 executor 或另建转发服务。
本轮起点为 `07a46c0c5d78132bcddf7cbdb82e893786025a40`，本轮未提交、推送或部署。

使用说明见[上游客户端参数配置](../request-client-profiles.md)。

## 已实现

- `auto`、`codex_cli`、`claude_cli`、`cpa` 四套配置；默认 Claude → Claude CLI，
  GPT 及其余模型 → Codex CLI。服务商默认与账号显式覆盖分别编译，旧配置无须迁移。
- 新建服务商表单及已有服务商卡片详情提供选择；账号级覆盖使用既有配置文档。
  更新只修改该字段，保留账号、OAuth 刷新字段、Basispoints、站点健康及其他服务商设置。
- 推理专用 header builder 统一应用套件；通用 buffered/SSE、passthrough、
  ChatGPT 官方执行入口和 Gemini API 执行入口接入，不修改管理/配额调用的 header builder。
- Codex Responses 仅补缺失的 `store`、`include`、`parallel_tool_calls`、`instructions`；
  原有业务字段、生成参数和 continuation 保留。Claude beta 按目标协议、工具存在与否合并。
- 认证头与软件指纹名字碰撞时保留认证；不伪造 OAuth scope、账号身份或固定系统提示词。
- Basispoints、Kiro、浏览器产品和非文本接口不套用通用指纹。回归测试发现 Basispoints
  的 canonical adapter 为 OpenAI，已增加原始产品 adapter 排除，避免误入专用通道。

## 已完成的验证

1. 实现前的 Rust focused 测试按预期失败：缺少 Codex Responses `store: false` 缺省值；
   前端新增服务商序列化测试亦先失败，再实现到通过。
2. 最终 Rust 配置/请求合同 **8/8 通过**：继承、覆盖、旧格式、非法枚举、显式值保留、
   官方 Codex 执行旁路、48 个认证头碰撞组合、条件 beta 及专用通道排除。
3. 最终 Rust runtime 合同 **1/1 通过**，捕获 **72 次真实本机回环 HTTP 请求**：
   4 套配置 × 3 类模型 × Responses/Chat/Messages × buffered/SSE，均从实际发送链路验证。
4. 前端相关 **5 个文件、28/28 测试通过**；`typecheck` 与 Tauri 前端资源 `build` 通过。
   此处的资源构建不是原生桌面 EXE 打包。
5. Python 全套 **362 项，358 通过、4 跳过**。首次因当前 Python 缺少 `jsonschema`
   导致 24 项失败；在 `linshi` 独立 venv 按仓库 `jsonschema==4.25.1` 安装依赖后全套复跑通过，
   未修改系统 Python 或仓库依赖。
6. Node scripts **2124 项，2122 通过、2 跳过**；44 条 line manifest 校验通过。
7. 有效行数 checker 测试 **31/31 通过**，ratchet 通过；相关源码不需要超限例外或 baseline 更新。
   格式检查、UTF-8 无 BOM 检查与 `git diff --check` 通过。
8. 最终源码的 `cargo check --locked --all-targets` 通过。相关源码文件最大 483 有效行，
   新文件最大 253 有效行，未增加历史超限债务。

## 全量 Rust 运行与重启夹具修复

完整执行了 `cargo test --locked`，并以 `--no-fail-fast` 继续收集全部目标的结果，未把失败忽略成成功。
库单元测试为 **3485 通过、36 忽略**，另有二进制、集成与 doctest 结果保存在完整日志中。

首轮原生请求透传合同要求请求不能增加任何字段，与用户要求的默认 Codex 参数冲突。
已更新 `tests/account_protocol_routing_contract.rs` 的 Responses 预期，准确要求四个缺省字段，
同时保留其完整 JSON 比较、原有扩展、工具、continuation 和原样 SSE 校验；修改前 258、
修改后 264 有效行。后续全套运行中该合同的两项测试通过，没有改动生产行为去迎合旧预期。

全量运行最初还有 9 项 model-recovery 测试因未设置 `TEMP/TMP` 到 `linshi` 而失败。
修正测试进程环境后，`console_model_recovery_contract` **9/9 通过**，不需要修改源码。

另两项在正确临时目录环境中仍失败，诊断日志确认两处重启夹具均为
`strong=1, weak=1`。`build_router` 会启动站点健康后台任务，该任务以弱引用持有
`AppState`；即使只剩一个强引用，`Arc::get_mut` 也不能返回可变引用。
原夹具在活跃应用状态上直接替换 SQLite 运行时，因而在 `unwrap()` 处失败：

- `tests/console_named_test_plan_contract.rs:174` 的
  `two_plans_in_one_scope_keep_separate_results_and_slots_across_restart_and_deletion`。
- `tests/console_test_policy_contract.rs:222` 的
  `manual_and_automatic_share_overrides_quality_and_sqlite_results_without_redis`。

按真实重启语义修正这两处夹具：通知旧后台任务关闭、关闭旧 SQLite 运行时、创建新
`AppState`、重新打开相同数据库目录并重新获取控制台授权。没有修改生产生命周期代码，
没有删断言、忽略测试或使用不安全的 Arc 可变访问。命名计划夹具由 303 变为 306 有效行，
测试策略夹具由 276 变为 282 有效行。

聚焦复跑模型恢复、命名测试计划、测试策略三个合同，**13/13 通过**，包括重启后的结果保留、
调度槽不重复执行及后续计划删除/手动测试校验。原始失败日志保留，不能用夹具修复反向声称
已经重编译并验证过原始 HEAD 的 Rust 基线。

最终在正确的 `TEMP/TMP` 环境中重新执行：

```powershell
cargo test --locked --no-fail-fast -- --test-threads=1
```

命令正常退出，105 个结果汇总为 **4118 通过、0 失败、104 忽略**，包含库、二进制、
集成测试及 doctest。忽略项保持仓库既有状态，没有为通过门禁新增忽略。
本轮参数功能的配置合同、72 次回环 HTTP 请求及两处重启夹具均包含在最终全量结果中。

## 全量前端门禁的已有失败

全量前端结果为 **707 通过、12 失败，另 1 个 suite 加载失败**，共 145 个文件。
失败涉及既有账号/分组/布局断言、模型映射关闭回调，以及 `node:test` 在 Vitest 中的加载问题。
本轮没有修改这些失败测试或为本功能修复无关业务。

为避免把推断写成事实，已在 `linshi` 从起点 HEAD 导出只读基线前端源码，
复跑全部 11 个失败文件，复现相同 12 个断言及同一个 suite 错误。
对换行归一化后的 13 个失败标题逐项比较一致，新增套件的 focused 测试则单独验证为通过。
因此不能将本轮描述为“全仓全量门禁绿色”。

## 证据与未验边界

本轮原始证据保存在：

```text
C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-request-profiles-20261020/
  rust-focused-final.log
  rust-check-final.log
  rust-full-final.log
  rust-full-rerun.log
  rust-full-summary.json
  rust-console-environment-rerun.log
  rust-restart-ownership-red.log
  rust-restart-focused-green.log
  rust-full-closure.log
  rust-full-closure-summary.json
  frontend-focused-final.log
  frontend-typecheck-final.log
  frontend-build-final.log
  frontend-full-final.log
  frontend-baseline-failures.log
  frontend-baseline-comparison-final.json
  python-full-final.log
  node-scripts.log
  line-manifests.log
  effective-affected-final.json
  static-closure-returncodes.json
  final-receipt.json
```

没有使用真实上游账号进行付费调用，没有验证真实服务商对套件的接受程度或模型质量，
没有新增 Codex 上游 WebSocket，没有执行 44 条 feature 的完整编译矩阵或发布包验收，
没有更改运行实例配置、重启服务或迁移本地数据。初始已有的本地锁/缓存/度量文件状态保留。
