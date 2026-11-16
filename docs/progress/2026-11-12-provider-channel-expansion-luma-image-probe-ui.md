# R14 Luma Web 图片任务：正常网页联合验收

日期：2026-11-12（Asia/Shanghai）。承接单账号图片 probe 子批，本次只新增 opt-in 测试宿主和两行接线，不修改生产实现。

## 实际执行链

真实 Edge 新建隔离测试标签页 → 上批源码一致的正常 `main.tsx` Web 构建 → 透明 loopback 代理 → 正常管理 TCP 登录/Grant/probe → 精确合成账号 browser executor → 测试级图片 remap → 完整 PNG 解码 → 中性结果与审计。

资产 remap 只存在于测试宿主的逐请求 task-local middleware，不新增生产 loopback 例外。未调用真实 Luma，也没有运行本地供应商 browser worker。

## 观察与断言

1. 通过正常登录和凭据池账号库，打开 `luma-pipeline` 的 `exact` 账号图片测试，选择配置别名 `public-luma-image`。
2. 填入合成 prompt，进入费用确认后先取消。快照为 **0 管理 probe POST / 0 executor POST / 0 图片 GET**。
3. 再次进入确认并提交。快照为 **1 管理 probe POST / 1 executor POST / 1 图片 GET**，管理 HTTP 返回 200；没有第二次提交或备用账号调用。
4. 页面 AX 实际显示“已验证 1 张图片；模型绑定未验证，未恢复模型健康；供应商扣额未知”。保存了确认弹窗截图和结果 AX 观察记录；没有保存结果截图，不将确认截图冒称结果截图。
5. 测试宿主检查审计 `completed`、精确账号/模型别名、固定图片验证字段及敏感信息未进入审计；公共 Key 余额仍为 2；配置与 revision 未变；模型健康表仍为空。
6. 宿主退出后才写完成回执，确认 TCP/SQLite/合成 peer 收尾。代理 stdin 在非 TTY 终端不可写，随后核对 PID、进程名及完整脚本路径，只终止本轮拥有的 Node 代理。最终两进程与两监听端口均不存在。

## 验证与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-ui-20261112`。

- 定向非浏览器测试 6 passed，新增 opt-in 宿主显式运行 1 passed，合计 **7 个去重测试通过**。
- 本轮 Cargo `--locked --offline --lib --no-default-features --features runtime-defaults -j 4` 编译通过；编译期间输入无漂移，实际输出 harness 已复制冻结。
- harness SHA-256：`d8f54d42cb2bce71c56767cf39ed73bc4921395bf8d51ca7570ebebc369f9624`。
- 官方 Rust formatter、checker 31 项、ratchet 和 diff 检查通过；共享 checker 报告保存并恢复原 bytes。
- 有效行：`lumalabs_probe.rs` 121→123；新增 `lumalabs_probe/browser_host.rs` 为 128。
- 前端输入哈希与上批一致，复用上批 64 项、类型和隔离 main 构建，不重复声称本轮重跑。
- 主要证据：`browser-cancel.json`、`browser-confirmed.json`、`browser-result-observed.json`、`consent.jpg`、`browser-runtime-v1/completed.json`、`resource-final.json`、`final-receipt.json`。

运行宿主期间，另一工作线修改了 Sensenova probe 与测试，以及 `src/provider_runtime.rs` 的 Sensenova 模块接线。原始 `ui-host.json` 保留该阶段全部 6 个漂移路径；收尾又观察到 `src/protocol/sensenova.rs` 和 `src/provider_runtime/console_model_probe.rs` 的 Sensenova 接线。已保存两个共享父模块的直接差异：新增 Sensenova 模块、strict-key 准入与分派，未修改 Luma 产品分支。本批两个测试文件和 UI 输入未变。结论绑定冻结 harness，不冒称当前整个工作树已被同一构建验证；没有为其他工作线的持续变更无限重编。

## 未完成边界

本批关闭上批“真实浏览器 UI→管理 TCP→合成 executor 尚未联合验证”的缺口，不关闭真实供应商 session/wire、模型/action 绑定、额度和资产发行方验证。
没有验收窄屏/全部视觉状态、Server/PostgreSQL、本地供应商 browser worker、跨进程部署或原生包。未出 EXE、未操作在线实例、未提交或推送。
新增模板仍为 17，无新增整卡 `offline_ready`；P50/P60/P70 和总计划仍未完成。
