# Luma Agents 失败任务的供应商费用审计

日期：2026-11-17。承接[成功媒体晚到扣费补查](2026-11-17-provider-channel-expansion-luma-agents-settlement.md)。本子批实现已接线，Luma 及相邻定向运行回归通过；不是 Luma 整体完成。

## 实际缺口与范围

生成状态为 failed 时，原执行器只保留脱敏错误分类，丢弃已验证的任务 UUID 和实际 credits。公共请求和管理 probe 的审计因此只有本地审计 ID，无法对齐供应商任务。

本轮重新联网核对 [Luma Usage 指南](https://docs.agents.lumalabs.ai/guides/usage/)：生成终态与费用结算不必同步，失败不等于零扣费。沿用原独立 Agents REST 身份，不把此费用当作 App/MCP 费用。

## 实现

- `GatewayError` 增加可选、装箱的 `LumaGenerationFailure`。字段只包含已绑定 UUID、模型、操作类型、实际 credits，所有字段私有、构造仅 crate 内可用；公共 HTTP 错误仍只投影 message/type/code，没有序列化这个内部 receipt。
- 原有错误构造的 23 处仅机械补 `None`，不改变其他产品分类、retry/fallback 或鉴权行为。
- 生成执行器先校验原任务，再固定失败分类。若费用仍待结算，复用已有同 UUID、同精确 Bearer、最多 3 GET/8 秒并受原 deadline 限制的查询；只接受原任务的 failed 状态，补查失败或状态变化仍保留原失败及未知费用，不下载资产、不重复 POST。
- 公共失败 finalizer 与显式媒体 probe 写入原 SQLite 审计的 `response_id` 和 `route_trace.lumaAgentsGeneration`，没有新增数据库字段或迁移。失败路径尚无成功 selected adapter，因此公共审计使用 typed receipt 与错误的精确产品身份，probe 再校验当前 target adapter。
- 请求状态保持 failed；公共普通 Key 仍按原流程退款，probe 不扣普通 Key 余额。供应商 credits 不进入 token 或现金账本，也不会因确认非零费用而把请求标为成功。

## 验证记录

日志目录：`../evidence/luma/gateway-luma-failure-audit-20261117`。

- 红测：2 failed / 0 passed，分别证实公共失败与正常管理 probe 的 response ID 都丢成了本地 ID；不是编译或环境失败。见 `red.log`。
- 新增 9 个测试覆盖失败公共入口/管理入口、原本地退款、未知/零/小数/晚到费用、固定原错误分类、任务/模型/操作/终态错配、非法费用、HTTP 错误、查询上限、截止关闭连接、取消不重提，以及公共错误不泄漏 receipt。
- `cargo test --locked --offline --lib upstream::luma_agents:: -j 4 -- --test-threads=4`：52 passed / 0 failed / 1 ignored，运行 13.27 秒，见 `green-v2.log`。唯一 ignored 是既有专用 browser-host 用例，需要显式新 fixture 和独立浏览器会话；本轮无 UI 改动，没有把它算作通过。首次 green 编译只因新增测试漏导入合成 `KEY` 失败，补导入后完成本次编译；未改生产语义或削弱断言。
- 复用本次 Cargo 构建产出的 `target/debug/deps/neuro_gateway-4884fc59172c5aec.exe`，相邻过滤器分别为 `error::` 53 项、`retry::tests::` 17 项、`pipeline::stage_finalize::luma_agents_audit::tests::` 2 项、`provider_runtime::model_probe_audit::tests::` 7 项及 probe 错误脱敏 1 项，全部通过。见 `adjacent.log`；与 Luma 组无重复，共 132 个 Rust 测试通过，未扩大为全仓库回归。
- 生产 `cargo check --locked --offline --lib -j 4` 通过，耗时 1m44s；见 `production-check.log`。既有 warnings 保留，没有执行自动修复或依赖升级。
- checker 31 项测试和 effective-line ratchet 通过：5781 个文件、14 项 immutable runtime artifacts 单独归类，governed source 没有超过 700 行。新文件最大 199 有效行；直接行为改动文件均小于 500。
- 独立只读审查未发现正常路径串账号、公开秘密或改变退款语义的缺陷；它不替代运行验证。
- 受影响源码的 rustfmt、`git diff --check` 与本子批 26 个源码/文档文件的 UTF-8 无 BOM 检查通过。没有重跑全仓库发布门禁，也没有用本地合成上游代替真实平台验收。

## 未覆盖边界

如果观察到 failed 后，费用补查尚未返回就被调用者取消或 shutdown，receipt 仍属于被取消的 future，尚未上交外层审计；本轮只保证取消关闭连接、不派生后台任务、不重提。不能宣称取消审计已完整保留远端任务或费用。跨进程追账、超过截止后的主动重查及现金账单仍未实现。

没有读取真实账号、发起付费生成或修改 4200/4400 的程序和数据库。本轮不打包、不提交或推送 Git；当前 4400 EXE 不包含本子批源码。MCP 创作业务、其他官方操作、真实账号/播放/Server 验收继续，整项剩余数量不因此减少。
