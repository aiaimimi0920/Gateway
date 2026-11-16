# Command Code 原生 JSON 回复保真

## 范围与实现

本次仅推进原生 Responses／Messages 的非流式回复承载，不代表原生请求、SSE、
System One、ZDR 或整个服务商已经收口。

- `commandcode/native_json.rs` 使用产品自有 sender，保留固定目标、精确账号 Bearer Key、
  禁跳转、错误脱敏及不重放；回复沿既有有界 body reader 和重复键拒绝解析器读取。
- 原生 envelope、终态、usage 和已知文本／函数工具结构校验与 canonical 转换分离。
  原生路径保留带非空 `type` 的扩展对象，不执行或解释上游工具。
- 公共 buffered 路径复用已有 JSON passthrough，不从 canonical 重建 body。
  原始 ID、reasoning、refusal、thinking/signature、引用和工具扩展得以保留；
  先观察上游模型，再只将公开顶层 `model` 投影为调用方别名。
- canonical／probe 路径仍拒绝其无法无损表示的回复，不把 refusal 或工具回复当文本探测成功。
- 原生用量与终态由产品侧独立观察，不让 `gateway_sources` 等不属于该产品的扩展字段触发
  Bedrock／Vertex／来源解析器。明确记录有无用量，不回落到未校验的通用 usage 别名求和。
  Responses 兼容 usage 别名也校验整数、求和溢出和 total 一致性。
- 未新增后台任务、无界缓冲、重试或另一套计费机制。读取取消随现有 future 释放；
  入站鉴权、预扣、provider attempt gate、成功审计和失败退款沿原有 pipeline。

## 合同依据

2026-11-14 本轮再次联网核对：

- https://commandcode.ai/docs/provider ：Responses／Messages 分别沿 OpenAI／Anthropic schema；
  Responses custom／内置工具另有明确合同。
- https://platform.claude.com/docs/en/api/messages ：Messages 包含 thinking/signature，
  stop reason 包含 pause_turn、refusal、model_context_window_exceeded。

本次保留这些回复内容不等于请求端已支持相应工具／历史回传，更不等于真实账号验收。

## 测试证据

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `commandcode-json-red.log`：新增公共 JSON 富内容回归在旧实现上失败，实际 502，预期 200。
  编译 13m08s；筛选合计 1 passed / 1 failed。该失败证明旧 canonical admission 拒绝有效原生回复。
- `commandcode-json-green.log`：初版修复产品回归 **60 passed / 0 failed**，编译 11m27s，运行 74.05s。
  随后交叉核验发现通用观测失败可进入 usage 别名溢出路径，补了上述独立观察和防回归断言；
  初版结果不能代替补强后结果。
- `commandcode-json-final.log`：补强后最终产品回归 **60 passed / 0 failed**，
  构建共 21m35s（含等待其他工作线的共享 Cargo 锁），运行 72.76s。
- `commandcode-json-shared-0.log`／`-1.log`／`-2.log`：共享 Responses **60 passed**、
  Anthropic **60 passed**、native Codex dispatch **22 passed**。
- `commandcode-json-checker-final.log`／`commandcode-json-ratchet-final.log`：checker **31 passed**，ratchet 通过。
- `commandcode-json-sensenova_.log`／`commandcode-json-sap_aicore_.log`：其他两项定向回归
  **24 passed／26 passed**，仅证明此轮公共接线未破坏对应测试，不是其功能整卡验收。
- 本切片源码独立 feature 检查已完成：
  `cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2`。
  `commandcode-json-feature.log`：**passed，exit 0**，总计 11m12s（含共享 Cargo target 锁等待）。
  session `50645` 已终态；没有重复启动或中断其他工作线。
- 定向 rustfmt、scoped diff、UTF-8 无 BOM 通过。受改源码 SHA-256 见
  `commandcode-json-source-hashes.json`；没有修改有效行 baseline。
  最终测试后再次比较源码哈希，全部一致。
- 新测试覆盖两种原生回复整体 JSON 等值（仅 model 别名投影）、usage=5、预付余额与审计；
  以及失败终态、空 type／非对象内容、负数／溢出 usage、错误正文及重复 JSON 键的退款和单次发送。

有效行增量：`commandcode.rs` 244→245，`responses.rs` 105→145，`messages.rs` 108→134，
`wire_tests.rs` 191→192，`buffered.rs` 433→445，`passthrough.rs` 439→443。
新生产模块 `native_json.rs` 106 行，新测试模块 129 行；所有受改文件均低于 500 行。

## 尚未完成

原生输入的 reasoning／媒体／custom 与内置工具、签名历史回传、Messages SSE 扩展内容、
ZDR、System One/Jev、适用 Server 真服务验收以及当前源码手测包仍开放。
三个服务商状态均不提升为“仅待真实账号”。
