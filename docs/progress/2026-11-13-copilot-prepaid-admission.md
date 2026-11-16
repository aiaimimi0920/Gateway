# Copilot Local 额度准入与收尾

本块从 unlimited Key 的余额不变验证推进到真实 Local 收费额度的预扣和收尾。
测试仍使用显式可信内部 candidate／gate，公共 compiled credential 保持禁用。

## 改动

- 新增 `stage_send/copilot_tests/prepaid.rs`，通过正常 Local Key 保存接口配置额度，调用
  真实 stage_filter、cash admission、SDK sender 与成功／失败 finalizer，不手工模拟扣款。
- cash admission 明确排除 `copilot_sdk`。通用 OpenAI adapter／DirectHttp 标签不能证明
  SDK 提供现金结算所需的总 usage 和强制输出上限；在加载价格、HTTP 请求打包及现金
  reservation 之前拒绝。不改变其他 provider 的现金路径或既有收费政策。

## 验收场景

1. Chat buffered × message_prepaid／token_prepaid × SDK 成功／拒绝：发送前额度实际
   预扣；成功保留扣款，失败按既有政策退还。message 一次扣 1；token 未知 usage 时保留
   预扣估算，不能把这个数称为 SDK 实测 token。
2. message／token 额度为零：准入拒绝、零预扣、零 attempt、额度保持、失败审计。
3. cash Key：没有输出上限时返回 cash_output_limit_required；提供 max_tokens 后返回
   cash_adapter_unsupported。两者均零 attempt、无 cash_charge、现金余额保持且 ledger
   为空。证明现金准入安全拒绝，不是支持 SDK 现金结算。
4. 各场景审计保留未知 token 与上游 HTTP 状态，绑定真实测试 Key 和正确 attempt 数。

## 验证边界

本块直接调用真实内部 stages，没有经过公共路由选择；仅 Chat buffered，不包含收费
Responses、调用方取消、startup failure 或重启恢复验收。此前文档描述的未知用量现金
unresolved/reservation 是通用 ledger 兜底语义，不代表当前 Copilot 现金路径已被准入。
真实 CLI／账号、公开成功请求、原生 EXE 与 Server 仍开放，不增加整卡完成数。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-prepaid-20261113`。
最终 `prepaid.log`：3 passed / 0 failed / 0 ignored，包含四个实际 SDK 成功／拒绝组合、
两个零额度场景、两种 cash 拒绝场景。构建 15m23s，包含 package/build 锁等待和原生
依赖编译，不是纯增量耗时。4 个合成 child 各创建一次会话、发送一次、shutdown 一次，
均核对 Windows PID 已退出，见 `process-exit.json`。

复用同一新测试 EXE：cash_billing 回归 12 passed，access_balance 额度规则回归
3 passed，分别见 `cash-regression.log`、`quota-regression.log`。checker 31 项、
ratchet、formatter 和 scoped diff 检查通过。涉及源码均 UTF-8 无 BOM；有效行数为测试
注册 190、prepaid 测试 173、cash admission 197，未改 baseline。源码及二进制哈希保留
于 `source-final.json`、`binaries-final.json`。

没有提交、推送、出包、部署或真实上游请求。
