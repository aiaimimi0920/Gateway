# Udio 精确账号音乐任务测试收尾

日期：2026-11-11（Asia/Shanghai）。状态：精确账号音乐任务测试的离线子批完成。承接“Gateway改造”对话中断的 Udio 子批；不是整个服务商扩展计划完成。

## 实现范围

沿已有账号测试入口接 Udio 专用音乐任务表单，使用已配置模型/别名、精确单账号、已保存配置和当次费用确认。身份、授权、请求或草稿变化撤销旧确认，关闭忽略迟到结果。后端禁止默认文本测试、批量账号、参数覆盖、错误受众或执行模式触发生成，复用普通 BrowserBacked sender、模型绑定、原任务 ID 终态校验与取消/no-replay 机制。

成功只说明歌曲任务元数据完整：返回中性 `unsupported` 及明确摘要，不进入音频可用性/IQ 成功计数，不下载音频，不恢复模型健康，不消耗公共 Gateway Key 次数。审计只保留任务完成、数量、`audioVerified=false` 和未知供应商费用，不记录歌词、歌曲 URL、远端任意消息或凭据。失败与取消沿现有审计 owner 收尾；取消本地等待不承诺撤销远端生成或退款。

本次接续未新增生产代码，保留并验证上一对话的实现及修正后的 `songs` 测试夹具。未顺带修改其他工作线。

## 已执行验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-task-probe-20261111`。

- `green-core-r2.log`：8 passed / 0 failed，包含完整终态、精确账号/模型、费用/Grant/Origin 准入、partial/pending 失败、取消、审计脱敏与已有受保护健康原样保留。
- `adjacent-01.log`、`adjacent-02.log`：110 + 54 passed，1 ignored。连同核心按完整名去重 **172 passed / 0 failed / 1 ignored**。唯一 ignored 是需要专用浏览器会话的新 Suno main/TCP fixture，不算通过。
- `green-ui.log`：6 个前端测试文件，97 passed；`typecheck.json` 与 `ui-build.json` 通过。网页从正常 `src/main.tsx` 构建到证据目录，不发布到运行实例。
- `scope-format.json`：9 个 Rust 文件通过；`whole-format.json`：当前全仓 `cargo fmt --all -- --check` 通过。
- `checker.json`：31 项通过；`ratchet.json`：官方有效行门禁通过，继承报告原 bytes 已恢复。本批 15 个源码/测试文件最大 463 有效行；没有改 baseline 或 exception。
- `diff-check.json` 与范围 UTF-8 无 BOM 检查通过。只读独立复核未发现确定缺陷，不以静态审查代替测试。
- `production.json`、`production-udio-off.json`：API-only/runtime-defaults 及仅关闭 Udio 的两项生产 lib 检查通过，输入前后无变化。首项含等待共享 Cargo 锁耗时 741.343 秒，关闭配置 84.937 秒，不据此宣称冷/热构建速度或提速倍数。
- `final-receipt.json`：汇总实际检查、固定 harness、当前范围 hashes 与明确排除的 SAP 测试漂移；`final-diff-check.json` 为文档收尾后的差异检查。

## 构建来源与原失败

旧对话 `gateway-udio-task-probe-20261109` 的首次后端运行是 5 passed / 3 failed，原因是夹具错误使用 `latestSongs`；已改为实际 worker 合同 `songs`。本次不覆盖原失败、不放宽生产解析，也不复用旧 EXE。

本次 `green-build-r2.json` 编译成功，但构建期间 SAP AI Core 及共用探测入口变化，因此未用它作最终测试。`green-build-r3.json` 再编译成功，期间仅范围外 `src/protocol/sap_aicore/wire_tests/probe_management.rs` 变化；该测试不在本批执行集合。Udio 与所选相邻生产输入保持不变，固定 Cargo 实际输出为 `green-r2.exe`（名称不代表构建轮次，来源见 `green-r2-frozen.json`）。其 SHA256 为 `640eb9177af98cdff143368213e1f535c95442931d599c7e74a17d8b43486471`。冻结脚本仅允许并记录上述唯一差异，没有把并行漂移擦掉或当成全仓一致。

这证明所选合同在该固定测试程序上通过，不证明最新共享全仓的所有测试来自单一无漂移快照。没有为了无关 SAP 测试变化继续反复重编译或覆盖他人工作。

## 保留边界

- 未真实登录、调用 Udio、验证当前 wire/session/模型目录/额度或播放权益；公开平台限制沿既有渠道文档，本次未重新联网，不将历史资料当作当日再次核实。
- 未完成正常浏览器页面到真实管理 TCP 的 Udio 联合验收、本地浏览器完整生命周期、平台内有效播放、Server/独立进程/原生包验收。
- 不将元数据成功改称可播放音频，不新增绕过下载限制的能力。
- 未读取真实凭据、修改运行实例、创建/删除容器、升级依赖、出包、提交或推送。测试程序不是 Gateway 手测 EXE。
- 原计划仍为 17 个新增模板，无新增整卡 `offline_ready`；其他渠道、P50/P60/P70 与集中真实验收继续保留。
