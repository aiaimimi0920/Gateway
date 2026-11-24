# L2 Web：额度拒绝进入 Rust 额度分类

## 当前证据与问题

继续使用本机原有普通 Edge，重新绑定窗口后读到 Luma 升级弹窗：`Current plan · Free`、`5 of 5 left`，可访问性进度条名称为 `Credits remaining`、值为 5。没有重新提交生成、充值或升级。此快照不证明实际账单，也不证明前次请求已扣费或零扣费。随后用户在浏览器中切换页面，本轮不继续输入。

前次唯一真实 `Uni-1.1` 提交返回 402，见[真实请求回执](2026-11-21-luma-web-existing-edge.md#一次真实提交上游拒绝credits-不足)。Node worker 已返回 `lumalabs_action_payment_required` 并省略私有正文，但 Rust 的 `classify_lumalabs_browser_worker_failure` 仅用正文／stderr 分类，然后才覆盖 code 和 message。因此正常无正文回执在 Rust 中成为 `Unknown`，没有进入 `InsufficientQuota`。

## 修改范围

- 在 Luma worker 错误转换入口同时匹配固定错误码与 HTTP 402，构造 `InsufficientQuota`，保留状态、provider 与固定公开提示。
- 在处理正文和诊断前返回；不使用付款正文、stderr 或其他动态消息作为额度错误内容。
- 显式保留 `Abort`，不重试、不切换账号，不因构造额度错误而继承通用 quota 的 provider fallback。
- 其他错误码及状态沿用原分类；共享错误分类器、任务成功协议、费用结算和运行实例不变。

新增两个解析边界回归和一个真实本地 Node 子进程回归。后者经现有 worker supervisor、JSON 读取和 Luma 错误转换，验证额度分类、402、无 retry-after、不重放及无私有诊断；不访问供应商。负对照覆盖普通 402、同错误码的 401／503。

## 验证

修复前，定向 `luma_web_payment_refusal` 为 **1 passed / 2 failed**。两个失败均是 `left: Unknown`、`right: InsufficientQuota`；负对照通过。编译 23m43s，执行 0.23s。

修复后 `upstream::lumalabs` 过滤器为 **58 passed / 0 failed**，含全部新增用例、原 worker 资源回收与结果绑定、执行计划和输出合同回归；编译 23m08s，执行 4.40s。没有运行全仓库 Rust 测试。官方 rustfmt 检查、Node 夹具语法、31 项有效行检查器测试、仓库 ratchet、范围内 `git diff --check` 与 UTF-8 无 BOM 检查通过。前次 Node 浏览器操作回归未因本批仅 Rust 分类改动而重复执行。

红绿日志节选、各检查退出码和最终源码 SHA-256 位于[长期验证记录](../evidence/luma/gateway-luma-web-payment-20261121/verification.json)。

有效代码行：生产转换模块 84 → 102；解析回归 151 → 210；进程回归 133 → 141；Node 夹具 30 → 33。没有修改规模 baseline 或 exception。

L2 成功提交 → 原输出 ID → 完成事件 → 最终媒体仍未验收。本批不新增付费请求，不刷新凭据，不打包、提交、推送或部署；不关闭 L1／L4／L6。
